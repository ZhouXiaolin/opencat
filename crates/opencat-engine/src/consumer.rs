use std::collections::HashMap;

use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_types::ImageRef;
use opencat_core::ir::media_plan::FrameMediaPlan;
use opencat_core::ir::GeneratedImageId;
use opencat_core::ir::asset_id::{AssetId, ResourceKind};
use skia_safe::{AlphaType, Canvas, ColorType, Data, Image, ImageInfo, RuntimeEffect, images};

use crate::executor::{DrawError, EngineDrawExecutor, EnginePreparedFrameMedia};
use crate::media::MediaContext;

// ---------------------------------------------------------------------------
// MediaError — typed error for frame media preparation failures
// ---------------------------------------------------------------------------

/// Typed error for frame media preparation failures.
///
/// Each variant identifies the specific asset that caused the failure, enabling
/// host-level error handling per asset type (image, video, Lottie, generated
/// image, runtime effect) without pattern-matching on opaque strings.
#[derive(Debug)]
pub enum MediaError {
    MissingImage {
        asset_id: String,
    },
    MissingVideo {
        asset_id: String,
    },
    MissingLottieBundle {
        bundle_id: String,
    },
    ImageDecodeFailed {
        asset_id: String,
        detail: String,
    },
    VideoFrameDecodeFailed {
        asset_id: String,
        detail: String,
    },
    GeneratedImageDecodeFailed {
        id: GeneratedImageId,
        detail: String,
    },
    RuntimeEffectCompileFailed {
        hash: u64,
        detail: String,
    },
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaError::MissingImage { asset_id } => {
                write!(f, "media error: missing image `{asset_id}`")
            }
            MediaError::MissingVideo { asset_id } => {
                write!(f, "media error: missing video `{asset_id}`")
            }
            MediaError::MissingLottieBundle { bundle_id } => {
                write!(f, "media error: missing Lottie bundle `{bundle_id}`")
            }
            MediaError::ImageDecodeFailed { asset_id, detail } => {
                write!(
                    f,
                    "media error: image `{asset_id}` decode failed: {detail}"
                )
            }
            MediaError::VideoFrameDecodeFailed { asset_id, detail } => {
                write!(
                    f,
                    "media error: video frame `{asset_id}` decode failed: {detail}"
                )
            }
            MediaError::GeneratedImageDecodeFailed { id, detail } => {
                write!(
                    f,
                    "media error: generated image {id:?} decode failed: {detail}"
                )
            }
            MediaError::RuntimeEffectCompileFailed { hash, detail } => {
                write!(
                    f,
                    "media error: runtime effect {hash:#x} compile failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for MediaError {}

// ---------------------------------------------------------------------------
// ConsumerError: bridges anyhow::Error / DrawError / MediaError →
// std::error::Error
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

impl From<MediaError> for ConsumerError {
    fn from(err: MediaError) -> Self {
        Self(Box::new(err))
    }
}

// ---------------------------------------------------------------------------
// Validators — atomic fail-fast before any media decode
// ---------------------------------------------------------------------------

/// Check that every image referenced in `plan` has a handle in `loader`.
fn validate_image_handles(
    plan: &FrameMediaPlan,
    loader: &crate::resource::loader::EngineLoader,
) -> Result<(), MediaError> {
    for image_ref in &plan.images {
        match image_ref {
            ImageRef::Static { asset_id } => {
                let aid = AssetId::new(ResourceKind::Image, asset_id.clone());
                if loader.handle(&aid).is_none() {
                    return Err(MediaError::MissingImage {
                        asset_id: asset_id.clone(),
                    });
                }
            }
            // Static bucket only carries external images.
            ImageRef::VideoFrame { .. } | ImageRef::Generated { .. } => {}
        }
    }
    Ok(())
}

/// Check that every video frame referenced in `plan` has a handle in `loader`.
fn validate_video_handles(
    plan: &FrameMediaPlan,
    loader: &crate::resource::loader::EngineLoader,
) -> Result<(), MediaError> {
    for image_ref in &plan.video_frames {
        match image_ref {
            ImageRef::VideoFrame { asset_id, .. } => {
                let aid = AssetId::new(ResourceKind::Video, asset_id.clone());
                if loader.handle(&aid).is_none() {
                    return Err(MediaError::MissingVideo {
                        asset_id: asset_id.clone(),
                    });
                }
            }
            ImageRef::Static { .. } | ImageRef::Generated { .. } => {}
        }
    }
    Ok(())
}

/// Check that every Lottie bundle referenced in `plan` has a handle in
/// `loader`.
fn validate_lottie_handles(
    plan: &FrameMediaPlan,
    loader: &crate::resource::loader::EngineLoader,
) -> Result<(), MediaError> {
    for bundle_id in &plan.lottie_bundles {
        let aid = AssetId::new(ResourceKind::Lottie, bundle_id.clone());
        if loader.handle(&aid).is_none() {
            return Err(MediaError::MissingLottieBundle {
                bundle_id: bundle_id.clone(),
            });
        }
    }
    Ok(())
}

/// Decode media for a single frame via EngineLoader paths and host-cached
/// generated images. Generated RGBA comes entirely from
/// [`FrameMediaPlan::generated_images`] — no pipeline table access.
///
/// All failures produce a typed [`MediaError`] — no silent skip. Callers must
/// validate handles via [`validate_image_handles`] / [`validate_video_handles`]
/// before calling this function to ensure atomic fail-fast (no partial decode).
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
                let handle = loader.handle(&aid).ok_or_else(|| {
                    MediaError::MissingImage {
                        asset_id: asset_id.clone(),
                    }
                })?;
                let path = handle.local_path().ok_or_else(|| {
                    MediaError::ImageDecodeFailed {
                        asset_id: asset_id.clone(),
                        detail: "no local path for handle".into(),
                    }
                })?;
                let bytes = std::fs::read(path).map_err(|e| {
                    MediaError::ImageDecodeFailed {
                        asset_id: asset_id.clone(),
                        detail: e.to_string(),
                    }
                })?;
                let sk_image = Image::from_encoded(Data::new_copy(&bytes)).ok_or_else(|| {
                    MediaError::ImageDecodeFailed {
                        asset_id: asset_id.clone(),
                        detail: "Image::from_encoded returned None".into(),
                    }
                })?;
                let idx = sk_images.len();
                sk_images.push(sk_image);
                image_index.insert(image_ref.clone(), idx);
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
                    .ok_or_else(|| MediaError::MissingVideo {
                        asset_id: asset_id.clone(),
                    })?;
                let frame =
                    video.frame_rgba_at_time_by_path(path, *time_micros as f64 / 1_000_000.0)
                        .map_err(|e| MediaError::VideoFrameDecodeFailed {
                            asset_id: asset_id.clone(),
                            detail: e.to_string(),
                        })?;
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
                .ok_or_else(|| MediaError::VideoFrameDecodeFailed {
                    asset_id: asset_id.clone(),
                    detail: "failed to create Skia image from decoded video frame".into(),
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
            let src = images::raster_from_data(
                &info,
                Data::new_copy(&entry.rgba),
                entry.width as usize * 4,
            )
            .ok_or_else(|| MediaError::GeneratedImageDecodeFailed {
                id: entry.id,
                detail: "images::raster_from_data returned None".into(),
            })?;
            generated_cache.insert(entry.id, src.clone());
            src
        };
        let idx = sk_images.len();
        sk_images.push(sk_image);
        image_index.insert(image_ref, idx);
    }

    let mut runtime_effects = Vec::with_capacity(plan.runtime_effects.len());
    for effect_ref in &plan.runtime_effects {
        let effect = RuntimeEffect::make_for_shader(&effect_ref.sksl, None)
            .map_err(|e| MediaError::RuntimeEffectCompileFailed {
                hash: effect_ref.hash,
                detail: e.to_string(),
            })?;
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
/// 1. validates that all media is available (atomic fail-fast),
/// 2. resolves `FrameMediaPlan` into Skia images / runtime effects,
/// 3. hydrates Lottie animations from the engine loader,
/// 4. replays the typed DrawOp IR.
///
/// Any missing or failed media entry produces a typed [`MediaError`]. No partial
/// drawing is observable after a media preparation failure — validation and
/// decode both complete before the first DrawOp is replayed.
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
    // Atomic validation: check all media handles exist BEFORE starting any
    // decode. This ensures no partial work is observable if any media is
    // missing — the frame stays uncommitted.
    validate_image_handles(plan, loader)?;
    validate_video_handles(plan, loader)?;
    validate_lottie_handles(plan, loader)?;

    // Now decode (all failures are hard errors, no silent skips).
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

#[cfg(test)]
mod tests {
    use super::*;
    use opencat_core::ir::media_plan::FrameMediaPlan;
    use opencat_core::ir::draw_types::ImageRef;
    use crate::resource::loader::EngineLoader;
    use opencat_core::ir::GeneratedImageId;
    use tempfile::TempDir;

    fn make_loader() -> (TempDir, EngineLoader) {
        let tmp = TempDir::new().expect("tmp dir");
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&cache).ok();
        let loader = EngineLoader::new(
            tmp.path().to_path_buf(),
            cache,
        )
        .expect("loader");
        (tmp, loader)
    }

    #[test]
    fn validate_image_handles_reports_missing_image() {
        let (_tmp, loader) = make_loader();
        let plan = FrameMediaPlan {
            images: vec![ImageRef::Static {
                asset_id: "nonexistent.png".into(),
            }],
            ..Default::default()
        };
        let err = validate_image_handles(&plan, &loader).unwrap_err();
        match err {
            MediaError::MissingImage { asset_id } => {
                assert_eq!(asset_id, "nonexistent.png");
            }
            other => panic!("expected MissingImage, got {other:?}"),
        }
    }

    #[test]
    fn validate_image_handles_passes_with_no_images() {
        let (_tmp, loader) = make_loader();
        let plan = FrameMediaPlan::default();
        validate_image_handles(&plan, &loader)
            .expect("empty plan should validate");
    }

    #[test]
    fn validate_video_handles_reports_missing_video() {
        let (_tmp, loader) = make_loader();
        let plan = FrameMediaPlan {
            video_frames: vec![ImageRef::VideoFrame {
                asset_id: "missing.mp4".into(),
                time_micros: 166_667,
            }],
            ..Default::default()
        };
        let err = validate_video_handles(&plan, &loader).unwrap_err();
        match err {
            MediaError::MissingVideo { asset_id } => {
                assert_eq!(asset_id, "missing.mp4");
            }
            other => panic!("expected MissingVideo, got {other:?}"),
        }
    }

    #[test]
    fn validate_lottie_handles_reports_missing_bundle() {
        let (_tmp, loader) = make_loader();
        let plan = FrameMediaPlan {
            lottie_bundles: vec!["lottie:missing".into()],
            ..Default::default()
        };
        let err = validate_lottie_handles(&plan, &loader).unwrap_err();
        match err {
            MediaError::MissingLottieBundle { bundle_id } => {
                assert_eq!(bundle_id, "lottie:missing");
            }
            other => panic!("expected MissingLottieBundle, got {other:?}"),
        }
    }

    #[test]
    fn validate_empty_plan_passes() {
        let (_tmp, loader) = make_loader();
        let plan = FrameMediaPlan::default();
        validate_image_handles(&plan, &loader).expect("no images");
        validate_video_handles(&plan, &loader).expect("no videos");
        validate_lottie_handles(&plan, &loader).expect("no lottie");
    }

    #[test]
    fn media_error_display_contains_variant_info() {
        let e1 = MediaError::MissingImage { asset_id: "a.png".into() };
        let e2 = MediaError::MissingVideo { asset_id: "b.mp4".into() };
        let e3 = MediaError::MissingLottieBundle { bundle_id: "c".into() };
        let e4 = MediaError::ImageDecodeFailed { asset_id: "d.png".into(), detail: "e".into() };
        let e5 = MediaError::VideoFrameDecodeFailed { asset_id: "f.mp4".into(), detail: "g".into() };
        let e6 = MediaError::GeneratedImageDecodeFailed { id: GeneratedImageId(0xDEAD), detail: "h".into() };
        let e7 = MediaError::RuntimeEffectCompileFailed { hash: 0x1234, detail: "i".into() };

        assert!(e1.to_string().contains("a.png"), "missing image display: {}", e1);
        assert!(e2.to_string().contains("b.mp4"), "missing video display: {}", e2);
        assert!(e3.to_string().contains("c"), "missing lottie display: {}", e3);
        assert!(e4.to_string().contains("d.png"), "image decode display: {}", e4);
        assert!(e5.to_string().contains("f.mp4"), "video decode display: {}", e5);
        assert!(e6.to_string().contains("GeneratedImageId"), "generated image display: {}", e6);
        assert!(e7.to_string().contains("0x1234"), "effect compile display: {}", e7);
    }
}
