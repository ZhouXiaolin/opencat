import { describe, expect, test } from 'vitest';
import { audioPlaybackWindow, playbackPosition } from './playback';

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
