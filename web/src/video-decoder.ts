// RPC client shell for the video-decode worker.
// All decoding happens in web/src/workers/video-decode-worker.ts;
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
let nextRpcId = 1;
const pending = new Map<
  number,
  { resolve: (value: unknown) => void; reject: (err: Error) => void }
>();
const metaCache = new Map<string, VideoSourceMeta>();

function ensureWorker(): Worker {
  if (worker) return worker;
  worker = new Worker(
    new URL('./workers/video-decode-worker.ts', import.meta.url),
    { type: 'module' },
  );
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
