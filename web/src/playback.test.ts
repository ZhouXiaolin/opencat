import { describe, expect, test } from 'vitest';
import {
  activeSegmentsAt,
  audioPlaybackWindow,
  playbackPosition,
  segmentExportSlice,
  segmentPlaybackAt,
} from './playback';
import type { AudioPlan } from 'opencat.js';

describe('playback position', () => {
  test('wraps frames and increments loop index after one full pass', () => {
    expect(playbackPosition(0, 2, 30, 60)).toEqual({
      frame: 0,
      loopIndex: 1,
    });
  });

  test('tracks loops when playback starts from the middle', () => {
    expect(playbackPosition(45, 0.5, 30, 60)).toEqual({
      frame: 0,
      loopIndex: 1,
    });
  });

  test('computes the audio offset and duration from the current frame', () => {
    expect(audioPlaybackWindow(15, 30, 60)).toEqual({
      offsetSecs: 0.5,
      durationSecs: 1.5,
    });
  });
});

describe('audio plan segment windows', () => {
  const scenePlan: AudioPlan = {
    segments: [
      {
        assetId: 'audio:url:a.mp3',
        startMicros: 0,
        endMicros: 333_333,
        durationMicros: 333_333,
      },
      {
        assetId: 'audio:url:b.mp3',
        startMicros: 500_000,
        endMicros: 1_166_667,
        durationMicros: 666_667,
      },
    ],
  };

  test('segmentPlaybackAt returns source offset inside active scene', () => {
    // Play from composition 0.1s: still inside scene-a [0, 0.333)
    expect(segmentPlaybackAt(scenePlan.segments[0], 0.1, 2)).toEqual({
      offsetSecs: 0.1,
      durationSecs: expect.closeTo(0.233333, 5),
    });
  });

  test('activeSegmentsAt only schedules overlapping clips', () => {
    // At composition 0.6s only scene-b is active
    const active = activeSegmentsAt(scenePlan, 0.6, 2);
    expect(active).toHaveLength(1);
    expect(active[0].assetId).toBe('audio:url:b.mp3');
    expect(active[0].window.offsetSecs).toBeCloseTo(0.1, 5);
  });

  test('export slice maps composition time onto source offset', () => {
    // Frame at 0.55s (during scene-b): source offset = 0.05s
    const slice = segmentExportSlice(scenePlan.segments[1], 0.55, 1 / 30);
    expect(slice).not.toBeNull();
    expect(slice!.offsetSecs).toBeCloseTo(0.05, 5);
    expect(slice!.durationSecs).toBeCloseTo(1 / 30, 5);
  });

  test('export slice is null outside segment', () => {
    expect(segmentExportSlice(scenePlan.segments[0], 0.5, 0.1)).toBeNull();
  });
});
