use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::cache::lru::BoundedLruCache;
use std::collections::HashMap;
use opencat_core::ir::draw_types::ImageRef;
use opencat_core::ir::media_plan::FrameMediaPlan;
use opencat_core::ir::GeneratedImageId;
use opencat_core::ir::asset_id::{AssetId, ResourceKind};
use skia_safe::{AlphaType, Canvas, ColorType, Data, Image, ImageInfo, RuntimeEffect, images};

use crate::executor::{DrawError, EngineDrawExecutor, EnginePreparedFrameMedia};
use crate::media::MediaContext;

/// GPU 图像句柄的解析器（GPU 管线用）。
///
/// 把 `FrameMediaPlan` 里带 [`opencat_core::ir::generated_image::GpuImageHandle`]
/// 的条目导入为 Skia image（零拷贝：纹理已在显存里）。CPU 管线传 `None`，
/// 此时任何 GPU 句柄都是硬错误（说明两路径被混用）。
///
/// 实现方是 [`crate::gpu_compositor::CompositorResolver`]（在 GL 锁内调用）。
pub trait GpuImageResolver {
    /// `id` 是句柄携带的共享纹理凭据（见
    /// [`opencat_core::ir::generated_image::GpuImageHandle::token`]）。
    fn import(
        &mut self,
        id: opencat_gpu_native::TextureId,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Image>;
}

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
            ImageRef::VideoFrame { .. } | ImageRef::Generated { .. } | ImageRef::Encoded { .. } => {}
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
            ImageRef::Static { .. } | ImageRef::Generated { .. } | ImageRef::Encoded { .. } => {}
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

/// Alpha type for a decoded video frame handed to Skia.
///
/// Sources without an alpha plane (e.g. `yuv420p`) decode with alpha == 255
/// on every pixel, so their straight RGBA bytes are byte-identical to the
/// premultiplied form. Tagging them `Premul` skips Skia's per-pixel
/// premultiply on every draw. Sources that do carry alpha stay `Unpremul`
/// until a real premultiply is required.
fn video_frame_alpha_type(has_alpha: bool) -> AlphaType {
    if has_alpha {
        AlphaType::Unpremul
    } else {
        AlphaType::Premul
    }
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
    generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
    mut gpu: Option<&mut dyn GpuImageResolver>,
) -> Result<EnginePreparedFrameMedia, ConsumerError> {
    let mut sk_images = Vec::new();
    let mut image_index = HashMap::new();
    let mut image_scales = Vec::new();

    // External (static asset) image references. When the plan carries a draw
    // size, decode down to it (shrink-only): drawing smaller never needs
    // full-resolution pixels, and decoding at draw size removes the
    // full-resolution RGBA peak as well as the per-frame file read + decode.
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
                let target = plan.image_draw_sizes.get(image_ref).copied();
                let (sk_image, scale) =
                    video.static_image_scaled(path, target).map_err(|e| {
                        MediaError::ImageDecodeFailed {
                            asset_id: asset_id.clone(),
                            detail: e.to_string(),
                        }
                    })?;
                let idx = sk_images.len();
                sk_images.push(sk_image);
                image_scales.push(scale);
                image_index.insert(image_ref.clone(), idx);
            }
            // Static bucket only carries external images; video refs live in
            // `plan.video_frames`, generated refs in `plan.generated_images`,
            // encoded refs in `plan.encoded_images`.
            ImageRef::VideoFrame { .. } | ImageRef::Generated { .. } | ImageRef::Encoded { .. } => {}
        }
    }

    // Frame-embedded encoded images (svg `<image>` bytes interned by core).
    // Core forwards the bytes untouched; decoding is engine work. Drawn at
    // natural decode size through the CTM — no draw-size pre-shrink, svg
    // alignment wants resvg's single bicubic resample.
    let mut encoded_pixmaps = HashMap::new();
    for entry in &plan.encoded_images {
        let image_ref = ImageRef::Encoded { data: entry.data };
        let sk_image = video.encoded_image(entry.data, &entry.bytes).map_err(|e| {
            MediaError::ImageDecodeFailed {
                asset_id: format!("<encoded #{}>", entry.data.0),
                detail: e.to_string(),
            }
        })?;
        // tiny Pattern 重采样的解码像素在 encoded_image 内生成（resvg 字节
        // 布局）；这里取用即可。缺失不致命，回退 Skia 路径。
        if let Some(pm) = video.encoded_tiny_pixmap(entry.data) {
            encoded_pixmaps.insert(image_ref.clone(), pm);
        }
        let idx = sk_images.len();
        sk_images.push(sk_image);
        image_scales.push(1.0);
        image_index.insert(image_ref, idx);
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
                // Author-asserted YUV→RGB matrix (data-color-matrix): decode
                // the asset with its real encoding, not a mislabeled tag.
                if let Some(&matrix) = plan.video_color_matrices.get(asset_id) {
                    video.set_video_color_matrix(path, matrix);
                }
                let target = plan.image_draw_sizes.get(image_ref).copied();
                let (frame, scale) = video
                    .frame_rgba_at_time_sized(path, *time_micros as f64 / 1_000_000.0, target)
                    .map_err(|e| MediaError::VideoFrameDecodeFailed {
                        asset_id: asset_id.clone(),
                        detail: e.to_string(),
                    })?;
                let info = ImageInfo::new(
                    (frame.width as i32, frame.height as i32),
                    ColorType::RGBA8888,
                    video_frame_alpha_type(frame.has_alpha),
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
                image_scales.push(scale);
                image_index.insert(image_ref.clone(), idx);
            }
            // Defensive: the video bucket only carries video refs.
            ImageRef::Static { .. } | ImageRef::Generated { .. } | ImageRef::Encoded { .. } => {}
        }
    }

    // Core-generated images. Either CPU RGBA (color-emoji bitmap glyphs,
    // `canvas putImageData`) or a GPU handle (`ctx.three` render target in the
    // GPU pipeline). The engine caches CPU Skia images by GeneratedImageId.
    for entry in &plan.generated_images {
        let image_ref = ImageRef::Generated { id: entry.id };
        if image_index.contains_key(&image_ref) {
            continue;
        }
        // GPU-resident pixels: import the wgpu texture zero-copy. No CPU
        // upload, no cache — the texture lives only for this frame.
        if let Some(handle) = entry.gpu {
            let resolver = gpu.as_deref_mut().ok_or_else(|| {
                MediaError::GeneratedImageDecodeFailed {
                    id: entry.id,
                    detail: "plan carries a GPU image handle but the CPU pipeline \
                             is active (set OC_GPU_PIPELINE=0, or drop the handle)"
                        .into(),
                }
            })?;
            let id = opencat_gpu_native::TextureId::from_raw(handle.token);
            let sk_image = resolver.import(id, entry.width, entry.height).map_err(|e| {
                MediaError::GeneratedImageDecodeFailed {
                    id: entry.id,
                    detail: format!("GPU texture import failed: {e}"),
                }
            })?;
            let idx = sk_images.len();
            sk_images.push(sk_image);
            image_scales.push(1.0);
            image_index.insert(image_ref, idx);
            continue;
        }
        // Bounded LRU mirrors the core table's frame watermark: a surface the
        // script rewrites every frame would otherwise grow this map one Skia
        // image (GPU-uploadable, so ~1x the RGBA size) per frame for the whole
        // composition.
        let bytes = entry.width as usize * entry.height as usize * 4;
        let generated_enabled =
            !opencat_core::ablate::ablated(opencat_core::ablate::Ablation::GeneratedImages);
        let sk_image = if let Some(cached) = generated_enabled
            .then(|| generated_cache.get_cloned(&entry.id))
            .flatten()
        {
            cached
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
            generated_cache.insert_with_weight(entry.id, src.clone(), bytes.max(1));
            src
        };
        let idx = sk_images.len();
        sk_images.push(sk_image);
        image_scales.push(1.0);
        image_index.insert(image_ref, idx);
    }

    let mut runtime_effects = Vec::with_capacity(plan.runtime_effects.len());
    for effect_ref in &plan.runtime_effects {
        let effect = compile_runtime_effect_cached(effect_ref.hash, &effect_ref.sksl)?;
        runtime_effects.push(crate::executor::PreparedRuntimeEffect {
            effect,
            hash: effect_ref.hash,
            sksl: effect_ref.sksl.clone(),
        });
    }


    Ok(EnginePreparedFrameMedia {
        images: sk_images,
        image_index,
        runtime_effects,
        image_scales,
        encoded_pixmaps,
    })
}

