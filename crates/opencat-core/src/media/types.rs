//! Pure media request types shared by engine and web backends.
//!
//! Design language still authors offsets/durations in seconds on
//! [`VideoFrameTiming`]; resolution into authoritative microsecond timestamps
//! is owned by core via [`VideoFrameRequest::resolve_time_micros`].

use crate::probe::catalog::VideoInfoMeta;
use crate::time::{DurationMicros, TimestampMicros, secs_to_micros, timestamp_micros_to_secs};

/// The YUV→RGB matrix a host decoder must convert a video asset with.
///
/// Authors declare it per asset via `data-color-matrix` when a stream's
/// metadata lies about its actual encoding (common with capture tools that
/// encode BT.601 but tag BT.709). `None` keeps the host's automatic
/// metadata-driven selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoColorMatrix {
    /// ITU-R BT.601 coefficients (SD-era / most capture-tool encodes).
    Bt601,
    /// ITU-R BT.709 coefficients (HD-era broadcasts, modern encodes).
    Bt709,
}

impl VideoColorMatrix {
    /// Parse an author-facing matrix token (`"bt601"` | `"bt709"`). `None` for
    /// `auto`/empty keeps the host's automatic metadata-driven selection.
    /// Shared by every frontend that accepts `data-color-matrix` (markup attr,
    /// JSONL `dataColorMatrix`), so unknown tokens fail the parse identically.
    pub fn parse_token(value: &str) -> Option<Option<Self>> {
        match value {
            "auto" | "" => Some(None),
            "bt601" => Some(Some(Self::Bt601)),
            "bt709" => Some(Some(Self::Bt709)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrameTiming {
    pub timeline_start_secs: f64,
    pub timeline_duration_secs: Option<f64>,
    pub media_start_secs: f64,
    pub playback_rate: f64,
    pub looping: bool,
    /// Author-asserted YUV→RGB matrix override for this asset (see
    /// [`VideoColorMatrix`]). `None` = automatic (stream metadata).
    pub color_matrix: Option<VideoColorMatrix>,
    /// Opt-in HyperFrames removal semantics: when true the video subtree leaves
    /// the tree once its timeline window closes. Default false keeps the
    /// historical contract — `data-duration` clamps media time only and the
    /// node keeps holding its last frame.
    #[serde(default)]
    pub hide_after_end: bool,
}

impl std::hash::Hash for VideoFrameTiming {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.timeline_start_secs.to_bits().hash(state);
        self.timeline_duration_secs.map(f64::to_bits).hash(state);
        self.media_start_secs.to_bits().hash(state);
        self.playback_rate.to_bits().hash(state);
        self.looping.hash(state);
        self.color_matrix.hash(state);
        self.hide_after_end.hash(state);
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
            color_matrix: None,
            hide_after_end: false,
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

    /// A video is on screen during its timeline window: from `timeline_start_secs`
    /// until `timeline_start_secs + timeline_duration_secs` when a duration is
    /// declared (HyperFrames `data-start`/`data-duration` semantics — the element
    /// is removed from the tree once its window closes). Without a declared
    /// duration the video stays visible from its start onward, matching the
    /// historical hold-last-frame behaviour.
    pub fn is_visible(&self) -> bool {
        let t = self.composition_time_secs + 1e-9;
        if t < self.timing.timeline_start_secs {
            return false;
        }
        match self.timing.timeline_duration_secs {
            Some(duration_secs) if duration_secs > 0.0 && self.timing.hide_after_end => {
                t < self.timing.timeline_start_secs + duration_secs
            }
            _ => true,
        }
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


