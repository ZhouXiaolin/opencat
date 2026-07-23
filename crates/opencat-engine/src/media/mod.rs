//! Engine-owned media implementations.
//!
//! Core defines media contracts and planning (`AudioPlan`, `VideoFrameRequest`,
//! `VideoFrameTiming`). This module owns native FFmpeg decode/encode, Skia
//! bitmap preparation, video frame caches, and audio mix.
//!
//! AudioPlan is the **sole** composition-level canonical output. The engine
//! exclusively reads `pipeline.info().audio_plan` (from core's
//! [`collect_audio_plan`]) for decode/mix/encode — it must not re-walk the
//! composition tree to produce a second set of semantics.

pub mod audio;
pub mod decode;
pub mod encode;
pub mod seek;
pub mod video;
pub mod video_cache;

pub use decode::{AudioTrack, VideoDecodeCache, VideoInfo, decode_audio_to_f32_stereo};
pub use encode::{Mp4Config, encode_rgba_frames};
pub use video::{
    MediaContext, VideoBitmap, VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality,
};
