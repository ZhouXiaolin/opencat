import { describe, expect, test } from 'vitest';
import {
  exportMp4,
  exportPngFrame,
  getDecodedFrameRgba,
  initWasm,
  type CompositionInfo,
  type ResourceMeta,
} from 'opencat-web';

describe('opencat-web browser API', () => {
  test('exposes wasm, decode, and encode entry points from the package facade', () => {
    const comp: CompositionInfo = {
      width: 1920,
      height: 1080,
      fps: 30,
      frames: 60,
    };
    const video: ResourceMeta = {
      kind: 'video',
      width: 1920,
      height: 1080,
      durationSecs: 2,
    };

    expect(comp.frames).toBe(60);
    expect(video.kind).toBe('video');
    expect(typeof initWasm).toBe('function');
    expect(typeof getDecodedFrameRgba).toBe('function');
    expect(typeof exportMp4).toBe('function');
    expect(typeof exportPngFrame).toBe('function');
  });
});
