import { describe, expect, test } from 'vitest';
import {
  exportMp4,
  exportPngFrame,
  getDecodedFrameRgba,
  initWasm,
  renderEncodedDrawFrame,
  type CompositionInfo,
  type EncodedDrawFrame,
  type ResourceMeta,
  type WebRendererInstance,
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
    expect(typeof renderEncodedDrawFrame).toBe('function');
  });

  test('models wasm drawop IR as the web renderer boundary', () => {
    const frame: EncodedDrawFrame = new Uint8Array([
      0x4f, 0x43, 0x49, 0x52,
      1, 0, 0, 0,
    ]);

    expect(frame).toBeInstanceOf(Uint8Array);

    const renderer = {
      build_frame_ir: (_jsonl: string, _frame: number, _resources: string) => frame,
    } satisfies Pick<WebRendererInstance, 'build_frame_ir'>;

    expect(renderer.build_frame_ir('', 0, '{}')).toBe(frame);
  });
});
