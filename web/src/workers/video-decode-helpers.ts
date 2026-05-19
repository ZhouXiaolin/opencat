// Pure helpers for the video-decode worker.
// No WebCodecs / DOM types — safe to import from node-env Vitest tests.
//
// Algorithms mirror opencat-engine/src/codec/decode.rs to keep web preview
// behavior aligned with native FFmpeg seek strategy.

import type { VideoPreviewQuality } from './video-decode-worker.types';

/** Largest keyframe PTS ≤ targetUs. Returns 0 (or targetUs floored at 0)
 *  when the list is empty, mirroring engine's `nearest_keyframe_before`. */
export function nearestKeyframeBefore(
  keyframeTimesUs: readonly number[],
  targetUs: number,
): number {
  if (keyframeTimesUs.length === 0) return Math.max(0, targetUs);
  const eps = 1; // 1 μs tolerance
  let lo = 0;
  let hi = keyframeTimesUs.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (keyframeTimesUs[mid] <= targetUs + eps) lo = mid + 1;
    else hi = mid;
  }
  const idx = lo - 1;
  return Math.max(0, keyframeTimesUs[Math.max(0, idx)]);
}

/** Largest keyframe PTS strictly less than targetUs. Returns -1 if none.
 *  Used for Open-GOP fallback: when feeding from a key produced 0 frames,
 *  retry with the keyframe before it. */
export function previousKeyframeBefore(
  keyframeTimesUs: readonly number[],
  targetUs: number,
): number {
  if (keyframeTimesUs.length === 0) return -1;
  let lo = 0;
  let hi = keyframeTimesUs.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (keyframeTimesUs[mid] < targetUs) lo = mid + 1;
    else hi = mid;
  }
  const idx = lo - 1;
  if (idx < 0) return -1;
  return keyframeTimesUs[idx];
}

/** Engine-aligned seek thresholds in microseconds.
 *  Source: opencat-engine/src/codec/decode.rs:226-232. */
export function seekThresholdUs(quality: VideoPreviewQuality): number {
  switch (quality) {
    case 'scrubbing':
      return 120_000; // 0.12 s
    case 'realtime':
      return 350_000; // 0.35 s
    case 'exact':
      return 1_500_000; // 1.5 s
  }
}

/** Engine-aligned should_seek decision.
 *  - hasFrame=false: always seek (cold decoder)
 *  - target < current: backward jump, seek
 *  - target - current > threshold: large forward jump, seek
 *  - otherwise: forward-decode from current cursor */
export function shouldSeekToTarget(
  hasFrame: boolean,
  currentPtsUs: number,
  targetUs: number,
  quality: VideoPreviewQuality,
): boolean {
  if (!hasFrame) return true;
  if (targetUs < currentPtsUs) return true;
  return targetUs - currentPtsUs > seekThresholdUs(quality);
}
