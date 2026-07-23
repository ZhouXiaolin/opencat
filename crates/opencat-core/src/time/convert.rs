//! Deterministic unit conversions owned by core.

use super::types::{DurationMicros, FrameCount, FrameIndex, RationalFrameRate, TimestampMicros};

const MICROS_PER_SEC: f64 = 1_000_000.0;

/// Design-language seconds → microseconds (round half away from zero via `round`).
pub fn secs_to_micros(secs: f64) -> u64 {
    if !secs.is_finite() || secs <= 0.0 {
        return 0;
    }
    let micros = (secs * MICROS_PER_SEC).round();
    if micros >= u64::MAX as f64 {
        u64::MAX
    } else {
        micros as u64
    }
}

/// Microseconds → seconds for host platform APIs.
pub fn timestamp_micros_to_secs(micros: u64) -> f64 {
    micros as f64 / MICROS_PER_SEC
}

pub fn duration_micros_to_secs(micros: u64) -> f64 {
    timestamp_micros_to_secs(micros)
}

/// Duration in seconds → frame count at integer fps (legacy FrameCtx path).
///
/// Matches historical ceil-with-epsilon so short positive durations stay visible
/// and near-integer fractional error does not inflate the count.
pub fn duration_secs_to_frames(duration_secs: f64, fps: u32) -> u32 {
    duration_secs_to_frame_count(duration_secs, RationalFrameRate::integer(fps)).0
}

pub fn duration_secs_to_frame_count(duration_secs: f64, rate: RationalFrameRate) -> FrameCount {
    if !duration_secs.is_finite() || duration_secs <= 0.0 {
        return FrameCount(0);
    }
    let frame_position = duration_secs * rate.as_f64();
    let frames = (frame_position - 1e-6).ceil().max(1.0);
    if frames >= u32::MAX as f64 {
        FrameCount(u32::MAX)
    } else {
        FrameCount(frames as u32)
    }
}

/// Frame count → duration seconds at integer fps.
pub fn frames_to_duration_secs(frames: u32, fps: u32) -> f64 {
    frames as f64 / fps.max(1) as f64
}

/// Composition frame index → authoritative media/composition timestamp.
pub fn frames_to_timestamp_micros(frame: FrameIndex, rate: RationalFrameRate) -> TimestampMicros {
    // t = frame * den / num  seconds → micros, exact rational path via f64.
    if rate.num == 0 {
        return TimestampMicros(0);
    }
    let secs = frame.0 as f64 * rate.den as f64 / rate.num as f64;
    TimestampMicros(secs_to_micros(secs))
}

/// Timestamp → nearest frame index at the given rate (for diagnostics only;
/// host video decode must use [`TimestampMicros`], never a guessed source frame).
pub fn timestamp_micros_to_frame(time: TimestampMicros, rate: RationalFrameRate) -> FrameIndex {
    let secs = timestamp_micros_to_secs(time.0);
    let frame = (secs * rate.as_f64()).round().clamp(0.0, u32::MAX as f64) as u32;
    FrameIndex(frame)
}

/// Optional media duration in seconds → micros (None stays None).
pub fn optional_secs_to_duration_micros(secs: Option<f64>) -> Option<DurationMicros> {
    secs.filter(|s| s.is_finite() && *s > 0.0)
        .map(|s| DurationMicros(secs_to_micros(s)))
}

/// Milliseconds (probe tags) → micros.
pub fn ms_to_duration_micros(ms: u64) -> DurationMicros {
    DurationMicros(ms.saturating_mul(1_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secs_round_trip_common_media_times() {
        assert_eq!(secs_to_micros(0.0), 0);
        assert_eq!(secs_to_micros(-1.0), 0);
        assert_eq!(secs_to_micros(1.5), 1_500_000);
        assert_eq!(secs_to_micros(12.0), 12_000_000);
        assert!((timestamp_micros_to_secs(1_500_000) - 1.5).abs() < 1e-12);
    }

    #[test]
    fn duration_secs_to_frames_tolerates_fraction_rounding() {
        assert_eq!(duration_secs_to_frames(10.0000003 / 30.0, 30), 10);
        assert_eq!(duration_secs_to_frames(0.000000001, 30), 1);
    }

    #[test]
    fn rational_ntsc_frame_timestamp() {
        let rate = RationalFrameRate::new(30_000, 1_001);
        // frame 0 → 0
        assert_eq!(frames_to_timestamp_micros(FrameIndex(0), rate).0, 0);
        // frame 30000 ≈ 1001 seconds at 30000/1001
        let t = frames_to_timestamp_micros(FrameIndex(30_000), rate);
        assert_eq!(t.0, 1_001_000_000);
    }

    #[test]
    fn non_integer_rate_boundary_frames() {
        let rate = RationalFrameRate::new(24_000, 1_001); // ~23.976
        let t0 = frames_to_timestamp_micros(FrameIndex(0), rate);
        let t1 = frames_to_timestamp_micros(FrameIndex(1), rate);
        assert_eq!(t0.0, 0);
        // 1001/24000 s ≈ 41708.333… µs → 41708
        assert_eq!(t1.0, 41_708);
        assert_eq!(timestamp_micros_to_frame(t1, rate), FrameIndex(1));
    }
}
