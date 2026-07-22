use std::collections::HashMap;

use anyhow::anyhow;
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_types::ImageRef;
use opencat_core::ir::media_plan::FrameMediaPlan;
use opencat_core::ir::GeneratedImageId;
use opencat_core::resource::asset_id::{AssetId, ResourceKind};
use skia_safe::{AlphaType, Canvas, ColorType, Data, Image, ImageInfo, RuntimeEffect, images};

use crate::executor::{DrawError, EngineDrawExecutor, EnginePreparedFrameMedia};
use crate::media::MediaContext;

// ---------------------------------------------------------------------------
// ConsumerError: bridges anyhow::Error / DrawError → std::error::Error
// ---------------------------------------------------------------------------

/// Error returned while preparing or executing a frame from a [`RenderFrame`].
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

/// Decode media for a single frame via EngineLoader paths and host-cached
/// generated images. Generated RGBA comes entirely from
/// [`FrameMediaPlan::generated_images`] — no pipeline table access.
fn prepare_frame(
    plan: &FrameMediaPlan,
    loader: &crate::resource::loader::EngineLoader,
    video: &mut MediaContext,
    generated_cache: &mut HashMap<GeneratedImageId, Image>,
) -> Result<EnginePreparedFrameMedia, ConsumerError> {
    let mut sk_images = Vec::new();
    let mut image_index = HashMap::new();

    // External (static asset) image references.
    for image_ref in &plan.images {
        match image_ref {
            ImageRef::Static { asset_id } => {
                let aid = AssetId::new(ResourceKind::Image, asset_id.clone());
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
            // Static bucket only carries external images; video refs live in
            // `plan.video_frames` and generated refs in `plan.generated_images`.
            ImageRef::VideoFrame { .. } | ImageRef::Generated { .. } => {}
        }
    }

    // Video frame references: resolved from the authoritative `time_micros`,
    // never from a source frame index (the contract carries none).
    for image_ref in &plan.video_frames {
        match image_ref {
            ImageRef::VideoFrame {
                asset_id,
                time_micros,
            } => {
                let aid = AssetId::new(ResourceKind::Video, asset_id.clone());
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
            // Defensive: the video bucket only carries video refs.
            ImageRef::Static { .. } | ImageRef::Generated { .. } => {}
        }
    }

    // Core-generated images (color-emoji bitmap glyphs). Full RGBA is on the
    // FrameMediaPlan entry; the engine caches Skia images by GeneratedImageId.
    for entry in &plan.generated_images {
        let image_ref = ImageRef::Generated { id: entry.id };
        if image_index.contains_key(&image_ref) {
            continue;
        }
        let sk_image = if let Some(cached) = generated_cache.get(&entry.id) {
            cached.clone()
        } else {
            let info = ImageInfo::new(
                (entry.width as i32, entry.height as i32),
                ColorType::RGBA8888,
                AlphaType::Unpremul,
                None,
            );
            let Some(sk_image) = images::raster_from_data(
                &info,
                Data::new_copy(&entry.rgba),
                entry.width as usize * 4,
            ) else {
                continue;
            };
            generated_cache.insert(entry.id, sk_image.clone());
            sk_image
        };
        let idx = sk_images.len();
        sk_images.push(sk_image);
        image_index.insert(image_ref, idx);
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
// Direct RenderFrame execution (no FrameConsumer / RenderSessionHeader)
// ---------------------------------------------------------------------------

/// Execute a core [`RenderFrame`] onto a Skia canvas.
///
/// Hosts own media decode/cache; this path only:
/// 1. resolves `FrameMediaPlan` into Skia images / runtime effects,
/// 2. hydrates Lottie animations from the engine loader,
/// 3. replays the typed DrawOp IR.
///
/// Composition size / fps / frames are not required for draw execution — they
/// remain available on `pipeline.info()` for callers that need them.
pub fn execute_render_frame(
    draw: &mut DrawOpFrame,
    plan: &FrameMediaPlan,
    executor: &mut EngineDrawExecutor,
    loader: &crate::resource::loader::EngineLoader,
    media_ctx: &mut MediaContext,
    generated_cache: &mut HashMap<GeneratedImageId, Image>,
    canvas: &mut Canvas,
) -> Result<(), ConsumerError> {
    let prepared = prepare_frame(plan, loader, media_ctx, generated_cache)?;
    executor.ensure_lottie_animations(draw, |bundle_id| {
        let asset_id = AssetId::new(ResourceKind::Lottie, bundle_id.to_string());
        loader
            .handle(&asset_id)
            .and_then(|h| h.read_bytes().ok())
            .map(|c| c.into_owned())
    });
    executor.execute(draw, &prepared, canvas)?;
    Ok(())
}
