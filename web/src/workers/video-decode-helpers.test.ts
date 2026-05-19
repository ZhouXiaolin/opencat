import { describe, expect, test } from 'vitest';
import {
  nearestKeyframeBefore,
  previousKeyframeBefore,
  seekThresholdUs,
  shouldSeekToTarget,
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

describe('seekThresholdUs', () => {
  test('matches engine thresholds', () => {
    expect(seekThresholdUs('scrubbing')).toBe(120_000);
    expect(seekThresholdUs('realtime')).toBe(350_000);
    expect(seekThresholdUs('exact')).toBe(1_500_000);
  });
});

describe('shouldSeekToTarget', () => {
  test('always seeks when no frame yet', () => {
    expect(shouldSeekToTarget(false, -1, 1_000_000, 'realtime')).toBe(true);
    expect(shouldSeekToTarget(false, 5_000_000, 1_000_000, 'realtime')).toBe(true);
  });

  test('seeks on backward jump', () => {
    expect(shouldSeekToTarget(true, 2_000_000, 1_000_000, 'realtime')).toBe(true);
    expect(shouldSeekToTarget(true, 2_000_000, 1_999_999, 'realtime')).toBe(true);
  });

  test('does not seek for forward delta within threshold', () => {
    expect(shouldSeekToTarget(true, 2_000_000, 2_300_000, 'realtime')).toBe(false);
    expect(shouldSeekToTarget(true, 2_000_000, 2_110_000, 'scrubbing')).toBe(false);
  });

  test('seeks for forward delta beyond threshold', () => {
    expect(shouldSeekToTarget(true, 2_000_000, 2_400_000, 'realtime')).toBe(true);
    expect(shouldSeekToTarget(true, 2_000_000, 2_125_000, 'scrubbing')).toBe(true);
    expect(shouldSeekToTarget(true, 2_000_000, 3_400_000, 'exact')).toBe(false);
    expect(shouldSeekToTarget(true, 2_000_000, 3_600_000, 'exact')).toBe(true);
  });
});
