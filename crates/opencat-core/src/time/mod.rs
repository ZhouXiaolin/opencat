//! Typed time contract shared by composition timeline, video/audio media plans,
//! and host decode requests.
//!
//! Design language still authors durations/offsets in seconds; core normalizes
//! those into the units below before any host-facing media contract leaves the
//! pipeline. Hosts only convert to platform APIs at the last call site.

pub mod convert;
mod types;

pub use convert::{
    duration_secs_to_frames, frames_to_duration_secs, frames_to_timestamp_micros, ms_to_duration_micros,
    optional_secs_to_duration_micros, secs_to_micros, timestamp_micros_to_frame,
    timestamp_micros_to_secs,
};
pub use types::{
    DurationMicros, DurationRange, FrameCount, FrameIndex, RationalFrameRate, TimestampMicros,
};
