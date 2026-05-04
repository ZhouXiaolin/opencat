//! 纯描述结构，无 ffmpeg / skia 依赖。

use crate::core::resource::catalog::VideoInfoMeta;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VideoPreviewQuality {
    Scrubbing,
    Realtime,
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoFrameTiming {
    pub media_offset_secs: f64,
    pub playback_rate: f64,
    pub looping: bool,
}

impl std::hash::Hash for VideoFrameTiming {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.media_offset_secs.to_bits().hash(state);
        self.playback_rate.to_bits().hash(state);
        self.looping.hash(state);
    }
}

impl Default for VideoFrameTiming {
    fn default() -> Self {
        Self {
            media_offset_secs: 0.0,
            playback_rate: 1.0,
            looping: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoFrameRequest {
    pub composition_time_secs: f64,
    pub timing: VideoFrameTiming,
    pub quality: VideoPreviewQuality,
    /// Caller's desired output size in pixels. `MediaContext` quantizes and
    /// clamps this against the source resolution; the decoder/cache trust the
    /// already-normalized value.
    pub target_size: Option<(u32, u32)>,
}

impl VideoFrameRequest {
    pub fn resolve_time_secs(&self, info: &VideoInfoMeta) -> f64 {
        let composition_time_secs = self.composition_time_secs.max(0.0);
        let local_time_secs =
            self.timing.media_offset_secs + composition_time_secs * self.timing.playback_rate;

        if !self.timing.looping {
            return clamp_video_time(local_time_secs, info.duration_secs);
        }

        match info.duration_secs {
            Some(duration_secs) if duration_secs > self.timing.media_offset_secs => {
                let playable_duration = duration_secs - self.timing.media_offset_secs;
                let wrapped =
                    (composition_time_secs * self.timing.playback_rate) % playable_duration;
                self.timing.media_offset_secs + wrapped
            }
            _ => clamp_video_time(local_time_secs, info.duration_secs),
        }
    }
}

fn clamp_video_time(time_secs: f64, duration_secs: Option<f64>) -> f64 {
    let clamped = time_secs.max(0.0);
    match duration_secs {
        Some(duration_secs) if duration_secs > 0.0 => clamped.min(duration_secs),
        _ => clamped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::resource::catalog::VideoInfoMeta;

    #[test]
    fn video_frame_request_applies_media_offset_and_rate() {
        let info = VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_secs: Some(12.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 2.0,
            timing: VideoFrameTiming {
                media_offset_secs: 1.5,
                playback_rate: 0.5,
                looping: false,
            },
            quality: VideoPreviewQuality::Exact,
            target_size: None,
        };

        assert!((request.resolve_time_secs(&info) - 2.5).abs() < 1e-6);
    }

    #[test]
    fn video_frame_request_wraps_looping_video_time() {
        let info = VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_secs: Some(5.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 6.0,
            timing: VideoFrameTiming {
                media_offset_secs: 1.0,
                playback_rate: 1.0,
                looping: true,
            },
            quality: VideoPreviewQuality::Scrubbing,
            target_size: None,
        };

        assert!((request.resolve_time_secs(&info) - 3.0).abs() < 1e-6);
    }
}
