use super::draw_types::{EffectRef, ImageRef};
use super::generated_image::GeneratedImageId;

/// Per-frame media preparation plan describing what a host must prepare for the
/// current frame, derived deterministically from the `DrawOpFrame`. Each bucket
/// is deduplicated so a given image / video / Lottie / effect appears at most
/// once.
///
/// Core only describes the current frame's needs; it never predicts future
/// frames or dictates decoder/seek/prefetch strategy. Hosts build their own
/// decoder cache, seek, and prefetch windows on top of this.
#[derive(Clone, Debug, Default)]
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
    /// Core-generated image ids (color-emoji bitmap glyphs) referenced by
    /// `ImageRef::Generated`, deduplicated. The RGBA lives in the pipeline's
    /// `GeneratedImageTable`; this list only tells hosts which entries to
    /// resolve for the current frame.
    pub generated_images: Vec<GeneratedImageId>,
}
