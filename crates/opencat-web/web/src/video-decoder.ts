// RPC client shell for the video-decode worker.
// All decoding happens in crates/opencat-web/web/src/workers/video-decode-worker.ts;
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

const SLOW_WORKER_RPC_WARN_MS = 300;
const SLOW_RGBA_READBACK_WARN_MS = 300;

function fmtSecs(seconds: number): string {
  return `${seconds.toFixed(3)}s`;
}

function fmtUs(us: number): string {
  return `${(us / 1_000_000).toFixed(3)}s`;
}

function fmtMs(ms: number): string {
  return `${ms.toFixed(1)}ms`;
}

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
    console.error('[video-decoder] worker error:', e.message);
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
  const startedAt = performance.now();
  console.log(
    `[video-decoder rpc#${id}] prepare start asset=${url} bytes=${buffer.byteLength}`,
  );
  const res = await rpc<{ type: 'prepare'; id: number; meta: VideoSourceMeta }>(
    { type: 'prepare', id, assetId: url, buffer },
    [buffer],
  );
  console.log(
    `[video-decoder rpc#${id}] prepare done asset=${url} dt=${fmtMs(performance.now() - startedAt)} meta=${res.meta.width}x${res.meta.height} duration=${res.meta.durationSecs === null ? 'null' : fmtSecs(res.meta.durationSecs)}`,
  );
  metaCache.set(url, res.meta);
  return res.meta;
}

export async function getDecodedVideoFrame(
  url: string,
  timeSecs: number,
  quality: VideoPreviewQuality = 'realtime',
): Promise<VideoFrame | null> {
  if (!metaCache.has(url)) {
    console.warn(
      `[video-decoder] getFrame skipped: asset not prepared asset=${url} t=${fmtSecs(timeSecs)} q=${quality}`,
    );
    return null;
  }
  const id = nextId();
  const startedAt = performance.now();
  const shouldLog = quality === 'exact';
  if (shouldLog) {
    console.log(
      `[video-decoder rpc#${id}] getFrame start asset=${url} t=${fmtSecs(timeSecs)} q=${quality}`,
    );
  }
  const res = await rpc<{
    type: 'getFrame';
    id: number;
    frame: VideoFrame | null;
  }>({ type: 'getFrame', id, assetId: url, timeSecs, quality });
  const elapsedMs = performance.now() - startedAt;
  const log = !res.frame || elapsedMs >= SLOW_WORKER_RPC_WARN_MS
    ? console.warn
    : console.log;
  if (shouldLog || !res.frame || elapsedMs >= SLOW_WORKER_RPC_WARN_MS) {
    log(
      `[video-decoder rpc#${id}] getFrame done asset=${url} t=${fmtSecs(timeSecs)} q=${quality} dt=${fmtMs(elapsedMs)} result=${res.frame ? `${res.frame.displayWidth}x${res.frame.displayHeight} ts=${fmtUs(res.frame.timestamp)}` : 'NULL'}`,
    );
  }
  return res.frame;
}

export async function getDecodedFrameRgba(
  url: string,
  timeSecs: number,
  quality: VideoPreviewQuality = 'realtime',
): Promise<{ rgba: Uint8Array; width: number; height: number } | null> {
  const meta = metaCache.get(url);
  if (!meta) {
    console.warn(
      `[video-decoder] rgba skipped: asset not prepared asset=${url} t=${fmtSecs(timeSecs)} q=${quality}`,
    );
    return null;
  }
  const totalStartedAt = performance.now();
  const frame = await getDecodedVideoFrame(url, timeSecs, quality);
  if (!frame) {
    console.warn(
      `[video-decoder] rgba NULL asset=${url} t=${fmtSecs(timeSecs)} q=${quality} dt=${fmtMs(performance.now() - totalStartedAt)}`,
    );
    return null;
  }

  try {
    const w = frame.displayWidth || meta.width;
    const h = frame.displayHeight || meta.height;
    const readbackStartedAt = performance.now();
    const off = new OffscreenCanvas(w, h);
    const ctx = off.getContext('2d', { willReadFrequently: true });
    if (!ctx) {
      console.warn(
        `[video-decoder] rgba failed: no 2d context asset=${url} t=${fmtSecs(timeSecs)} q=${quality} size=${w}x${h}`,
      );
      return null;
    }
    ctx.drawImage(frame, 0, 0);
    const img = ctx.getImageData(0, 0, w, h);
    const readbackMs = performance.now() - readbackStartedAt;
    const totalMs = performance.now() - totalStartedAt;
    const log = readbackMs >= SLOW_RGBA_READBACK_WARN_MS ? console.warn : console.log;
    if (quality === 'exact' || readbackMs >= SLOW_RGBA_READBACK_WARN_MS) {
      log(
        `[video-decoder] rgba done asset=${url} t=${fmtSecs(timeSecs)} q=${quality} readback=${fmtMs(readbackMs)} total=${fmtMs(totalMs)} size=${w}x${h} bytes=${img.data.byteLength}`,
      );
    }
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
