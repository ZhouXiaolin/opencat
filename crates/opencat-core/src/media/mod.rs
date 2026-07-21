//! Pure timeline-to-media-time semantics.

pub mod audio_plan;
pub mod types;

pub use audio_plan::{collect_audio_plan, AudioPlan, AudioSegment};
pub use types::{VideoFrameRequest, VideoFrameTiming};
