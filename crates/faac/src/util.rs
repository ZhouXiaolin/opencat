use crate::codec::*;

pub fn get_sr_index(sample_rate: u32) -> usize {
    if sample_rate >= 92017 { return 0; }
    if sample_rate >= 75132 { return 1; }
    if sample_rate >= 55426 { return 2; }
    if sample_rate >= 46009 { return 3; }
    if sample_rate >= 37566 { return 4; }
    if sample_rate >= 27713 { return 5; }
    if sample_rate >= 23004 { return 6; }
    if sample_rate >= 18783 { return 7; }
    if sample_rate >= 13856 { return 8; }
    if sample_rate >= 11502 { return 9; }
    if sample_rate >= 9391 { return 10; }
    11
}

pub fn max_bitrate(sample_rate: u32) -> u64 {
    (0x2000u64 * 8 * sample_rate as u64) / FRAME_LEN as u64
}

pub fn min_bitrate() -> u64 {
    8000
}

/// C `lrint()` with default `FE_TONEAREST` rounding mode — round-to-even.
#[inline]
pub fn lrint(x: f64) -> i32 {
    x.round_ties_even() as i32
}
