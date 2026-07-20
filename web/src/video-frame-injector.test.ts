import { beforeEach, describe, expect, test, vi } from 'vitest';
import {
  clearCachedVideoFrames,
  getCachedVideoFrameRgba,
  getCachedVideoFrameSource,
  injectVideoFramesForRender,
  prefetchVideoFramesForRender,
} from '../../crates/opencat-web/web/src/media/video-frame-injector';
import {
  getDecodedFrameRgba,
  getDecodedVideoFrame,
  prefetchDecodedVideoFrame,
} from '../../crates/opencat-web/web/src/media/video-decoder';

vi.mock('../../crates/opencat-web/web/src/media/video-decoder', () => ({
  getDecodedFrameRgba: vi.fn(),
  getDecodedVideoFrame: vi.fn(),
  prefetchDecodedVideoFrame: vi.fn(),
}));

function mockFn<T extends (...args: any[]) => any>(fn: T) {
  return fn as unknown as ReturnType<typeof vi.fn>;
}

// Build a renderer mock whose `prepare_frame` returns the given video
// frames. `timeMicros` is the authoritative core media identity.
function rendererWith(videoFrames: { assetId: string; timeMicros: number }[]) {
  return {
    prepare_frame: () =>
      JSON.stringify({ videoFrames }),
  };
}

describe('video frame injector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearCachedVideoFrames();
  });

  test('prepares the core frame once before decoding its media plan', async () => {
    const frameSource = {
      displayWidth: 640,
      displayHeight: 360,
      close: vi.fn(),
    } as unknown as VideoFrame;
    mockFn(getDecodedVideoFrame).mockResolvedValue(frameSource);
    const renderer = {
      prepare_frame: vi.fn(() => JSON.stringify({
        videoFrames: [{ assetId: 'video:test.mp4', timeMicros: 1_250_000 }],
      })),
    };

    await injectVideoFramesForRender({
      renderer: renderer as any,
      frame: 42,
      quality: 'exact',
    });

    expect(renderer.prepare_frame).toHaveBeenCalledOnce();
    expect(renderer.prepare_frame).toHaveBeenCalledWith(42);
    expect(getDecodedVideoFrame).toHaveBeenCalledWith('video:test.mp4', 1.25, 'exact');
  });

  test('caches decoded VideoFrame sources without forcing RGBA readback', async () => {
    const frameSource = {
      displayWidth: 640,
      displayHeight: 360,
      close: vi.fn(),
    } as unknown as VideoFrame;
    mockFn(getDecodedVideoFrame).mockResolvedValue(frameSource);

    // 1.25s = 1_250_000 micros
    await injectVideoFramesForRender({
      renderer: rendererWith([{ assetId: 'video:test.mp4', timeMicros: 1_250_000 }]) as any,
      frame: 42,
      quality: 'exact',
    });

    expect(getDecodedVideoFrame).toHaveBeenCalledWith('video:test.mp4', 1.25, 'exact');
    expect(getCachedVideoFrameSource('video:test.mp4', 1_250_000n)?.source).toBe(frameSource);
  });

  test('caches each distinct (assetId, timeMicros) from the media plan', async () => {
    const firstFrame = {
      displayWidth: 640,
      displayHeight: 360,
      close: vi.fn(),
    } as unknown as VideoFrame;
    const secondFrame = {
      displayWidth: 640,
      displayHeight: 360,
      close: vi.fn(),
    } as unknown as VideoFrame;
    mockFn(getDecodedVideoFrame)
      .mockResolvedValueOnce(firstFrame)
      .mockResolvedValueOnce(secondFrame);

    await injectVideoFramesForRender({
      renderer: rendererWith([
        { assetId: 'video:test.mp4', timeMicros: 1_250_000 },
        { assetId: 'video:test.mp4', timeMicros: 12_250_000 },
      ]) as any,
      frame: 42,
      quality: 'exact',
    });

    expect(getDecodedVideoFrame).toHaveBeenCalledTimes(2);
    expect(getDecodedVideoFrame).toHaveBeenNthCalledWith(1, 'video:test.mp4', 1.25, 'exact');
    expect(getDecodedVideoFrame).toHaveBeenNthCalledWith(2, 'video:test.mp4', 12.25, 'exact');
    expect(getCachedVideoFrameSource('video:test.mp4', 1_250_000n)?.source).toBe(firstFrame);
    expect(getCachedVideoFrameSource('video:test.mp4', 12_250_000n)?.source).toBe(secondFrame);
  });

  test('can cache RGBA frames for software CanvasKit export surfaces', async () => {
    const rgbaFrame = {
      rgba: new Uint8Array([1, 2, 3, 4]),
      width: 1,
      height: 1,
    };
    mockFn(getDecodedFrameRgba).mockResolvedValue(rgbaFrame);

    await injectVideoFramesForRender({
      renderer: rendererWith([{ assetId: 'video:test.mp4', timeMicros: 1_250_000 }]) as any,
      frame: 42,
      quality: 'exact',
      frameOutput: 'rgba',
    });

    expect(getDecodedFrameRgba).toHaveBeenCalledWith('video:test.mp4', 1.25, 'exact');
    expect(getDecodedVideoFrame).not.toHaveBeenCalled();
    expect(getCachedVideoFrameRgba('video:test.mp4', 1_250_000n)).toBe(rgbaFrame);
    expect(getCachedVideoFrameSource('video:test.mp4', 1_250_000n)).toBeUndefined();
  });

  test('prefetch warms worker cache without retaining a main-thread VideoFrame', async () => {
    mockFn(prefetchDecodedVideoFrame).mockResolvedValue(undefined);

    await prefetchVideoFramesForRender({
      renderer: rendererWith([{ assetId: 'video:test.mp4', timeMicros: 2_500_000 }]) as any,
      frame: 75,
      quality: 'realtime',
    });

    expect(prefetchDecodedVideoFrame).toHaveBeenCalledWith('video:test.mp4', 2.5, 'realtime');
    expect(getDecodedVideoFrame).not.toHaveBeenCalled();
    expect(getCachedVideoFrameSource('video:test.mp4', 2_500_000n)).toBeUndefined();
  });

  test('prefetch dedupes by (assetId, timeMicros)', async () => {
    mockFn(prefetchDecodedVideoFrame).mockResolvedValue(undefined);

    await prefetchVideoFramesForRender({
      renderer: rendererWith([
        { assetId: 'video:test.mp4', timeMicros: 1_250_000 },
        { assetId: 'video:test.mp4', timeMicros: 1_250_000 }, // dup of the first
        { assetId: 'video:test.mp4', timeMicros: 12_250_000 },
      ]) as any,
      frame: 42,
      quality: 'realtime',
    });

    expect(prefetchDecodedVideoFrame).toHaveBeenCalledTimes(2);
    expect(prefetchDecodedVideoFrame).toHaveBeenCalledWith(
      'video:test.mp4',
      1.25,
      'realtime',
    );
    expect(prefetchDecodedVideoFrame).toHaveBeenCalledWith(
      'video:test.mp4',
      12.25,
      'realtime',
    );
  });
});
