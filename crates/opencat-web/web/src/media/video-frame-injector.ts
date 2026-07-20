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

/**
 * Canonical video-frame cache identity. Mirrors the core render contract: a
 * video frame is identified by `(canonical assetId, authoritative timeMicros)`,
 * never by a source `frame_index`. This keeps the injector cache and the
 * IR-decoded `ImageRef::VideoFrame` reference (which carries only `timeMicros`)
 * keyed on the same value.
 */
function videoFrameKey(assetId: string, timeMicros: bigint): string {
  return `${assetId}\0${timeMicros}`;
}

/**
 * Convert a plan item's `localTimeSecs` into the same `timeMicros` value the
 * core pipeline emits in the draw IR, so injector-populated entries match the
 * renderer's lookups. Matches `time_micros = (time_secs * 1_000_000).round()`
 * on the Rust side.
 */
function localTimeSecsToMicros(localTimeSecs: number): bigint {
  return BigInt(Math.round(localTimeSecs * 1_000_000));
}

export function getCachedVideoFrameRgba(
  assetId: string,
  timeMicros: bigint,
): CachedVideoFrameRgba | undefined {
  return decodedFrameRgbaCache.get(videoFrameKey(assetId, timeMicros));
}

export function getCachedVideoFrameSource(
  assetId: string,
  timeMicros: bigint,
): CachedVideoFrameSource | undefined {
  return decodedFrameSourceCache.get(videoFrameKey(assetId, timeMicros));
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

  // Dedupe plan items by (assetId, timeMicros). The plan_video_frames path may
  // still emit a source `frameIndex` for legacy reasons; we ignore it here and
  // key purely on the authoritative time, matching the core media contract.
  const byTime = new Map<string, VideoFramePlanItem>();
  for (const item of plan) {
    byTime.set(videoFrameKey(item.assetId, localTimeSecsToMicros(item.localTimeSecs)), item);
  }

  if (byTime.size === 0) return;

  await Promise.all(
    Array.from(byTime.values()).map(async (item) => {
      try {
        const timeMicros = localTimeSecsToMicros(item.localTimeSecs);
        if (frameOutput === 'rgba') {
          const decoded = await getDecodedFrameRgba(
            item.assetId,
            item.localTimeSecs,
            quality,
          );
          if (!decoded) return;

          decodedFrameRgbaCache.set(videoFrameKey(item.assetId, timeMicros), decoded);
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

        decodedFrameSourceCache.set(videoFrameKey(item.assetId, timeMicros), {
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

  const byTime = new Map<string, VideoFramePlanItem>();
  for (const item of plan) {
    byTime.set(videoFrameKey(item.assetId, localTimeSecsToMicros(item.localTimeSecs)), item);
  }
  if (byTime.size === 0) return;

  await Promise.all(
    Array.from(byTime.values()).map((item) => (
      prefetchDecodedVideoFrame(item.assetId, item.localTimeSecs, quality)
    )),
  );
}
