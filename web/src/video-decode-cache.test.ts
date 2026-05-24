import { describe, expect, test } from 'vitest';
import {
  realtimeCacheWindowUs,
  seekFeedMarginUs,
  shouldCacheDecodedFrame,
  shouldStartRealtimePump,
} from '../../crates/opencat-web/web/src/media/workers/video-decode-helpers';

describe('video decode cache window', () => {
  test('keeps target-adjacent and forward frames from a decoded slice', () => {
    const targetUs = 3_000_000;
    const coverUs = 3_350_000;
    const behindUs = 250_000;

    const kept = [
      2_700_000,
      2_760_000,
      3_000_000,
      3_100_000,
      3_340_000,
      3_420_000,
    ].filter((timestampUs) => (
      shouldCacheDecodedFrame(timestampUs, targetUs, coverUs, behindUs)
    ));

    expect(kept).toEqual([
      2_760_000,
      3_000_000,
      3_100_000,
      3_340_000,
    ]);
  });

  test('realtime lookahead covers frame skips caused by slow decode', () => {
    const targetUs = 3_000_000;
    const coverUs = targetUs + seekFeedMarginUs('realtime');

    expect(
      shouldCacheDecodedFrame(3_900_000, targetUs, coverUs, 250_000),
    ).toBe(true);
  });

  test('realtime cache window stays small and ahead of the playhead', () => {
    expect(realtimeCacheWindowUs(3_000_000, 33_333, 3, 24)).toEqual({
      minUs: 2_900_001,
      maxUs: 3_799_992,
    });
  });

  test('realtime pump respects low and high watermarks', () => {
    expect(shouldStartRealtimePump(9, 10, 24)).toBe(true);
    expect(shouldStartRealtimePump(10, 10, 24)).toBe(false);
    expect(shouldStartRealtimePump(10, 10, 24, true)).toBe(true);
    expect(shouldStartRealtimePump(24, 10, 24, true)).toBe(false);
  });
});
