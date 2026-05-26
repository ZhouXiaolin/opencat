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
    vi.mocked(getDecodedVideoFrame).mockResolvedValue(frameSource);

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

  test('can cache RGBA frames for software CanvasKit export surfaces', async () => {
    const rgbaFrame = {
      rgba: new Uint8Array([1, 2, 3, 4]),
      width: 1,
      height: 1,
    };
    vi.mocked(getDecodedFrameRgba).mockResolvedValue(rgbaFrame);

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
    vi.mocked(prefetchDecodedVideoFrame).mockResolvedValue(undefined);
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
});
