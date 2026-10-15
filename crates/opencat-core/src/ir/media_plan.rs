use std::sync::Arc;

use super::draw_types::{EffectRef, ImageRef};
use super::generated_image::GeneratedImageId;

/// Host-facing payload for one core-generated image needed by the current frame.
///
/// Carries the stable id plus full RGBA so hosts never need to open the
/// pipeline's internal generated-image table. Hosts may cache platform images
/// by [`GeneratedImageId`]; cache epoch is a host concern and is not part of
/// this contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameGeneratedImage {
    pub id: GeneratedImageId,
    pub width: u32,
    pub height: u32,
    /// RGBA_8888, unpremultiplied. Length is `width * height * 4`.
    ///
    /// Empty when [`Self::gpu`] is set.
    pub rgba: Arc<[u8]>,
    /// GPU-resident pixels (opaque host handle). When set, hosts resolve the
    /// texture instead of uploading `rgba`. See
    /// [`GpuImageHandle`](crate::ir::generated_image::GpuImageHandle).
    pub gpu: Option<crate::ir::generated_image::GpuImageHandle>,
}

/// Per-frame media preparation plan describing what a host must prepare for the
/// current frame, derived deterministically from the `DrawOpFrame` plus the
/// pipeline's generated-image table. Each bucket is deduplicated so a given
/// image / video / Lottie / effect / generated image appears at most once.
///
/// Core only describes the current frame's needs; it never predicts future
/// frames or dictates decoder/seek/prefetch strategy. Hosts build their own
/// decoder cache, seek, and prefetch windows on top of this.
///
/// `generated_images` includes full RGBA content — `RenderFrame` is the sole
/// core→host current-frame contract; hosts do not read pipeline-internal
/// tables for glyph bitmaps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameMediaPlan {
    /// External (static asset) image references, deduplicated.
    pub images: Vec<ImageRef>,
    /// Video frame references (canonical `AssetId` + authoritative
    /// `time_micros`), deduplicated.
    pub video_frames: Vec<ImageRef>,
    /// Lottie bundle ids referenced by `DrawOp::LottieRect`, deduplicated.
    pub lottie_bundles: Vec<String>,
    /// Runtime shader effects, deduplicated by effect id.
    pub runtime_effects: Vec<EffectRef>,
    /// Core-generated images (color-emoji bitmap glyphs) referenced by
    /// `ImageRef::Generated`, deduplicated by id, each with full RGBA.
    pub generated_images: Vec<FrameGeneratedImage>,
    /// Author-asserted YUV→RGB conversion matrices (`data-color-matrix`) for
    /// the video assets this frame references, keyed by canonical asset id.
    /// Hosts must decode the named assets with the declared matrix instead of
    /// trusting stream metadata (capture tools often tag BT.709 over BT.601
    /// encodes). Assets absent from this map decode automatically.
    pub video_color_matrices: std::collections::BTreeMap<String, crate::media::VideoColorMatrix>,
    /// Effective on-device draw size per `ImageRect`-drawn reference (destination
    /// size × accumulated ancestor `Scale`, ceiled, max-merged across all draw
    /// sites). Hosts may decode down to this size — never up — without visible
    /// change; assets absent from this map decode at native resolution.
    ///
    /// Only `DrawOp::ImageRect` contributes: `DrawOp::Image` has no destination
    /// (whole-image semantics), generated glyphs ship exact-size RGBA, and
    /// runtime-effect child textures are sampled at shader-controlled UVs.
    pub image_draw_sizes: std::collections::HashMap<ImageRef, (u32, u32)>,
}
