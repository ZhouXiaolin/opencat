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

import {
  type EncodedChunkDesc,
  encodedChunkFrom,
  nearestKeyframeBefore,
  previousKeyframeBefore,
  chunkIdxAtTime,
  shouldSeekToTarget,
} from './video-decode-helpers';

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
//
// Caller (web/src/video-decoder.ts) is expected to deduplicate prepare per
// assetId via its metaCache; the worker does not guard against two
// concurrent prepares for the same asset.
async function handlePrepare(req: PrepareRequest): Promise<void> {
  // Hoisted so the catch can destroy a partially-built demuxer. Set to
  // null once ownership transfers to the assets Map (or after an inline
  // destroy() in an error branch) so the catch never double-destroys.
  let demuxer: WebDemuxer | null = null;
  try {
    // Release any prior state for this assetId.
    const prior = assets.get(req.assetId);
    if (prior) {
      destroyAssetState(prior);
      assets.delete(req.assetId);
    }

    demuxer = new WebDemuxer({ wasmFilePath: WD_WASM_FILE_PATH });
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
      demuxer = null;
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

    // Transfer demuxer ownership to the assets Map; null the local so the
    // catch (and any later code in this scope) can't double-destroy.
    const ownedDemuxer = demuxer;
    demuxer = null;
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
      demuxer: ownedDemuxer,
    };
    assets.set(req.assetId, st);

    postResponse({
      type: 'prepare',
      id: req.id,
      meta: { width, height, durationSecs },
    });
  } catch (err) {
    if (demuxer) {
      try {
        demuxer.destroy();
      } catch {
        // ignore — demuxer may already be torn down
      }
    }
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

const FEED_MARGIN_US = 500_000;
const FLUSH_TIMEOUT_MS = 3_000;

function makeDecoder(st: AssetState): VideoDecoder {
  st.decoderErrored = false;
  return new VideoDecoder({
    output: (frame) => {
      st.pendingFrames.push({ timestamp: frame.timestamp, videoFrame: frame });
    },
    error: (err) => {
      console.warn('[video-decode-worker] decoder error:', err.message);
      st.decoderErrored = true;
    },
  });
}

function closeDecoder(st: AssetState): void {
  if (st.decoder) {
    try { st.decoder.close(); } catch { /* ignore */ }
    st.decoder = null;
  }
}

function closeAndClearPendingFrames(st: AssetState): void {
  for (const f of st.pendingFrames) {
    try { f.videoFrame.close(); } catch { /* ignore */ }
  }
  st.pendingFrames = [];
}

function flushWithTimeout(decoder: VideoDecoder, timeoutMs: number): Promise<void> {
  return new Promise((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      console.warn('[video-decode-worker] flush timed out');
      resolve();
    }, timeoutMs);
    decoder.flush().then(
      () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve();
      },
      () => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve();
      },
    );
  });
}

/** Feed chunks from `st.cursorChunkIdx` until past `targetUs + margin`,
 *  flush, then return the frame in `pendingFrames` closest to targetUs.
 *  Closes the other pending frames. */
async function feedAndCollect(
  st: AssetState,
  targetUs: number,
): Promise<VideoFrame | null> {
  if (!st.decoder) return null;

  while (st.cursorChunkIdx < st.chunks.length) {
    if (st.decoderErrored) break;
    const chunk = st.chunks[st.cursorChunkIdx];
    if (chunk.timestamp > targetUs + FEED_MARGIN_US) break;
    try {
      st.decoder.decode(encodedChunkFrom(chunk));
    } catch (err) {
      console.warn('[video-decode-worker] decode threw:', err);
      st.decoderErrored = true;
      break;
    }
    st.cursorChunkIdx++;
  }

  if (st.decoderErrored) {
    closeAndClearPendingFrames(st);
    closeDecoder(st);
    return null;
  }

  await flushWithTimeout(st.decoder, FLUSH_TIMEOUT_MS);

  if (st.decoderErrored) {
    closeAndClearPendingFrames(st);
    closeDecoder(st);
    return null;
  }

  if (st.pendingFrames.length === 0) return null;

  // Pick closest, close the rest
  let bestIdx = 0;
  let bestDiff = Infinity;
  for (let i = 0; i < st.pendingFrames.length; i++) {
    const diff = Math.abs(st.pendingFrames[i].timestamp - targetUs);
    if (diff < bestDiff) {
      bestDiff = diff;
      bestIdx = i;
    }
  }
  const best = st.pendingFrames[bestIdx];
  for (let i = 0; i < st.pendingFrames.length; i++) {
    if (i !== bestIdx) {
      try { st.pendingFrames[i].videoFrame.close(); } catch { /* ignore */ }
    }
  }
  st.pendingFrames = [];

  st.currentPtsUs = best.timestamp;
  st.hasFrame = true;
  return best.videoFrame;
}

/** Engine-aligned seek + decode. Walks back through keyframes if a feed
 *  yields zero frames (Open-GOP / non-IDR keyframe fallback).
 *  Decision is purely "feed yielded 0 frames → try previous keyframe"
 *  — codec-agnostic, no NAL parsing. */
async function seekAndDecode(
  st: AssetState,
  targetUs: number,
): Promise<VideoFrame | null> {
  let keyTimeUs = nearestKeyframeBefore(st.keyframeTimesUs, targetUs);
  while (keyTimeUs >= 0) {
    // Reset decoder and cursor to this keyframe
    closeDecoder(st);
    closeAndClearPendingFrames(st);
    st.decoder = makeDecoder(st);
    try {
      st.decoder.configure(st.config);
    } catch (err) {
      console.warn('[video-decode-worker] configure failed:', err);
      closeDecoder(st);
      return null;
    }
    st.decoderKeyTimeUs = keyTimeUs;

    const idx = chunkIdxAtTime(st.chunks, keyTimeUs);
    if (idx < 0) {
      // Keyframe time wasn't in our chunk table — should never happen
      // unless the demuxer mis-reported. Bail.
      closeDecoder(st);
      return null;
    }
    st.cursorChunkIdx = idx;
    st.hasFrame = false;
    st.currentPtsUs = -1;

    const frame = await feedAndCollect(st, targetUs);
    if (frame) return frame;

    // Feed-yielded-no-frames fallback. H.264 Open-GOP "key" packets
    // (non-IDR) fail under strict WebCodecs configure; retry from the
    // previous keyframe. VP9 / AV1 keyframes are independently
    // decodable so this fallback never triggers on those codecs.
    keyTimeUs = previousKeyframeBefore(st.keyframeTimesUs, keyTimeUs);
  }
  return null;
}

async function handleGetFrame(req: GetFrameRequest): Promise<void> {
  postError(req.id, 'getFrame: not implemented yet');
}
async function handleRelease(req: ReleaseRequest): Promise<void> {
  postError(req.id, 'release: not implemented yet');
}
