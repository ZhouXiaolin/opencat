import { getBlobBytes } from '../wasm';
import {
  getDecodedFrameRgba,
  getDecodedVideoFrame,
  prepareVideoSource,
  prefetchDecodedVideoFrame,
  type VideoPreviewQuality,
} from './video-decoder';

interface MediaPlanVideoFrame {
  assetId: string;
  timeMicros: number;
}

interface InjectVideoFramesOptions {
  /** Media plan JSON string from `build_frame_ir` result or `get_frame_plan`. */
  mediaPlan: string;
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
 * Convert the core media plan's authoritative `timeMicros` into seconds for the
 * video decoder API (which takes a target time in seconds).
 */
function microsToSecs(timeMicros: number): number {
  return timeMicros / 1_000_000;
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

/**
 * Feed every video asset from an `open_design` catalog into the WebCodecs
 * worker. Required before {@link injectVideoFramesForRender}: decode APIs
 * return null when `metaCache` has no entry for the assetId, which silently
 * blanks every VideoFrame draw op (see profile-showcase web oracle).
 *
 * Accepts the catalog JSON string returned by `open_design` / `openDesign`.
 */
export async function prepareCatalogVideoSources(
  catalogJson: string,
): Promise<void> {
  let catalog: Record<string, { kind?: string } | undefined> = {};
  try {
    catalog = JSON.parse(catalogJson || '{}') as Record<
      string,
      { kind?: string } | undefined
    >;
  } catch {
    return;
  }

  await Promise.all(
    Object.entries(catalog).map(async ([assetId, meta]) => {
      if (!meta || meta.kind !== 'video') return;
      const raw = getBlobBytes(assetId);
      if (!raw) return;
      try {
        await prepareVideoSource(assetId, new Uint8Array(raw).buffer);
      } catch {
        // Best-effort — missing/corrupt video should not block still frames.
      }
    }),
  );
}

export async function injectVideoFramesForRender({
  mediaPlan,
  quality,
  frameOutput = 'source',
}: InjectVideoFramesOptions): Promise<void> {
  clearCachedVideoFrames();

  // Parse the frame media plan (from build_frame_ir or get_frame_plan).
  // The plan carries the authoritative `timeMicros`, replacing the old
  // `plan_video_frames` tree walk.
  let videoFrames: MediaPlanVideoFrame[] = [];
  try {
    const plan = JSON.parse(mediaPlan);
    videoFrames = (plan.videoFrames ?? []) as MediaPlanVideoFrame[];
  } catch {
    return;
  }

  // Dedupe by (assetId, timeMicros) — the authoritative core media identity.
  const byTime = new Map<string, MediaPlanVideoFrame>();
  for (const item of videoFrames) {
    byTime.set(videoFrameKey(item.assetId, BigInt(item.timeMicros)), item);
  }

  if (byTime.size === 0) return;

  await Promise.all(
    Array.from(byTime.values()).map(async (item) => {
      try {
        const timeMicros = BigInt(item.timeMicros);
        const timeSecs = microsToSecs(item.timeMicros);
        if (frameOutput === 'rgba') {
          const decoded = await getDecodedFrameRgba(item.assetId, timeSecs, quality);
          if (!decoded) return;

          decodedFrameRgbaCache.set(videoFrameKey(item.assetId, timeMicros), decoded);
          return;
        }

        const decoded = await getDecodedVideoFrame(item.assetId, timeSecs, quality);
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
  mediaPlan,
  quality,
}: InjectVideoFramesOptions): Promise<void> {
  let videoFrames: MediaPlanVideoFrame[] = [];
  try {
    const plan = JSON.parse(mediaPlan);
    videoFrames = (plan.videoFrames ?? []) as MediaPlanVideoFrame[];
  } catch {
    return;
  }

  const byTime = new Map<string, MediaPlanVideoFrame>();
  for (const item of videoFrames) {
    byTime.set(videoFrameKey(item.assetId, BigInt(item.timeMicros)), item);
  }
  if (byTime.size === 0) return;

  await Promise.all(
    Array.from(byTime.values()).map((item) => (
      prefetchDecodedVideoFrame(item.assetId, microsToSecs(item.timeMicros), quality)
    )),
  );
}
