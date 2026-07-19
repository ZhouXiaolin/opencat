use std::collections::HashMap;

use anyhow::anyhow;
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_types::ImageRef;
use opencat_core::ir::media_plan::FrameMediaPlan;
use opencat_core::platform::frame_consumer::{FrameConsumer, RenderSessionHeader};
use opencat_core::probe::{AssetHandle, AssetLoader};
use opencat_core::resource::asset_id::AssetId;
use skia_safe::{AlphaType, Canvas, ColorType, Data, Image, ImageInfo, RuntimeEffect, images};

use crate::executor::{DrawError, EngineDrawExecutor, EnginePreparedFrameMedia};
use crate::media::MediaContext;

// ---------------------------------------------------------------------------
// ConsumerError: bridges anyhow::Error / DrawError → std::error::Error
// ---------------------------------------------------------------------------

/// Error returned by engine frame consumers.
#[derive(Debug)]
pub struct ConsumerError(Box<dyn std::error::Error + Send + Sync + 'static>);

impl std::fmt::Display for ConsumerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ConsumerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<anyhow::Error> for ConsumerError {
    fn from(err: anyhow::Error) -> Self {
        Self(err.into())
    }
}

impl From<DrawError> for ConsumerError {
    fn from(err: DrawError) -> Self {
        Self(Box::new(err))
    }
}

// ---------------------------------------------------------------------------
// prepare_frame (module-private)
// ---------------------------------------------------------------------------

/// Decode media for a single frame via EngineLoader paths.
fn prepare_frame(
    plan: &FrameMediaPlan,
    loader: &crate::resource::loader::EngineLoader,
    video: &mut MediaContext,
) -> Result<EnginePreparedFrameMedia, ConsumerError> {
    let mut sk_images = Vec::new();
    let mut image_index = HashMap::new();

    for image_ref in &plan.images {
        match image_ref {
            ImageRef::Static { asset_id } => {
                let aid = AssetId(asset_id.clone());
                if let Some(path) = loader.handle(&aid).and_then(|h| h.local_path()) {
                    if let Ok(bytes) = std::fs::read(path) {
                        if let Some(sk_image) = Image::from_encoded(Data::new_copy(&bytes)) {
                            let idx = sk_images.len();
                            sk_images.push(sk_image);
                            image_index.insert(image_ref.clone(), idx);
                        }
                    }
                }
            }
            ImageRef::VideoFrame {
                asset_id,
                time_micros,
                ..
            } => {
                let aid = AssetId(asset_id.clone());
                let path = loader
                    .handle(&aid)
                    .and_then(|h| h.local_path())
                    .ok_or_else(|| anyhow!("video asset {:?} not found in loader", aid))?;
                let frame =
                    video.frame_rgba_at_time_by_path(path, *time_micros as f64 / 1_000_000.0)?;
                let info = ImageInfo::new(
                    (frame.width as i32, frame.height as i32),
                    ColorType::RGBA8888,
                    AlphaType::Unpremul,
                    None,
                );
                let sk_image = images::raster_from_data(
                    &info,
                    Data::new_copy(&frame.data),
                    frame.width as usize * 4,
                )
                .ok_or_else(|| {
                    anyhow!(
                        "failed to create Skia image from decoded video frame {:?}",
                        aid
                    )
                })?;
                let idx = sk_images.len();
                sk_images.push(sk_image);
                image_index.insert(image_ref.clone(), idx);
            }
        }
    }

    let mut runtime_effects = Vec::with_capacity(plan.runtime_effects.len());
    for effect_ref in &plan.runtime_effects {
        let effect = RuntimeEffect::make_for_shader(&effect_ref.sksl, None)
            .map_err(|e| anyhow!("RuntimeEffect {:#x} compile failed: {}", effect_ref.hash, e))?;
        runtime_effects.push(effect);
    }

    Ok(EnginePreparedFrameMedia {
        images: sk_images,
        image_index,
        runtime_effects,
    })
}

// ---------------------------------------------------------------------------
// EngineLoaderFrameConsumer (Pipeline / EngineLoader path)
// ---------------------------------------------------------------------------

/// Engine-side FrameConsumer for the pipeline path (uses EngineLoader).
pub struct EngineLoaderFrameConsumer<'a> {
    pub executor: &'a mut EngineDrawExecutor,
    pub loader: &'a crate::resource::loader::EngineLoader,
    pub media_ctx: &'a mut MediaContext,
    pub canvas: &'a mut Canvas,
}

impl FrameConsumer for EngineLoaderFrameConsumer<'_> {
    type Output = ();
    type Error = ConsumerError;

    fn consume_frame(
        &mut self,
        header: &RenderSessionHeader,
        draw: &mut DrawOpFrame,
        plan: &FrameMediaPlan,
    ) -> Result<(), ConsumerError> {
        let prepared = prepare_frame(plan, self.loader, self.media_ctx)?;
        self.executor.ensure_lottie_animations(draw, |bundle_id| {
            let asset_id = AssetId(bundle_id.to_string());
            self.loader
                .handle(&asset_id)
                .and_then(|h| h.read_bytes().ok())
                .map(|c| c.into_owned())
        });
        self.executor
            .execute(header, draw, &prepared, self.canvas)?;
        Ok(())
    }
}
