//! Regression tests for sequential (monotonic-target) video decoding.
//!
//! The codex-five-hour alignment run froze its tail (composition frames
//! ~597-608 stuck on one source frame) because (a) a request target that
//! lagged the displayed frame's pts triggered a backward seek, and (b) once
//! the packet iterator was exhausted, `decode_forward` short-circuited on
//! `eof` and never drained frames still queued in the frame-threaded
//! decoder. These tests replay the exact per-frame target sequence of that
//! composition against the same asset and assert the displayed-frame
//! sequence expected from "first pts >= target" semantics.

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

/// Expected displayed asset frame for a media-local target under the
/// engine's "first frame with pts >= target" semantics.
fn expected_frame(target: f64) -> u64 {
    (1..=ASSET_FRAMES)
        .find(|&k| pts_secs(k) + 1e-9 >= target)
        .unwrap_or(ASSET_FRAMES)
}

#[test]
fn full_composition_replay_never_freezes() {
    let path = fig_asset();
    let fig_data_start = 6.7783_f64;

    // Anchor frames decoded via a fresh cache with a single exact request
    // (deterministic seek path), used as byte-exact references.
    let anchor_targets = [3.125_000_f64, 3.167_000, 3.208_000, 3.333_000, 3.375_000];
    let mut reference = VideoDecodeCache::new();
    let mut refs: HashMap<u64, Vec<u8>> = HashMap::new();
    for target in anchor_targets {
        let frame = reference
            .get_frame(&path, target, VideoPreviewQuality::Realtime, None)
            .expect("reference decode should succeed");
        refs.insert(expected_frame(target), frame.as_ref().clone());
    }

    // Replay every fig composition frame of the codex-five run (f407..f608,
    // one get_frame per rendered frame, Realtime = the render default).
    let mut cache = VideoDecodeCache::new();
    let mut prev: Option<(u64, Vec<u8>)> = None;
    for composition_frame in 407..=608_u32 {
        let target = composition_frame as f64 / 60.0 - fig_data_start;
        let frame = cache
            .get_frame(&path, target, VideoPreviewQuality::Realtime, None)
            .expect("decode should succeed");
        let expected_b = expected_frame(target);

        if let Some((prev_expected, prev_bytes)) = &prev {
            if expected_b == *prev_expected {
                assert_eq!(
                    frame.as_ref(),
                    prev_bytes,
                    "f{composition_frame}: expected same source frame B{expected_b}"
                );
            } else {
                assert_ne!(
                    frame.as_ref(),
                    prev_bytes,
                    "f{composition_frame}: decoder froze on B{prev_expected}; expected B{expected_b}"
                );
            }
        }
        if let Some(expected_bytes) = refs.get(&expected_b) {
            assert_eq!(
                frame.as_ref(),
                expected_bytes,
                "f{composition_frame}: displayed frame must be source frame B{expected_b}"
            );
        }
        prev = Some((expected_b, frame.as_ref().clone()));
    }
}