// 会话级 SKSL 编译 memo（thread_local，按 hash 复用编译好的
// `RuntimeEffect`）：同一效果逐帧绘制时不再重复编译。宿主在会话结束/
// 换 composition 时可调 `clear_runtime_effect_cache` 归还内存。
thread_local! {
    static RUNTIME_EFFECT_CACHE: std::cell::RefCell<HashMap<u64, RuntimeEffect>> =
        std::cell::RefCell::new(HashMap::new());
}

/// 清空 SKSL 编译 memo（会话切换时宿主可选调用；不清也不影响正确性，
/// 只是常驻内存）。
pub fn clear_runtime_effect_cache() {
    RUNTIME_EFFECT_CACHE.with(|c| c.borrow_mut().clear());
}

pub(crate) fn compile_runtime_effect_cached(
    hash: u64,
    sksl: &str,
) -> Result<RuntimeEffect, ConsumerError> {
    let compile = || {
        RuntimeEffect::make_for_shader(sksl, None).map_err(|e| {
            MediaError::RuntimeEffectCompileFailed {
                hash,
                detail: e.to_string(),
            }
        })
    };
    if opencat_core::ablate::ablated(opencat_core::ablate::Ablation::RuntimeEffects) {
        return Ok(compile()?);
    }
    RUNTIME_EFFECT_CACHE.with(|c| {
        if let Some(hit) = c.borrow().get(&hash) {
            // skia-safe 对象非 Clone；这里复用引用计数？RuntimeEffect 内部是
            // refcounted handle —— 直接 clone 句柄即可。
            return Ok(hit.clone());
        }
        let effect = compile()?;
        c.borrow_mut().insert(hash, effect.clone());
        Ok(effect)
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
    generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
    canvas: &mut Canvas,
) -> Result<(), ConsumerError> {
    execute_render_frame_with_gpu(
        draw,
        plan,
        executor,
        loader,
        media_ctx,
        generated_cache,
        None,
        canvas,
    )
}

/// [`execute_render_frame`] 的 GPU 变体：多一个 GPU 句柄解析器。
///
/// 解析器在 **GL 锁内**调用（Skia 导入纹理需要 current 的 GL context）。
#[allow(clippy::too_many_arguments)]
pub fn execute_render_frame_with_gpu(
    draw: &mut DrawOpFrame,
    plan: &FrameMediaPlan,
    executor: &mut EngineDrawExecutor,
    loader: &crate::resource::loader::EngineLoader,
    media_ctx: &mut MediaContext,
    generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
    gpu: Option<&mut dyn GpuImageResolver>,
    canvas: &mut Canvas,
) -> Result<(), ConsumerError> {
    // Atomic validation: check all media handles exist BEFORE starting any
    // decode. This ensures no partial work is observable if any media is
    // missing — the frame stays uncommitted.
    validate_image_handles(plan, loader)?;
    validate_video_handles(plan, loader)?;
    validate_lottie_handles(plan, loader)?;

    // `FrameMediaPlan` 的契约把预取窗口留给宿主（见 core 的 `media_plan` 文档），
    // 这里就是宿主那一侧：把本帧的播放头位置告诉实时解码线程。必须**在解码之前**
    // 发——线程要先知道播放头在哪，才可能在 `prepare_frame` 等帧之前就把它解好。
    // 离线渲染没挂线程，这里是空操作。
    prefetch_frame_media(plan, loader, media_ctx);

    // Now decode (all failures are hard errors, no silent skips).
    let prepared = prepare_frame(plan, loader, media_ctx, generated_cache, gpu)?;

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

/// 把本帧计划里的视频引用转成预取提示。
///
/// 提示里的时刻是**素材时刻**（`time_micros`），与解码线程填充窗口用的键一致。
/// 绘制大小和色彩矩阵一并带上：线程要在另一个 `MediaContext` 上解码，它拿不到
/// 本帧的这两项。
fn prefetch_frame_media(
    plan: &FrameMediaPlan,
    loader: &crate::resource::loader::EngineLoader,
    media_ctx: &MediaContext,
) {
    for image_ref in &plan.video_frames {
        let ImageRef::VideoFrame {
            asset_id,
            time_micros,
        } = image_ref
        else {
            continue;
        };
        let aid = AssetId::new(ResourceKind::Video, asset_id.clone());
        let Some(path) = loader.handle(&aid).and_then(|h| h.local_path()) else {
            continue;
        };
        media_ctx.prefetch_video(
            path,
            *time_micros as f64 / 1_000_000.0,
            plan.image_draw_sizes.get(image_ref).copied(),
            plan.video_color_matrices.get(asset_id).copied(),
        );
    }
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
