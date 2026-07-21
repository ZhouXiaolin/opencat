//! Pure media request types shared by engine and web backends.

use crate::resource::catalog::VideoInfoMeta;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrameTiming {
    pub timeline_start_secs: f64,
    pub timeline_duration_secs: Option<f64>,
    pub media_start_secs: f64,
    pub playback_rate: f64,
    pub looping: bool,
}

impl std::hash::Hash for VideoFrameTiming {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.timeline_start_secs.to_bits().hash(state);
        self.timeline_duration_secs.map(f64::to_bits).hash(state);
        self.media_start_secs.to_bits().hash(state);
        self.playback_rate.to_bits().hash(state);
        self.looping.hash(state);
    }
}

impl Default for VideoFrameTiming {
    fn default() -> Self {
        Self {
            timeline_start_secs: 0.0,
            timeline_duration_secs: None,
            media_start_secs: 0.0,
            playback_rate: 1.0,
            looping: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrameRequest {
    pub composition_time_secs: f64,
    pub timing: VideoFrameTiming,
}

impl VideoFrameRequest {
    pub fn resolve_time_secs(&self, info: &VideoInfoMeta) -> f64 {
        let elapsed_secs = self.timeline_elapsed_secs();

        if !self.timing.looping {
            return clamp_video_time(
                self.timing.media_start_secs + elapsed_secs * self.timing.playback_rate,
                info.duration_secs,
            );
        }

        match info.duration_secs {
            Some(duration_secs) if duration_secs > self.timing.media_start_secs => {
                let playable_duration = duration_secs - self.timing.media_start_secs;
                let wrapped = (elapsed_secs * self.timing.playback_rate) % playable_duration;
                self.timing.media_start_secs + wrapped
            }
            _ => clamp_video_time(
                self.timing.media_start_secs + elapsed_secs * self.timing.playback_rate,
                info.duration_secs,
            ),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.composition_time_secs + 1e-9 >= self.timing.timeline_start_secs
    }

    pub fn resolved_frame_index(&self, info: &VideoInfoMeta, fps: u32) -> u32 {
        let frame = self.resolve_time_secs(info) * fps.max(1) as f64;
        frame.round().clamp(0.0, u32::MAX as f64) as u32
    }

    fn timeline_elapsed_secs(&self) -> f64 {
        let raw_elapsed = (self.composition_time_secs - self.timing.timeline_start_secs).max(0.0);
        match self.timing.timeline_duration_secs {
            Some(duration_secs) if duration_secs > 0.0 => raw_elapsed.min(duration_secs),
            _ => raw_elapsed,
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
    use crate::resource::catalog::VideoInfoMeta;

    #[test]
    fn video_frame_request_applies_media_offset_and_rate() {
        let info = VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_secs: Some(60.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 2.0,
            timing: VideoFrameTiming {
                timeline_start_secs: 0.0,
                timeline_duration_secs: None,
                media_start_secs: 1.5,
                playback_rate: 0.5,
                looping: false,
            },
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
                timeline_start_secs: 0.0,
                timeline_duration_secs: None,
                media_start_secs: 1.0,
                playback_rate: 1.0,
                looping: true,
            },
        };

        assert!((request.resolve_time_secs(&info) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn video_frame_request_clamps_to_timeline_duration_end() {
        let info = VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_secs: Some(12.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 8.0,
            timing: VideoFrameTiming {
                timeline_start_secs: 3.0,
                timeline_duration_secs: Some(3.0),
                media_start_secs: 2.0,
                playback_rate: 1.0,
                looping: false,
            },
        };

        assert!((request.resolve_time_secs(&info) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn video_frame_request_uses_timeline_start_and_media_start() {
        let info = VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_secs: Some(60.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 5.5,
            timing: VideoFrameTiming {
                timeline_start_secs: 3.0,
                timeline_duration_secs: Some(18.0),
                media_start_secs: 12.0,
                playback_rate: 1.0,
                looping: false,
            },
        };

        assert!((request.resolve_time_secs(&info) - 14.5).abs() < 1e-6);
    }

    #[test]
    fn video_frame_request_loops_source_after_media_end() {
        let info = VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_secs: Some(4.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 5.5,
            timing: VideoFrameTiming {
                timeline_start_secs: 0.0,
                timeline_duration_secs: Some(10.0),
                media_start_secs: 2.0,
                playback_rate: 1.0,
                looping: true,
            },
        };

        assert!((request.resolve_time_secs(&info) - 3.5).abs() < 1e-6);
    }

    #[test]
    fn video_frame_request_reports_invisible_before_timeline_start() {
        let request = VideoFrameRequest {
            composition_time_secs: 2.999,
            timing: VideoFrameTiming {
                timeline_start_secs: 3.0,
                timeline_duration_secs: Some(18.0),
                media_start_secs: 12.0,
                playback_rate: 1.0,
                looping: false,
            },
        };

        assert!(!request.is_visible());
    }

    #[test]
    fn video_frame_request_resolves_last_epoch_after_timeline_duration() {
        let info = VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_secs: Some(60.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 22.0,
            timing: VideoFrameTiming {
                timeline_start_secs: 3.0,
                timeline_duration_secs: Some(18.0),
                media_start_secs: 12.0,
                playback_rate: 1.0,
                looping: false,
            },
        };

        assert_eq!(request.resolved_frame_index(&info, 30), 900);
    }
}
