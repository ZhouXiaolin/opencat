//! Coarse preview requests independent of any platform renderer.

use crate::media::types::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewFrameRequest {
    pub frame_index: u32,
    pub fps: u32,
    pub quality: VideoPreviewQuality,
    pub target_size: Option<(u32, u32)>,
}

impl PreviewFrameRequest {
    pub fn composition_time_secs(&self) -> f64 {
        self.frame_index as f64 / self.fps.max(1) as f64
    }

    pub fn video_frame_request(&self, timing: VideoFrameTiming) -> VideoFrameRequest {
        VideoFrameRequest {
            composition_time_secs: self.composition_time_secs(),
            timing,
            quality: self.quality,
            target_size: self.target_size,
        }
    }
}
