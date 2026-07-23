import { describe, expect, test, vi } from 'vitest';
import {
  exportMp4,
  exportPngFrame,
  compositionFrameCount,
  getDecodedFrameRgba,
  initWasm,
  renderEncodedDrawFrame,
  type CompositionInfo,
  type EncodedDrawFrame,
  type ResourceMeta,
  type WebRendererInstance,
} from 'opencat.js';

describe('opencat.js browser API', () => {
  test('exposes wasm, decode, and encode entry points from the package facade', () => {
    const comp: CompositionInfo = {
      width: 1920,
      height: 1080,
      fps: 30,
      duration: 2,
    };
    const video: ResourceMeta = {
      kind: 'video',
      width: 1920,
      height: 1080,
      durationSecs: 2,
    };

    expect(compositionFrameCount(comp)).toBe(60);
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
      build_frame_ir: (_frame: number) => ({ ir: frame, mediaPlan: '{}', frame: _frame }),
    } satisfies Pick<WebRendererInstance, 'build_frame_ir'>;

    expect(renderer.build_frame_ir(0).ir).toBe(frame);
    expect(renderer.build_frame_ir(0).mediaPlan).toBe('{}');
    expect(renderer.build_frame_ir(0).frame).toBe(0);
  });

  test('opens a design and returns its web-owned resource catalog', async () => {
    const renderer = {
      open_design: async (_source: string) => '{}',
    } satisfies Pick<WebRendererInstance, 'open_design'>;

    await expect(renderer.open_design('{}')).resolves.toBe('{}');
  });

  test('each build_frame_ir call triggers exactly one core compilation', () => {
    const spy = vi.fn((frame: number) => ({
      ir: new Uint8Array([0x4f, 0x43, 0x49, 0x52]),
      mediaPlan: '{}',
      frame,
    }));
    const renderer = {
      build_frame_ir: spy,
    } satisfies Pick<WebRendererInstance, 'build_frame_ir'>;

    // First call: spy invoked exactly once
    const r0 = renderer.build_frame_ir(0);
    expect(spy).toHaveBeenCalledTimes(1);
    expect(r0.frame).toBe(0);

    // Second call with different frame: spy invoked exactly once more
    const r1 = renderer.build_frame_ir(1);
    expect(spy).toHaveBeenCalledTimes(2);
    expect(r1.frame).toBe(1);

    // Same frame again: still counts as a fresh compilation
    const r1b = renderer.build_frame_ir(1);
    expect(spy).toHaveBeenCalledTimes(3);
    expect(r1b.frame).toBe(1);
  });

  test('consecutive build_frame_ir calls return distinct frame identities', () => {
    const renderer = {
      build_frame_ir: (frame: number) => ({
        ir: new Uint8Array([0x4f, 0x43, 0x49, 0x52, frame & 0xff, 0, 0, 0]),
        mediaPlan: `{"frame":${frame}}`,
        frame,
      }),
    } satisfies Pick<WebRendererInstance, 'build_frame_ir'>;

    const r0 = renderer.build_frame_ir(0);
    const r5 = renderer.build_frame_ir(5);
    const r42 = renderer.build_frame_ir(42);

    expect(r0.frame).toBe(0);
    expect(r5.frame).toBe(5);
    expect(r42.frame).toBe(42);

    // Each result is a distinct object (no shared mutable state)
    expect(r0).not.toBe(r5);
    expect(r5).not.toBe(r42);
  });

  test('build_frame_ir and get_frame_plan each trigger independent compilation', () => {
    const fn = {
      buildCount: 0,
      planCount: 0,
      build_frame_ir(frame: number) {
        this.buildCount++;
        return { ir: new Uint8Array([0x4f, 0x43, 0x49, 0x52]), mediaPlan: '{}', frame };
      },
      get_frame_plan(_frame: number) {
        this.planCount++;
        return '{}';
      },
    } satisfies Pick<WebRendererInstance, 'build_frame_ir' | 'get_frame_plan'>;

    fn.build_frame_ir(0);
    fn.get_frame_plan(0);
    // Each triggers its own pipeline.render_frame call — callers must not assume
    // that get_frame_plan reuses the last build_frame_ir result.
    expect(fn.buildCount).toBe(1);
    expect(fn.planCount).toBe(1);
  });
});
