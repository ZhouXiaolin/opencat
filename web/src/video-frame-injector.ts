import type { WebRendererInstance } from './wasm';
import {
  getDecodedFrameRgba,
  type VideoPreviewQuality,
} from './video-decoder';

interface VideoFramePlanItem {
  assetId: string;
  localTimeSecs: number;
}

interface InjectVideoFramesOptions {
  renderer: WebRendererInstance;
  jsonlContent: string;
  frame: number;
  resourcesJson: string;
  quality: VideoPreviewQuality;
  logPrefix: 'render' | 'export';
  logEveryFrames?: number;
}

const SLOW_DECODE_WARN_MS = 500;

function fmtMs(ms: number): string {
  return `${ms.toFixed(1)}ms`;
}

export async function injectVideoFramesForRender({
  renderer,
  jsonlContent,
  frame,
  resourcesJson,
  quality,
  logPrefix,
  logEveryFrames,
}: InjectVideoFramesOptions): Promise<void> {
  const shouldLog = logPrefix === 'export' ||
    Boolean(logEveryFrames && frame % logEveryFrames === 0);
  const injectStartAt = performance.now();

  renderer.clear_video_cache('');

  let plan: VideoFramePlanItem[] = [];
  try {
    plan = JSON.parse(renderer.plan_video_frames(jsonlContent, frame, resourcesJson));
  } catch (err) {
    console.warn(`[${logPrefix}] plan_video_frames failed:`, err);
  }

  if (plan.length > 0 && shouldLog) {
    console.log(
      `[${logPrefix} f=${frame}] plan:`,
      plan.map(p => `${p.assetId}@${p.localTimeSecs.toFixed(3)}s`).join(', '),
    );
  }

  const byAsset = new Map<string, VideoFramePlanItem>();
  for (const item of plan) {
    const existing = byAsset.get(item.assetId);
    if (existing && Math.abs(existing.localTimeSecs - item.localTimeSecs) > 1e-6) {
      console.warn(
        `[${logPrefix} f=${frame}] duplicate video asset ${item.assetId} has multiple local times; using the last one`,
      );
    }
    byAsset.set(item.assetId, item);
  }

  if (byAsset.size === 0) {
    if (shouldLog && plan.length > 0) {
      console.log(
        `[${logPrefix} f=${frame}] inject skipped: plan=${plan.length} unique=0 dt=${fmtMs(performance.now() - injectStartAt)}`,
      );
    }
    return;
  }

  if (shouldLog) {
    console.log(
      `[${logPrefix} f=${frame}] inject start unique=${byAsset.size} plan=${plan.length} q=${quality}`,
    );
  }

  await Promise.all(
    Array.from(byAsset.values()).map(async (item) => {
      const decodeStartAt = performance.now();
      if (shouldLog) {
        console.log(
          `[${logPrefix} f=${frame}] decode start asset=${item.assetId} t=${item.localTimeSecs.toFixed(3)}s q=${quality}`,
        );
      }

      try {
        const decoded = await getDecodedFrameRgba(
          item.assetId,
          item.localTimeSecs,
          quality,
        );
        const decodeMs = performance.now() - decodeStartAt;
        if (!decoded) {
          console.warn(
            `[${logPrefix} f=${frame}] decode NULL: asset=${item.assetId} t=${item.localTimeSecs.toFixed(3)}s dt=${fmtMs(decodeMs)}`,
          );
          return;
        }
        const log = decodeMs >= SLOW_DECODE_WARN_MS ? console.warn : console.log;
        if (shouldLog || decodeMs >= SLOW_DECODE_WARN_MS) {
          log(
            `[${logPrefix} f=${frame}] decode done asset=${item.assetId} t=${item.localTimeSecs.toFixed(3)}s dt=${fmtMs(decodeMs)} size=${decoded.width}x${decoded.height} bytes=${decoded.rgba.byteLength}`,
          );
        }

        const injectFrameStartAt = performance.now();
        renderer.inject_video_frame(
          item.assetId,
          frame,
          decoded.rgba,
          decoded.width,
          decoded.height,
        );
        const injectFrameMs = performance.now() - injectFrameStartAt;
        if (shouldLog || injectFrameMs >= 16) {
          console.log(
            `[${logPrefix} f=${frame}] frame injected asset=${item.assetId} dt=${fmtMs(injectFrameMs)}`,
          );
        }
      } catch (err) {
        const decodeMs = performance.now() - decodeStartAt;
        console.warn(
          `[${logPrefix} f=${frame}] decode failed asset=${item.assetId} t=${item.localTimeSecs.toFixed(3)}s dt=${fmtMs(decodeMs)}:`,
          err,
        );
      }
    }),
  );

  if (shouldLog) {
    console.log(
      `[${logPrefix} f=${frame}] inject done dt=${fmtMs(performance.now() - injectStartAt)}`,
    );
  }
}
