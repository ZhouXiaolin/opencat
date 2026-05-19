// Web Worker that owns WebDemuxer + persistent VideoDecoder per asset.
// Communicates with web/src/video-decoder.ts via the RPC envelope
// defined in video-decode-worker.types.ts.

/// <reference lib="webworker" />
/// <reference lib="dom" />

import { WebDemuxer } from 'web-demuxer';

import type {
  ErrorResponse,
  GetFrameRequest,
  PrepareRequest,
  ReleaseRequest,
  WorkerRequest,
  WorkerResponse,
} from './video-decode-worker.types';

import type { EncodedChunkDesc } from './video-decode-helpers';

// URL of the web-demuxer wasm file, wired up by Task 1.2.
const WD_WASM_FILE_PATH = '/web-demuxer/wasm-files/web-demuxer.wasm';

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
  demuxer: WebDemuxer;
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

// `prepare`: demux entire stream, build chunk + keyframe tables,
// probe codec support, register asset state. `getFrame` / `release`
// are still stubbed and will land in Tasks 5.5 / 5.6.
async function handlePrepare(req: PrepareRequest): Promise<void> {
  try {
    // Release any prior state for this assetId.
    const prior = assets.get(req.assetId);
    if (prior) {
      destroyAssetState(prior);
      assets.delete(req.assetId);
    }

    const demuxer = new WebDemuxer({ wasmFilePath: WD_WASM_FILE_PATH });
    // web-demuxer v4 `load()` takes File | string. Wrap the transferred
    // ArrayBuffer in a File (the name/type are cosmetic — demuxing is by
    // probing, not by filename).
    const file = new File([req.buffer], 'video', {
      type: 'application/octet-stream',
    });
    await demuxer.load(file);

    const config = await demuxer.getDecoderConfig('video');

    // Enumerate every video packet in DTS order to build the chunk
    // descriptor list + keyframe table. `readAVPacket()` with no args
    // streams the whole video track (start=0, end=0 → no limit).
    const chunks: EncodedChunkDesc[] = [];
    const keyframeTimesUs: number[] = [];
    const packetStream = demuxer.readAVPacket();
    const reader = packetStream.getReader();
    try {
      // Sequential read loop — keep going until the stream closes.
      // eslint-disable-next-line no-constant-condition
      while (true) {
        const { value: pkt, done } = await reader.read();
        if (done) break;
        // `pkt.data` is a Uint8Array view; copy into a fresh Uint8Array so
        // we own a plain ArrayBuffer (never SharedArrayBuffer) for downstream
        // EncodedVideoChunk.
        const copy = new Uint8Array(pkt.data.byteLength);
        copy.set(pkt.data);
        const data = copy.buffer;
        const isKey = pkt.keyframe === 1;
        chunks.push({
          type: isKey ? 'key' : 'delta',
          timestamp: pkt.timestamp,
          duration: pkt.duration,
          data,
        });
        if (isKey) {
          const last = keyframeTimesUs[keyframeTimesUs.length - 1];
          if (
            keyframeTimesUs.length === 0 ||
            pkt.timestamp - last >= 1
          ) {
            keyframeTimesUs.push(Math.max(0, pkt.timestamp));
          }
        }
      }
    } finally {
      reader.releaseLock();
    }

    // Engine parity: always have at least one keyframe anchor.
    if (keyframeTimesUs.length === 0) keyframeTimesUs.push(0);

    // Probe codec support before keeping any state around.
    const support = await VideoDecoder.isConfigSupported(config);
    if (!support.supported) {
      demuxer.destroy();
      postError(
        req.id,
        `prepare: codec not supported (${config.codec ?? 'unknown'})`,
      );
      return;
    }

    const width = config.codedWidth ?? 0;
    const height = config.codedHeight ?? 0;
    const last = chunks[chunks.length - 1];
    const durationSecs =
      last !== undefined ? (last.timestamp + last.duration) / 1_000_000 : null;

    const st: AssetState = {
      config,
      width,
      height,
      durationSecs,
      chunks,
      keyframeTimesUs,
      decoder: null,
      decoderKeyTimeUs: -1,
      cursorChunkIdx: 0,
      currentPtsUs: -1,
      hasFrame: false,
      pendingFrames: [],
      decoderErrored: false,
      inflight: null,
      demuxer,
    };
    assets.set(req.assetId, st);

    postResponse({
      type: 'prepare',
      id: req.id,
      meta: { width, height, durationSecs },
    });
  } catch (err) {
    postError(req.id, err instanceof Error ? err.message : String(err));
  }
}
function destroyAssetState(st: AssetState): void {
  if (st.decoder) {
    try {
      st.decoder.close();
    } catch {
      // ignore — decoder may already be closed/errored
    }
    st.decoder = null;
  }
  for (const f of st.pendingFrames) {
    try {
      f.videoFrame.close();
    } catch {
      // ignore — frame may already be closed
    }
  }
  st.pendingFrames = [];
  try {
    st.demuxer.destroy();
  } catch {
    // ignore — demuxer worker may already be terminated
  }
}
async function handleGetFrame(req: GetFrameRequest): Promise<void> {
  postError(req.id, 'getFrame: not implemented yet');
}
async function handleRelease(req: ReleaseRequest): Promise<void> {
  postError(req.id, 'release: not implemented yet');
}
