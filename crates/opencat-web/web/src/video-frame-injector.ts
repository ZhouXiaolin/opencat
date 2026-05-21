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
}

export async function injectVideoFramesForRender({
  renderer,
  jsonlContent,
  frame,
  resourcesJson,
  quality,
}: InjectVideoFramesOptions): Promise<void> {
  renderer.clear_video_cache('');

  let plan: VideoFramePlanItem[] = [];
  try {
    plan = JSON.parse(renderer.plan_video_frames(jsonlContent, frame, resourcesJson));
  } catch (err) {
    console.warn('[renderer] plan_video_frames failed:', err);
  }

  const byAsset = new Map<string, VideoFramePlanItem>();
  for (const item of plan) {
    const existing = byAsset.get(item.assetId);
    if (existing && Math.abs(existing.localTimeSecs - item.localTimeSecs) > 1e-6) {
      console.warn(
        `[renderer f=${frame}] duplicate video asset ${item.assetId} has multiple local times; using the last one`,
      );
    }
    byAsset.set(item.assetId, item);
  }

  if (byAsset.size === 0) return;

  await Promise.all(
    Array.from(byAsset.values()).map(async (item) => {
      try {
        const decoded = await getDecodedFrameRgba(
          item.assetId,
          item.localTimeSecs,
          quality,
        );
        if (!decoded) {
          console.warn(
            `[renderer f=${frame}] decode NULL: asset=${item.assetId} t=${item.localTimeSecs.toFixed(3)}s`,
          );
          return;
        }

        renderer.inject_video_frame(
          item.assetId,
          frame,
          decoded.rgba,
          decoded.width,
          decoded.height,
        );
      } catch (err) {
        console.warn(
          `[renderer f=${frame}] decode failed asset=${item.assetId} t=${item.localTimeSecs.toFixed(3)}s:`,
          err,
        );
      }
    }),
  );
}
