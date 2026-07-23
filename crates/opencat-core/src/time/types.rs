//! Newtypes for the public media time contract.

/// Zero-based composition or source frame index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameIndex(pub u32);

/// Frame count (duration expressed as a number of frames at a known rate).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameCount(pub u32);

/// Rational frame rate `num / den` frames per second.
///
/// Integer fps is `RationalFrameRate::integer(n)`. Non-integer broadcast rates
/// (e.g. 30000/1001) keep exact arithmetic inside core conversions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RationalFrameRate {
    pub num: u32,
    pub den: u32,
}

impl RationalFrameRate {
    pub const fn integer(fps: u32) -> Self {
        Self {
            num: if fps == 0 { 1 } else { fps },
            den: 1,
        }
    }

    /// Build a rate; zero numerator or denominator falls back to 1/1.
    pub const fn new(num: u32, den: u32) -> Self {
        if num == 0 || den == 0 {
            Self { num: 1, den: 1 }
        } else {
            Self { num, den }
        }
    }

    pub fn as_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// Approximate integer fps used by legacy `FrameCtx.fps` surfaces.
    pub fn as_u32_fps(self) -> u32 {
        let v = self.as_f64().round();
        if v < 1.0 {
            1
        } else if v >= u32::MAX as f64 {
            u32::MAX
        } else {
            v as u32
        }
    }
}

impl From<u32> for RationalFrameRate {
    fn from(fps: u32) -> Self {
        Self::integer(fps)
    }
}

impl Default for RationalFrameRate {
    fn default() -> Self {
        Self::integer(30)
    }
}

/// Absolute media or composition timestamp in microseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimestampMicros(pub u64);

impl TimestampMicros {
    pub const ZERO: Self = Self(0);

    pub fn saturating_add(self, duration: DurationMicros) -> Self {
        Self(self.0.saturating_add(duration.0))
    }

    pub fn saturating_sub(self, duration: DurationMicros) -> Self {
        Self(self.0.saturating_sub(duration.0))
    }
}

/// Non-negative duration in microseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DurationMicros(pub u64);

impl DurationMicros {
    pub const ZERO: Self = Self(0);

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Half-open timeline range: `[start, start + duration)` when `duration` is
/// `Some`; open-ended from `start` when `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DurationRange {
    pub start: TimestampMicros,
    pub duration: Option<DurationMicros>,
}

impl DurationRange {
    pub fn from_start(start: TimestampMicros) -> Self {
        Self {
            start,
            duration: None,
        }
    }

    pub fn with_duration(start: TimestampMicros, duration: DurationMicros) -> Self {
        Self {
            start,
            duration: Some(duration),
        }
    }

    /// Exclusive end when a finite duration is set.
    pub fn end(self) -> Option<TimestampMicros> {
        self.duration.map(|d| self.start.saturating_add(d))
    }

    /// Whether `t` falls inside this range (open-ended when duration is None).
    pub fn contains(self, t: TimestampMicros) -> bool {
        if t.0 < self.start.0 {
            return false;
        }
        match self.duration {
            Some(d) if d.0 > 0 => t.0 < self.start.0.saturating_add(d.0),
            Some(_) => false,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rational_integer_and_broadcast() {
        assert_eq!(RationalFrameRate::integer(30).as_f64(), 30.0);
        let ntsc = RationalFrameRate::new(30_000, 1_001);
        assert!((ntsc.as_f64() - 29.970_029_97).abs() < 1e-6);
        assert_eq!(RationalFrameRate::new(0, 0).num, 1);
    }

    #[test]
    fn duration_range_contains_half_open() {
        let range =
            DurationRange::with_duration(TimestampMicros(1_000_000), DurationMicros(500_000));
        assert!(range.contains(TimestampMicros(1_000_000)));
        assert!(range.contains(TimestampMicros(1_499_999)));
        assert!(!range.contains(TimestampMicros(1_500_000)));
        assert!(!range.contains(TimestampMicros(999_999)));
        assert_eq!(range.end(), Some(TimestampMicros(1_500_000)));
    }

    #[test]
    fn open_ended_range() {
        let range = DurationRange::from_start(TimestampMicros(10));
        assert!(range.contains(TimestampMicros(10)));
        assert!(range.contains(TimestampMicros(u64::MAX)));
        assert!(!range.contains(TimestampMicros(9)));
    }
}
