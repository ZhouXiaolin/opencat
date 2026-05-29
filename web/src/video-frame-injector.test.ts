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

describe('video frame injector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearCachedVideoFrames();
  });

  test('caches decoded VideoFrame sources without forcing RGBA readback', async () => {
    const frameSource = {
      displayWidth: 640,
      displayHeight: 360,
      close: vi.fn(),
    } as unknown as VideoFrame;
    mockFn(getDecodedVideoFrame).mockResolvedValue(frameSource);

    const renderer = {
      plan_video_frames: () => JSON.stringify([
        { assetId: 'video:test.mp4', localTimeSecs: 1.25 },
      ]),
    };

    await injectVideoFramesForRender({
      renderer: renderer as any,
      jsonlContent: '{}',
      frame: 42,
      resourcesJson: '{}',
      quality: 'exact',
    });

    expect(getDecodedVideoFrame).toHaveBeenCalledWith('video:test.mp4', 1.25, 'exact');
    expect(getCachedVideoFrameSource('video:test.mp4', 42)?.source).toBe(frameSource);
  });

  test('uses resolved source frame index when caching same-asset video frames', async () => {
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

    const renderer = {
      plan_video_frames: () => JSON.stringify([
        { assetId: 'video:test.mp4', localTimeSecs: 1.25, frameIndex: 38 },
        { assetId: 'video:test.mp4', localTimeSecs: 12.25, frameIndex: 368 },
      ]),
    };

    await injectVideoFramesForRender({
      renderer: renderer as any,
      jsonlContent: '{}',
      frame: 42,
      resourcesJson: '{}',
      quality: 'exact',
    });

    expect(getDecodedVideoFrame).toHaveBeenCalledTimes(2);
    expect(getDecodedVideoFrame).toHaveBeenNthCalledWith(1, 'video:test.mp4', 1.25, 'exact');
    expect(getDecodedVideoFrame).toHaveBeenNthCalledWith(2, 'video:test.mp4', 12.25, 'exact');
    expect(getCachedVideoFrameSource('video:test.mp4', 38)?.source).toBe(firstFrame);
    expect(getCachedVideoFrameSource('video:test.mp4', 368)?.source).toBe(secondFrame);
    expect(getCachedVideoFrameSource('video:test.mp4', 42)).toBeUndefined();
  });

  test('can cache RGBA frames for software CanvasKit export surfaces', async () => {
    const rgbaFrame = {
      rgba: new Uint8Array([1, 2, 3, 4]),
      width: 1,
      height: 1,
    };
    mockFn(getDecodedFrameRgba).mockResolvedValue(rgbaFrame);

    const renderer = {
      plan_video_frames: () => JSON.stringify([
        { assetId: 'video:test.mp4', localTimeSecs: 1.25 },
      ]),
    };

    await injectVideoFramesForRender({
      renderer: renderer as any,
      jsonlContent: '{}',
      frame: 42,
      resourcesJson: '{}',
      quality: 'exact',
      frameOutput: 'rgba',
    });

    expect(getDecodedFrameRgba).toHaveBeenCalledWith('video:test.mp4', 1.25, 'exact');
    expect(getDecodedVideoFrame).not.toHaveBeenCalled();
    expect(getCachedVideoFrameRgba('video:test.mp4', 42)).toBe(rgbaFrame);
    expect(getCachedVideoFrameSource('video:test.mp4', 42)).toBeUndefined();
  });

  test('prefetch warms worker cache without retaining a main-thread VideoFrame', async () => {
    mockFn(prefetchDecodedVideoFrame).mockResolvedValue(undefined);
    const renderer = {
      plan_video_frames: () => JSON.stringify([
        { assetId: 'video:test.mp4', localTimeSecs: 2.5 },
      ]),
    };

    await prefetchVideoFramesForRender({
      renderer: renderer as any,
      jsonlContent: '{}',
      frame: 75,
      resourcesJson: '{}',
      quality: 'realtime',
    });

    expect(prefetchDecodedVideoFrame).toHaveBeenCalledWith('video:test.mp4', 2.5, 'realtime');
    expect(getDecodedVideoFrame).not.toHaveBeenCalled();
    expect(getCachedVideoFrameSource('video:test.mp4', 75)).toBeUndefined();
  });

  test('prefetch dedupes by resolved source frame index', async () => {
    mockFn(prefetchDecodedVideoFrame).mockResolvedValue(undefined);
    const renderer = {
      plan_video_frames: () => JSON.stringify([
        { assetId: 'video:test.mp4', localTimeSecs: 1.25, frameIndex: 38 },
        { assetId: 'video:test.mp4', localTimeSecs: 1.30, frameIndex: 38 },
        { assetId: 'video:test.mp4', localTimeSecs: 12.25, frameIndex: 368 },
      ]),
    };

    await prefetchVideoFramesForRender({
      renderer: renderer as any,
      jsonlContent: '{}',
      frame: 42,
      resourcesJson: '{}',
      quality: 'realtime',
    });

    expect(prefetchDecodedVideoFrame).toHaveBeenCalledTimes(2);
    expect(prefetchDecodedVideoFrame).toHaveBeenCalledWith(
      'video:test.mp4',
      1.30,
      'realtime',
    );
    expect(prefetchDecodedVideoFrame).toHaveBeenCalledWith(
      'video:test.mp4',
      12.25,
      'realtime',
    );
  });
});
