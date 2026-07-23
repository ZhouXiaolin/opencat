//! Pure timeline-to-media-time semantics.

pub mod audio_plan;
pub mod types;

pub use audio_plan::{AudioPlan, AudioSegment, collect_audio_plan};
pub use types::{VideoFrameRequest, VideoFrameTiming};
