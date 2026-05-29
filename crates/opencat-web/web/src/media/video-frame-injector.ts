import type { WebRendererInstance } from '../wasm';
import {
  getDecodedFrameRgba,
  getDecodedVideoFrame,
  prefetchDecodedVideoFrame,
  type VideoPreviewQuality,
} from './video-decoder';

interface VideoFramePlanItem {
  assetId: string;
  localTimeSecs: number;
  frameIndex?: number;
}

interface InjectVideoFramesOptions {
  renderer: WebRendererInstance;
  jsonlContent: string;
  frame: number;
  resourcesJson: string;
  quality: VideoPreviewQuality;
  frameOutput?: 'source' | 'rgba';
}

interface CachedVideoFrameRgba {
  rgba: Uint8Array;
  width: number;
  height: number;
}

export interface CachedVideoFrameSource {
  source: VideoFrame;
  width: number;
  height: number;
}

const decodedFrameRgbaCache = new Map<string, CachedVideoFrameRgba>();
const decodedFrameSourceCache = new Map<string, CachedVideoFrameSource>();

function videoFrameKey(assetId: string, frame: number): string {
  return `${assetId}\0${frame}`;
}

function planItemFrameIndex(item: VideoFramePlanItem, fallbackFrame: number): number {
  return typeof item.frameIndex === 'number' ? item.frameIndex : fallbackFrame;
}

export function getCachedVideoFrameRgba(
  assetId: string,
  frame: number,
): CachedVideoFrameRgba | undefined {
  return decodedFrameRgbaCache.get(videoFrameKey(assetId, frame));
}

export function getCachedVideoFrameSource(
  assetId: string,
  frame: number,
): CachedVideoFrameSource | undefined {
  return decodedFrameSourceCache.get(videoFrameKey(assetId, frame));
}

export function clearCachedVideoFrames(assetId?: string): void {
  if (!assetId) {
    for (const cached of decodedFrameSourceCache.values()) closeFrameSource(cached);
    decodedFrameSourceCache.clear();
    decodedFrameRgbaCache.clear();
    return;
  }
  for (const [key, cached] of decodedFrameSourceCache) {
    if (!key.startsWith(`${assetId}\0`)) continue;
    closeFrameSource(cached);
    decodedFrameSourceCache.delete(key);
  }
  for (const key of decodedFrameRgbaCache.keys()) {
    if (key.startsWith(`${assetId}\0`)) decodedFrameRgbaCache.delete(key);
  }
}

function closeFrameSource(cached: CachedVideoFrameSource): void {
  try { cached.source.close(); } catch { /* ignore already-closed frames */ }
}

export async function injectVideoFramesForRender({
  renderer,
  jsonlContent,
  frame,
  resourcesJson,
  quality,
  frameOutput = 'source',
}: InjectVideoFramesOptions): Promise<void> {
  clearCachedVideoFrames();

  let plan: VideoFramePlanItem[] = [];
  try {
    plan = JSON.parse(renderer.plan_video_frames(jsonlContent, frame, resourcesJson));
  } catch {
    return;
  }

  const byFrame = new Map<string, VideoFramePlanItem>();
  for (const item of plan) {
    byFrame.set(videoFrameKey(item.assetId, planItemFrameIndex(item, frame)), item);
  }

  if (byFrame.size === 0) return;

  await Promise.all(
    Array.from(byFrame.values()).map(async (item) => {
      try {
        const frameIndex = planItemFrameIndex(item, frame);
        if (frameOutput === 'rgba') {
          const decoded = await getDecodedFrameRgba(
            item.assetId,
            item.localTimeSecs,
            quality,
          );
          if (!decoded) return;

          decodedFrameRgbaCache.set(videoFrameKey(item.assetId, frameIndex), decoded);
          return;
        }

        const decoded = await getDecodedVideoFrame(
          item.assetId,
          item.localTimeSecs,
          quality,
        );
        if (!decoded) return;

        const width = decoded.displayWidth || decoded.codedWidth || 0;
        const height = decoded.displayHeight || decoded.codedHeight || 0;
        if (width <= 0 || height <= 0) {
          try { decoded.close(); } catch { /* ignore */ }
          return;
        }

        decodedFrameSourceCache.set(videoFrameKey(item.assetId, frameIndex), {
          source: decoded,
          width,
          height,
        });
      } catch {
        return;
      }
    }),
  );
}

export async function prefetchVideoFramesForRender({
  renderer,
  jsonlContent,
  frame,
  resourcesJson,
  quality,
}: InjectVideoFramesOptions): Promise<void> {
  let plan: VideoFramePlanItem[] = [];
  try {
    plan = JSON.parse(renderer.plan_video_frames(jsonlContent, frame, resourcesJson));
  } catch {
    return;
  }

  const byFrame = new Map<string, VideoFramePlanItem>();
  for (const item of plan) {
    byFrame.set(videoFrameKey(item.assetId, planItemFrameIndex(item, frame)), item);
  }
  if (byFrame.size === 0) return;

  await Promise.all(
    Array.from(byFrame.values()).map((item) => (
      prefetchDecodedVideoFrame(item.assetId, item.localTimeSecs, quality)
    )),
  );
}
