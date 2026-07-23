//! Pure media request types shared by engine and web backends.
//!
//! Design language still authors offsets/durations in seconds on
//! [`VideoFrameTiming`]; resolution into authoritative microsecond timestamps
//! is owned by core via [`VideoFrameRequest::resolve_time_micros`].

use crate::probe::catalog::VideoInfoMeta;
use crate::time::{secs_to_micros, timestamp_micros_to_secs, DurationMicros, TimestampMicros};

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
    /// Resolve the authoritative media timestamp (seconds) for host decode APIs
    /// that still take floating seconds. Prefer [`Self::resolve_time_micros`] for
    /// core → host media contracts.
    pub fn resolve_time_secs(&self, info: &VideoInfoMeta) -> f64 {
        timestamp_micros_to_secs(self.resolve_time_micros(info).0)
    }

    /// Authoritative media timestamp in microseconds. Core owns timeline
    /// rounding; hosts must not re-derive source frame indices.
    pub fn resolve_time_micros(&self, info: &VideoInfoMeta) -> TimestampMicros {
        let elapsed_secs = self.timeline_elapsed_secs();
        let media_start = self.timing.media_start_secs.max(0.0);
        let rate = if self.timing.playback_rate.is_finite() && self.timing.playback_rate > 0.0 {
            self.timing.playback_rate
        } else {
            1.0
        };

        if !self.timing.looping {
            let raw = media_start + elapsed_secs * rate;
            return clamp_video_time_micros(raw, info.duration_micros);
        }

        match info.duration_micros {
            Some(DurationMicros(duration_us)) if duration_us > 0 => {
                let duration_secs = timestamp_micros_to_secs(duration_us);
                if duration_secs > media_start {
                    let playable = duration_secs - media_start;
                    let wrapped = (elapsed_secs * rate) % playable;
                    TimestampMicros(secs_to_micros(media_start + wrapped))
                } else {
                    clamp_video_time_micros(media_start + elapsed_secs * rate, info.duration_micros)
                }
            }
            _ => clamp_video_time_micros(media_start + elapsed_secs * rate, info.duration_micros),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.composition_time_secs + 1e-9 >= self.timing.timeline_start_secs
    }

    /// Diagnostic helper only — never put the result in FrameMediaPlan.
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

fn clamp_video_time_micros(time_secs: f64, duration: Option<DurationMicros>) -> TimestampMicros {
    let clamped_secs = time_secs.max(0.0);
    let micros = secs_to_micros(clamped_secs);
    match duration {
        Some(DurationMicros(d)) if d > 0 => TimestampMicros(micros.min(d)),
        _ => TimestampMicros(micros),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::catalog::VideoInfoMeta;
    use crate::time::DurationMicros;

    fn info(duration_secs: Option<f64>) -> VideoInfoMeta {
        VideoInfoMeta {
            width: 1920,
            height: 1080,
            duration_micros: duration_secs.map(|s| DurationMicros(secs_to_micros(s))),
        }
    }

    #[test]
    fn video_frame_request_applies_media_offset_and_rate() {
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

        assert_eq!(request.resolve_time_micros(&info(Some(60.0))).0, 2_500_000);
        assert!((request.resolve_time_secs(&info(Some(60.0))) - 2.5).abs() < 1e-6);
    }

    #[test]
    fn video_frame_request_wraps_looping_video_time() {
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

        assert_eq!(request.resolve_time_micros(&info(Some(5.0))).0, 3_000_000);
    }

    #[test]
    fn video_frame_request_clamps_to_timeline_duration_end() {
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

        assert_eq!(request.resolve_time_micros(&info(Some(12.0))).0, 5_000_000);
    }

    #[test]
    fn video_frame_request_uses_timeline_start_and_media_start() {
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

        assert_eq!(request.resolve_time_micros(&info(Some(60.0))).0, 14_500_000);
    }

    #[test]
    fn video_frame_request_loops_source_after_media_end() {
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

        assert_eq!(request.resolve_time_micros(&info(Some(4.0))).0, 3_500_000);
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

        assert_eq!(request.resolved_frame_index(&info(Some(60.0)), 30), 900);
        assert_eq!(request.resolve_time_micros(&info(Some(60.0))).0, 30_000_000);
    }

    #[test]
    fn resolve_time_micros_clamps_to_duration_boundary() {
        let request = VideoFrameRequest {
            composition_time_secs: 100.0,
            timing: VideoFrameTiming::default(),
        };
        assert_eq!(
            request.resolve_time_micros(&info(Some(1.5))).0,
            1_500_000
        );
    }

    #[test]
    fn resolve_time_micros_with_non_integer_composition_fps() {
        // Composition timeline at ~29.97 fps equivalent seconds still rounds in core.
        // 1001/30000 s composition time + media_start 0 → micros owned by core.
        let request = VideoFrameRequest {
            composition_time_secs: 1001.0 / 30_000.0,
            timing: VideoFrameTiming::default(),
        };
        let micros = request.resolve_time_micros(&info(Some(60.0))).0;
        // 1001/30000 s = 0.0333666… → 33367 µs after round
        assert_eq!(micros, 33_367);
    }
}
