// Web Worker that owns WebDemuxer + persistent VideoDecoder per asset.
// Communicates with web/src/video-decoder.ts via the RPC envelope
// defined in video-decode-worker.types.ts.

/// <reference lib="webworker" />
/// <reference lib="dom" />

import type {
  ErrorResponse,
  GetFrameRequest,
  PrepareRequest,
  ReleaseRequest,
  WorkerRequest,
  WorkerResponse,
} from './video-decode-worker.types';

import type { EncodedChunkDesc } from './video-decode-helpers';

// Per-asset state lives here. Populated by `prepare`, consumed by
// `getFrame`, torn down by `release`.
interface AssetState {
  config: VideoDecoderConfig;
  width: number;
  height: number;
  durationSecs: number | null;
  chunks: EncodedChunkDesc[];
  keyframeTimesUs: number[];

  decoder: VideoDecoder | null;
  decoderKeyTimeUs: number; // -1 when uninitialized
  cursorChunkIdx: number;
  currentPtsUs: number; // -1 when no frame yet
  hasFrame: boolean;

  pendingFrames: { timestamp: number; videoFrame: VideoFrame }[];
  decoderErrored: boolean;

  // Per-asset serialization: while inflight, queue subsequent getFrame calls
  inflight: Promise<unknown> | null;

  // Demuxer instance retained for the lifetime of the asset
  demuxer: unknown; // typed loosely; see Task 5.2 for the imported type
}

const assets = new Map<string, AssetState>();

self.onmessage = (e: MessageEvent<WorkerRequest>) => {
  const req = e.data;
  switch (req.type) {
    case 'prepare':
      void handlePrepare(req);
      return;
    case 'getFrame':
      void handleGetFrame(req);
      return;
    case 'release':
      void handleRelease(req);
      return;
    default: {
      const _exhaustive: never = req;
      void _exhaustive;
      console.warn('[video-decode-worker] unknown request type', e.data);
    }
  }
};

function postResponse(res: WorkerResponse, transfer: Transferable[] = []): void {
  (self as unknown as Worker).postMessage(res, transfer);
}

function postError(id: number, message: string): void {
  const res: ErrorResponse = { type: 'error', id, message };
  postResponse(res);
}

// Stubs — filled in by Tasks 5.2 / 5.5 / 5.6.
async function handlePrepare(req: PrepareRequest): Promise<void> {
  postError(req.id, 'prepare: not implemented yet');
}
async function handleGetFrame(req: GetFrameRequest): Promise<void> {
  postError(req.id, 'getFrame: not implemented yet');
}
async function handleRelease(req: ReleaseRequest): Promise<void> {
  postError(req.id, 'release: not implemented yet');
}
