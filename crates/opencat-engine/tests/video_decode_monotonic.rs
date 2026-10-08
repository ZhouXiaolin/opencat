//! Regression tests for sequential (monotonic-target) video decoding.
//!
//! The codex-five-hour alignment run froze its tail (composition frames
//! ~597-608 stuck on one source frame) because (a) a request target that
//! lagged the displayed frame's pts triggered a backward seek, and (b) once
//! the packet iterator was exhausted, `decode_forward` short-circuited on
//! `eof` and never drained frames still queued in the frame-threaded
//! decoder. These tests replay the exact per-frame target sequence of that
//! composition against the same asset and assert the displayed-frame
//! sequence expected from browser `<video>` semantics: the last source frame
//! whose pts is <= the target (floor).

use std::collections::HashMap;
use std::path::PathBuf;

use opencat_engine::media::{VideoDecodeCache, VideoPreviewQuality};

fn fig_asset() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/codex-fig.mkv")
}

/// Asset frame count of `assets/codex-fig.mkv` (24 fps FFV1, all frames
/// keyframes, pts quantized to the container's 1/1000 time_base).
const ASSET_FRAMES: u64 = 91;

/// Container pts (seconds) of 1-based asset frame `k`, matching the mkv's
/// millisecond rounding: `round((k-1)/24 * 1000) / 1000`.
fn pts_secs(frame: u64) -> f64 {
    let ms = ((frame - 1) * 1_000 + 12) / 24; // round-half-up to whole ms
    ms as f64 / 1_000.0
}

/// Expected displayed asset frame for a media-local target under browser
/// `<video>` semantics: the last frame with pts <= target (floor), clamped to
/// the first frame when the target precedes it.
fn expected_frame(target: f64) -> u64 {
    (1..=ASSET_FRAMES)
        .rev()
        .find(|&k| pts_secs(k) <= target + 1e-9)
        .unwrap_or(1)
}

#[test]
fn full_composition_replay_never_freezes() {
    let path = fig_asset();
    let fig_data_start = 6.7783_f64;

    // Byte anchor for every source frame: decode it through a fresh cache
    // with a single exact request at that frame's own pts. Every frame is a
    // keyframe (FFV1), so the seek lands directly on it and floor semantics
    // select exactly B_k. Comparing against the expected frame's anchor is
    // robust to static content where adjacent source frames are
    // byte-identical (a byte "did it change?" check would false-fail there),
    // while a genuine freeze on the wrong source frame still differs.
    let mut refs: HashMap<u64, Vec<u8>> = HashMap::new();
    for k in 1..=ASSET_FRAMES {
        let mut reference = VideoDecodeCache::new();
        let frame = reference
            .get_frame(&path, pts_secs(k), VideoPreviewQuality::Realtime, None)
            .expect("reference decode should succeed");
        assert_eq!(
            expected_frame(pts_secs(k)),
            k,
            "anchor target for B{k} must select B{k}"
        );
        refs.insert(k, frame.as_ref().clone());
    }

    // Replay every fig composition frame of the codex-five run (f407..f608,
    // one get_frame per rendered frame, Realtime = the render default).
    let mut cache = VideoDecodeCache::new();
    for composition_frame in 407..=608_u32 {
        let target = composition_frame as f64 / 60.0 - fig_data_start;
        let frame = cache
            .get_frame(&path, target, VideoPreviewQuality::Realtime, None)
            .expect("decode should succeed");
        let expected_b = expected_frame(target);
        let anchor = refs
            .get(&expected_b)
            .expect("anchor for every source frame should exist");
        assert_eq!(
            frame.as_ref(),
            anchor,
            "f{composition_frame}: displayed frame must be source frame B{expected_b}"
        );
    }
}
