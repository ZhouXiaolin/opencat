// RPC client shell for the video-decode worker.
// All decoding happens in crates/opencat-web/web/src/media/workers/video-decode-worker.ts;
// this file just routes calls through postMessage.

import type {
  VideoPreviewQuality,
  VideoSourceMeta,
  WorkerRequest,
  WorkerResponse,
} from './workers/video-decode-worker.types';

export type { VideoPreviewQuality, VideoSourceMeta };

// ── State ──

let worker: Worker | null = null;
let workerBaseUrl: string | undefined;
let nextRpcId = 1;

export function setWorkerBaseUrl(url: string): void {
  workerBaseUrl = url.endsWith('/') ? url : url + '/';
}

function getWorkerUrl(): string {
  const base = workerBaseUrl || '';
  return `${base}workers/video-decode-worker.js`;
}
const pending = new Map<
  number,
  { resolve: (value: unknown) => void; reject: (err: Error) => void }
>();
const metaCache = new Map<string, VideoSourceMeta>();

function ensureWorker(): Worker {
  if (worker) return worker;
  worker = new Worker(getWorkerUrl(), { type: 'module' });
  worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
    const res = e.data;
    const handler = pending.get(res.id);
    if (!handler) return;
    pending.delete(res.id);
    if (res.type === 'error') handler.reject(new Error(res.message));
    else handler.resolve(res);
  };
  worker.onerror = (e) => {
    for (const { reject } of pending.values()) {
      reject(new Error(`worker crashed: ${e.message}`));
    }
    pending.clear();
    metaCache.clear();
    worker = null;
  };
  return worker;
}

function rpc<T extends WorkerResponse>(
  req: WorkerRequest,
  transfer: Transferable[] = [],
): Promise<T> {
  const w = ensureWorker();
  return new Promise<T>((resolve, reject) => {
    pending.set(req.id, {
      resolve: (v) => resolve(v as T),
      reject,
    });
    w.postMessage(req, transfer);
  });
}

function nextId(): number {
  return nextRpcId++;
}

// ── Public API ──

export async function prepareVideoSource(
  url: string,
  buffer: ArrayBuffer,
): Promise<VideoSourceMeta> {
  const existing = metaCache.get(url);
  if (existing) return existing;

  const id = nextId();
  const res = await rpc<{ type: 'prepare'; id: number; meta: VideoSourceMeta }>(
    { type: 'prepare', id, assetId: url, buffer },
    [buffer],
  );
  metaCache.set(url, res.meta);
  return res.meta;
}

export async function getDecodedVideoFrame(
  url: string,
  timeSecs: number,
  quality: VideoPreviewQuality = 'realtime',
): Promise<VideoFrame | null> {
  if (!metaCache.has(url)) return null;
  const id = nextId();
  const res = await rpc<{
    type: 'getFrame';
    id: number;
    frame: VideoFrame | null;
  }>({ type: 'getFrame', id, assetId: url, timeSecs, quality });
  return res.frame;
}

export async function prefetchDecodedVideoFrame(
  url: string,
  timeSecs: number,
  quality: VideoPreviewQuality = 'realtime',
): Promise<void> {
  if (!metaCache.has(url)) return;
  const id = nextId();
  await rpc<{
    type: 'prefetchFrame';
    id: number;
    ok: boolean;
  }>({ type: 'prefetchFrame', id, assetId: url, timeSecs, quality });
}

export async function getDecodedFrameRgba(
  url: string,
  timeSecs: number,
  quality: VideoPreviewQuality = 'realtime',
): Promise<{ rgba: Uint8Array; width: number; height: number } | null> {
  const meta = metaCache.get(url);
  if (!meta) return null;
  const frame = await getDecodedVideoFrame(url, timeSecs, quality);
  if (!frame) return null;

  try {
    const w = frame.displayWidth || meta.width;
    const h = frame.displayHeight || meta.height;
    const off = new OffscreenCanvas(w, h);
    const ctx = off.getContext('2d', { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(frame, 0, 0);
    const img = ctx.getImageData(0, 0, w, h);
    return {
      rgba: new Uint8Array(img.data.buffer.slice(0)),
      width: w,
      height: h,
    };
  } finally {
    try { frame.close(); } catch { /* ignore */ }
  }
}

export async function clearVideoCache(url?: string): Promise<void> {
  if (url) {
    if (!metaCache.has(url)) return;
    metaCache.delete(url);
    const id = nextId();
    await rpc<{ type: 'release'; id: number }>({
      type: 'release',
      id,
      assetId: url,
    }).catch(() => { /* swallow — release is best-effort */ });
    return;
  }
  const urls = Array.from(metaCache.keys());
  metaCache.clear();
  await Promise.all(
    urls.map((u) => {
      const id = nextId();
      return rpc<{ type: 'release'; id: number }>({
        type: 'release',
        id,
        assetId: u,
      }).catch(() => { /* ignore */ });
    }),
  );
}

export function getVideoDimensions(
  url: string,
): { width: number; height: number } | null {
  const m = metaCache.get(url);
  return m ? { width: m.width, height: m.height } : null;
}

export function getVideoDurationSecs(url: string): number | null {
  return metaCache.get(url)?.durationSecs ?? null;
}

export function registerVideoGlobals(): void {
  // No-op — retained for API compatibility with main.ts.
}
