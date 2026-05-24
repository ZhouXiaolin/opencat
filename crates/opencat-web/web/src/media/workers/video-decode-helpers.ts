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

export function seekFeedMarginUs(quality: VideoPreviewQuality): number {
  switch (quality) {
    case 'scrubbing':
      return 120_000;
    case 'realtime':
      return 1_000_000;
    case 'exact':
      return 500_000;
  }
}

export function shouldCacheDecodedFrame(
  frameUs: number,
  targetUs: number,
  coverUs: number,
  behindUs: number,
  toleranceUs = 50_000,
): boolean {
  const lowerUs = Math.max(0, targetUs - Math.max(0, behindUs));
  const upperUs = Math.max(targetUs, coverUs);
  return (
    Math.abs(frameUs - targetUs) <= Math.max(0, toleranceUs) ||
    (frameUs >= lowerUs && frameUs <= upperUs)
  );
}

export function realtimeCacheWindowUs(
  targetUs: number,
  frameDurationUs: number,
  behindFrames: number,
  aheadFrames: number,
): { minUs: number; maxUs: number } {
  const frameUs = Math.max(1, Math.round(frameDurationUs));
  return {
    minUs: Math.max(0, Math.round(targetUs) - Math.max(0, behindFrames) * frameUs),
    maxUs: Math.round(targetUs) + Math.max(0, aheadFrames) * frameUs,
  };
}

export function shouldStartRealtimePump(
  aheadFrames: number,
  lowWaterFrames: number,
  highWaterFrames: number,
  force = false,
): boolean {
  const ahead = Math.max(0, Math.floor(aheadFrames));
  const high = Math.max(0, Math.floor(highWaterFrames));
  if (ahead >= high) return false;
  if (force) return true;
  return ahead < Math.max(0, Math.floor(lowWaterFrames));
}

/** Chunk descriptor — sequencing format used internally by the worker. */
export interface EncodedChunkDesc {
  type: 'key' | 'delta';
  timestamp: number;  // μs
  duration: number;   // μs
  data: ArrayBuffer;
}

/** Find the smallest decode-order index `i` such that `chunks[i].timestamp === targetUs`.
 *  WebDemuxer yields H.264 packets in decode/DTS order; B-frames make their
 *  presentation timestamps non-monotonic, so binary search on timestamp would
 *  skip valid chunks. */
export function chunkIdxAtTime(
  chunks: readonly EncodedChunkDesc[],
  targetUs: number,
): number {
  for (let i = 0; i < chunks.length; i++) {
    if (chunks[i].timestamp === targetUs) return i;
  }
  return -1;
}

/** Last decode-order chunk index needed to cover `targetUs + marginUs`.
 *
 *  web-demuxer yields packets in decode/DTS order, while `timestamp` is the
 *  presentation timestamp. With B-frames, a packet can jump far ahead in PTS
 *  before later decode-order packets fill the earlier presentation gap. Track
 *  the maximum presentation timestamp observed instead of stopping on the
 *  first timestamp greater than the target. */
export function decodeSliceEndIndex(
  chunks: readonly EncodedChunkDesc[],
  startIdx: number,
  targetUs: number,
  marginUs: number,
  lookaheadChunks = 8,
): number {
  if (chunks.length === 0) return -1;
  const start = Math.max(0, Math.min(startIdx, chunks.length - 1));
  const stopPtsUs = targetUs + Math.max(0, marginUs);
  let endIdx = start;
  let maxSeenPtsUs = Number.NEGATIVE_INFINITY;
  let coveredAtIdx = -1;

  for (let i = start; i < chunks.length; i++) {
    endIdx = i;
    maxSeenPtsUs = Math.max(maxSeenPtsUs, chunks[i].timestamp);
    if (i > start && maxSeenPtsUs >= stopPtsUs && coveredAtIdx < 0) {
      coveredAtIdx = i;
    }
    if (
      coveredAtIdx >= 0 &&
      i - coveredAtIdx >= Math.max(0, lookaheadChunks)
    ) {
      break;
    }
  }

  return endIdx;
}

/** Build a fresh `EncodedVideoChunk` from our descriptor.
 *  Slices the buffer to dodge detached-ArrayBuffer issues across decoders. */
export function encodedChunkFrom(chunk: EncodedChunkDesc): EncodedVideoChunk {
  return new EncodedVideoChunk({
    type: chunk.type,
    timestamp: chunk.timestamp,
    duration: chunk.duration,
    data: chunk.data.slice(0),
  });
}
