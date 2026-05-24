use std::collections::HashMap;
use std::path::Path;

use anyhow::anyhow;
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_types::ImageRef;
use opencat_core::ir::media_plan::FrameMediaPlan;
use opencat_core::platform::frame_consumer::{FrameConsumer, RenderSessionHeader};
use opencat_core::probe::{AssetHandle, AssetLoader};
use opencat_core::resource::asset_id::AssetId;
use skia_safe::{AlphaType, Canvas, ColorType, Data, Image, ImageInfo, RuntimeEffect, images};

use crate::executor::{DrawError, EngineDrawExecutor, EnginePreparedFrameMedia};
use crate::resource::media::MediaContext;

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
// AssetPathSource: module-private trait for AssetId → Path resolution
// ---------------------------------------------------------------------------

/// Module-private trait: resolve AssetId to a filesystem path.
/// Engine has two sources (AssetPathStore for session path,
/// EngineLoader for pipeline path).
trait AssetPathSource {
    fn resolve_path(&self, id: &AssetId) -> Option<&Path>;
}

impl AssetPathSource for opencat_core::resource::AssetPathStore {
    fn resolve_path(&self, id: &AssetId) -> Option<&Path> {
        self.path(id)
    }
}

impl AssetPathSource for crate::resource::loader::EngineLoader {
    fn resolve_path(&self, id: &AssetId) -> Option<&Path> {
        self.handle(id).and_then(|h| h.local_path())
    }
}

// ---------------------------------------------------------------------------
// Unified prepare_frame (module-private)
// ---------------------------------------------------------------------------

/// Decode media for a single frame. Generic over the asset path source.
fn prepare_frame<P: AssetPathSource>(
    plan: &FrameMediaPlan,
    paths: &P,
    video: &mut MediaContext,
) -> Result<EnginePreparedFrameMedia, ConsumerError> {
    let mut sk_images = Vec::new();
    let mut image_index = HashMap::new();

    for image_ref in &plan.images {
        match image_ref {
            ImageRef::Static { asset_id } => {
                let aid = AssetId(asset_id.clone());
                if let Some(path) = paths.resolve_path(&aid) {
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
                frame_index,
            } => {
                let aid = AssetId(asset_id.clone());
                if let Some(path) = paths.resolve_path(&aid) {
                    if let Ok(frame) = video.frame_rgba_by_path(path, *frame_index) {
                        let info = ImageInfo::new(
                            (frame.width as i32, frame.height as i32),
                            ColorType::RGBA8888,
                            AlphaType::Unpremul,
                            None,
                        );
                        if let Some(sk_image) = images::raster_from_data(
                            &info,
                            Data::new_copy(&frame.data),
                            frame.width as usize * 4,
                        ) {
                            let idx = sk_images.len();
                            sk_images.push(sk_image);
                            image_index.insert(image_ref.clone(), idx);
                        }
                    }
                }
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
// EngineFrameConsumer (RenderSession / AssetPathStore path)
// ---------------------------------------------------------------------------

/// Engine-side FrameConsumer for the RenderSession path (uses AssetPathStore).
pub struct EngineFrameConsumer<'a> {
    pub executor: &'a mut EngineDrawExecutor,
    pub paths: &'a opencat_core::resource::AssetPathStore,
    pub media_ctx: &'a mut MediaContext,
    pub canvas: &'a mut Canvas,
}

impl FrameConsumer for EngineFrameConsumer<'_> {
    type Output = ();
    type Error = ConsumerError;

    fn consume_frame(
        &mut self,
        header: &RenderSessionHeader,
        draw: &mut DrawOpFrame,
        plan: &FrameMediaPlan,
    ) -> Result<(), ConsumerError> {
        let prepared = prepare_frame(plan, self.paths, self.media_ctx)?;
        self.executor.execute(header, draw, &prepared, self.canvas)?;
        Ok(())
    }
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
        self.executor.execute(header, draw, &prepared, self.canvas)?;
        Ok(())
    }
}
