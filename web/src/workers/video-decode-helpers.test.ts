import { describe, expect, test } from 'vitest';
import {
  chunkIdxAtTime,
  decodeSliceEndIndex,
  type EncodedChunkDesc,
  nearestKeyframeBefore,
  previousKeyframeBefore,
  seekFeedMarginUs,
} from './video-decode-helpers';

describe('nearestKeyframeBefore', () => {
  test('returns clamped anchor for target inside a span', () => {
    const keys = [0, 500_000, 1_200_000, 2_400_000];
    expect(nearestKeyframeBefore(keys, 100_000)).toBe(0);
    expect(nearestKeyframeBefore(keys, 1_800_000)).toBe(1_200_000);
    expect(nearestKeyframeBefore(keys, 3_000_000)).toBe(2_400_000);
  });

  test('returns 0 when target equals or is before first keyframe', () => {
    const keys = [0, 1_000_000];
    expect(nearestKeyframeBefore(keys, 0)).toBe(0);
    expect(nearestKeyframeBefore(keys, -500_000)).toBe(0);
  });

  test('returns target when keyframe list is empty', () => {
    expect(nearestKeyframeBefore([], 1_000_000)).toBe(1_000_000);
    expect(nearestKeyframeBefore([], -100)).toBe(0);
  });

  test('exact match returns that keyframe', () => {
    expect(nearestKeyframeBefore([0, 1_000_000, 2_000_000], 1_000_000)).toBe(1_000_000);
  });
});

describe('previousKeyframeBefore', () => {
  test('returns strictly-smaller keyframe', () => {
    const keys = [0, 500_000, 1_200_000, 2_400_000];
    expect(previousKeyframeBefore(keys, 1_200_000)).toBe(500_000);
    expect(previousKeyframeBefore(keys, 500_000)).toBe(0);
  });

  test('returns -1 when target is at or before first keyframe', () => {
    expect(previousKeyframeBefore([0, 1_000_000], 0)).toBe(-1);
    expect(previousKeyframeBefore([100_000, 500_000], 50_000)).toBe(-1);
  });

  test('returns -1 for empty list', () => {
    expect(previousKeyframeBefore([], 1_000_000)).toBe(-1);
  });
});

describe('seekFeedMarginUs', () => {
  test('uses bounded decode margins for preview and exact export', () => {
    expect(seekFeedMarginUs('scrubbing')).toBe(120_000);
    expect(seekFeedMarginUs('realtime')).toBe(350_000);
    expect(seekFeedMarginUs('exact')).toBe(500_000);
  });
});

function makeChunks(times: number[]): EncodedChunkDesc[] {
  return times.map((t, i) => ({
    type: i === 0 ? 'key' : 'delta',
    timestamp: t,
    duration: 33_333,
    data: new ArrayBuffer(1),
  }));
}

describe('chunkIdxAtTime', () => {
  test('returns the index whose timestamp matches', () => {
    const chunks = makeChunks([0, 33_333, 66_666, 100_000]);
    expect(chunkIdxAtTime(chunks, 33_333)).toBe(1);
    expect(chunkIdxAtTime(chunks, 0)).toBe(0);
    expect(chunkIdxAtTime(chunks, 100_000)).toBe(3);
  });

  test('returns first matching index when duplicates exist', () => {
    const chunks = makeChunks([0, 100, 100, 200]);
    expect(chunkIdxAtTime(chunks, 100)).toBe(1);
  });

  test('works when packet order is DTS order and presentation timestamps are not sorted', () => {
    const chunks = makeChunks([
      0,
      66_666,
      33_333,
      133_333,
      100_000,
    ]);
    expect(chunkIdxAtTime(chunks, 33_333)).toBe(2);
    expect(chunkIdxAtTime(chunks, 100_000)).toBe(4);
  });

  test('returns -1 when no exact match', () => {
    const chunks = makeChunks([0, 100, 200]);
    expect(chunkIdxAtTime(chunks, 50)).toBe(-1);
    expect(chunkIdxAtTime(chunks, 300)).toBe(-1);
  });

  test('returns -1 for empty list', () => {
    expect(chunkIdxAtTime([], 100)).toBe(-1);
  });
});

describe('decodeSliceEndIndex', () => {
  test('keeps a small decode-order lookahead after presentation timestamps cover the target', () => {
    const chunks = makeChunks([
      0,
      66_666,
      33_333,
      133_333,
      100_000,
      3_500_000,
      166_666,
      200_000,
      3_600_000,
    ]);

    expect(decodeSliceEndIndex(chunks, 0, 3_000_000, 500_000)).toBe(8);
  });

  test('can stop immediately after target coverage when lookahead is disabled', () => {
    const chunks = makeChunks([
      0,
      66_666,
      33_333,
      133_333,
      100_000,
      3_500_000,
      166_666,
      200_000,
      3_600_000,
    ]);

    expect(decodeSliceEndIndex(chunks, 0, 3_000_000, 500_000, 0)).toBe(5);
  });

  test('stops at eof when the target plus margin is never covered', () => {
    const chunks = makeChunks([0, 33_333, 66_666]);

    expect(decodeSliceEndIndex(chunks, 0, 3_000_000, 500_000)).toBe(2);
  });
});
