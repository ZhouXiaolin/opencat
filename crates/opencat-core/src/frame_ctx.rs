//! Per-frame composition context. Frame/fps remain integer for layout/script
//! surfaces; media contracts use [`crate::time`] micros / rational rates.

pub use crate::time::{duration_secs_to_frames, frames_to_duration_secs};

#[derive(Debug, Clone, Copy)]
pub struct FrameCtx {
    pub frame: u32,
    pub fps: u32,
    pub width: i32,
    pub height: i32,
    pub frames: u32,
}

impl FrameCtx {
    pub fn time_secs(&self) -> f64 {
        frames_to_duration_secs(self.frame, self.fps)
    }

    pub fn duration_secs(&self) -> f64 {
        frames_to_duration_secs(self.frames, self.fps)
    }

    /// Authoritative composition timestamp for the current frame index.
    pub fn time_micros(&self) -> crate::time::TimestampMicros {
        crate::time::frames_to_timestamp_micros(
            crate::time::FrameIndex(self.frame),
            crate::time::RationalFrameRate::integer(self.fps),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScriptFrameCtx {
    pub frame: u32,
    pub fps: u32,
    pub total_frames: u32,
    pub current_frame: u32,
    pub scene_frames: u32,
    pub time_secs: f64,
    pub total_duration_secs: f64,
    pub current_time_secs: f64,
    pub scene_duration_secs: f64,
}

impl ScriptFrameCtx {
    pub fn global(frame_ctx: &FrameCtx) -> Self {
        Self {
            frame: frame_ctx.frame,
            fps: frame_ctx.fps,
            total_frames: frame_ctx.frames,
            current_frame: frame_ctx.frame,
            scene_frames: frame_ctx.frames,
            time_secs: frame_ctx.time_secs(),
            total_duration_secs: frame_ctx.duration_secs(),
            current_time_secs: frame_ctx.time_secs(),
            scene_duration_secs: frame_ctx.duration_secs(),
        }
    }

    pub fn for_segment(frame_ctx: &FrameCtx, start_frame: u32, scene_frames: u32) -> Self {
        let max_local_frame = scene_frames.saturating_sub(1);
        let current_frame = frame_ctx
            .frame
            .saturating_sub(start_frame)
            .min(max_local_frame);
        Self {
            frame: frame_ctx.frame,
            fps: frame_ctx.fps,
            total_frames: frame_ctx.frames,
            current_frame,
            scene_frames,
            time_secs: frame_ctx.time_secs(),
            total_duration_secs: frame_ctx.duration_secs(),
            current_time_secs: frames_to_duration_secs(current_frame, frame_ctx.fps),
            scene_duration_secs: frames_to_duration_secs(scene_frames, frame_ctx.fps),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::duration_secs_to_frames;
    use super::FrameCtx;

    #[test]
    fn duration_secs_to_frames_tolerates_fraction_rounding() {
        assert_eq!(duration_secs_to_frames(10.0000003 / 30.0, 30), 10);
    }

    #[test]
    fn duration_secs_to_frames_keeps_positive_duration_visible() {
        assert_eq!(duration_secs_to_frames(0.000000001, 30), 1);
    }

    #[test]
    fn frame_ctx_time_micros_matches_integer_fps() {
        let ctx = FrameCtx {
            frame: 90,
            fps: 30,
            width: 320,
            height: 180,
            frames: 300,
        };
        assert_eq!(ctx.time_micros().0, 3_000_000);
    }
}
