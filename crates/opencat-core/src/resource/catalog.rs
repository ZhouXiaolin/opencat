use crate::time::DurationMicros;

/// Host-facing video metadata used during resolve/render.
///
/// Duration is always microsecond-based. Layout-critical width/height must be
/// positive; duration may be `None` when the probe could not determine length
/// (looping/clamp then treat the stream as open-ended).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_micros: Option<DurationMicros>,
}

impl VideoInfoMeta {
    pub fn duration_secs(&self) -> Option<f64> {
        self.duration_micros
            .map(|d| crate::time::timestamp_micros_to_secs(d.0))
    }
}
