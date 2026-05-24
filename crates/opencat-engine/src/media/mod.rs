//! Engine-owned media implementations.
//!
//! Core defines media contracts and planning. This module owns native FFmpeg
//! decode/encode, Skia bitmap preparation, video frame caches, and audio mix.

pub mod audio;
pub mod decode;
pub mod encode;
pub mod video;
pub mod video_cache;

pub use audio::{AudioBuffer, AudioIntervalCache, DecodedAudioCache};
pub use decode::{AudioTrack, VideoDecodeCache, VideoInfo, decode_audio_to_f32_stereo};
pub use encode::{Mp4Config, encode_rgba_frames};
pub use video::{
    EngineVideoProvider, MediaContext, VideoBitmap, VideoFrameRequest, VideoFrameTiming,
    VideoPreviewQuality,
};
