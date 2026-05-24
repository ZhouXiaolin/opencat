// Web Worker that owns web-demuxer packet tables and WebCodecs frame decode.
// The decode path intentionally mirrors the old mp4box.js implementation:
// pick the keyframe before the requested media time, feed a finite decode-order
// slice through a fresh VideoDecoder, then choose the decoded frame nearest to
// the target.  Subsequent frames from the same slice are cached in-worker so
// sequential preview/export frames do not re-decode the same long GOP.

/// <reference lib="webworker" />
/// <reference lib="dom" />

import { WebDemuxer } from 'web-demuxer';

import type {
  ErrorResponse,
  GetFrameRequest,
  PrepareRequest,
  ReleaseRequest,
  VideoPreviewQuality,
  WorkerRequest,
  WorkerResponse,
} from './video-decode-worker.types';

import {
  chunkIdxAtTime,
  decodeSliceEndIndex,
  type EncodedChunkDesc,
  encodedChunkFrom,
  nearestKeyframeBefore,
  previousKeyframeBefore,
  seekFeedMarginUs,
} from './video-decode-helpers';

const WD_WASM_FILE_PATH = new URL(
  '../web-demuxer.wasm',
  self.location.href,
).href;

const FLUSH_TIMEOUT_MS = 2500;
const OUTPUT_WAIT_TIMEOUT_MS = 2500;
const DECODE_QUEUE_HIGH_WATER = 12;
const CACHE_HIT_TOLERANCE_US = 50_000;
const CACHE_BEHIND_TARGET_US = 250_000;
const MAX_CACHED_FRAMES_PER_ASSET = 90;
const DECODE_SLICE_LOOKAHEAD_CHUNKS = 16;
const DECODE_YIELD_CHUNK_INTERVAL = 32;

interface DecodedFrame {
  timestamp: number;
  videoFrame: VideoFrame;
}

interface DecodeAttempt {
  keyTimeUs: number;
  keyIdx: number;
  endIdx: number;
  decodedCount: number;
  decodedFirstUs: number | null;
  decodedLastUs: number | null;
  flushed: boolean;
  errorMessage: string | null;
}

interface DecodeCollector {
  best: DecodedFrame | null;
  outputCount: number;
  closedCount: number;
  minUs: number;
  maxUs: number;
}

interface AssetState {
  config: VideoDecoderConfig;
  width: number;
  height: number;
  durationSecs: number | null;
  maxPtsUs: number;
  chunks: EncodedChunkDesc[];
  keyframeTimesUs: number[];
  demuxer: WebDemuxer;
  frameCache: DecodedFrame[];
  inflight: Promise<unknown> | null;
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

async function handlePrepare(req: PrepareRequest): Promise<void> {
  let demuxer: WebDemuxer | null = null;
  try {
    const prior = assets.get(req.assetId);
    if (prior) {
      destroyAssetState(prior);
      assets.delete(req.assetId);
    }

    demuxer = new WebDemuxer({ wasmFilePath: WD_WASM_FILE_PATH });
    const file = new File([req.buffer], 'video', {
      type: 'application/octet-stream',
    });
    await demuxer.load(file);

    const config = await demuxer.getDecoderConfig('video') as VideoDecoderConfig;
    const chunks: EncodedChunkDesc[] = [];
    const keyframeTimesUs: number[] = [];
    let maxPtsUs = 0;

    const packetStream = demuxer.readAVPacket();
    const reader = packetStream.getReader();
    try {
      // eslint-disable-next-line no-constant-condition
      while (true) {
        const { value: pkt, done } = await reader.read();
        if (done) break;

        const copy = new Uint8Array(pkt.data.byteLength);
        copy.set(pkt.data);
        const timestamp = Math.max(0, Math.round(pkt.timestamp * 1_000_000));
        const duration = Math.max(0, Math.round(pkt.duration * 1_000_000));
        const type: EncodedVideoChunkType = pkt.keyframe === 1 ? 'key' : 'delta';

        chunks.push({
          type,
          timestamp,
          duration,
          data: copy.buffer,
        });
        maxPtsUs = Math.max(maxPtsUs, timestamp);
        if (type === 'key') keyframeTimesUs.push(timestamp);
      }
    } finally {
      reader.releaseLock();
    }

    normalizeKeyframeTimes(keyframeTimesUs);
    if (keyframeTimesUs.length === 0) keyframeTimesUs.push(0);

    const support = await VideoDecoder.isConfigSupported(config);
    if (!support.supported) {
      demuxer.destroy();
      demuxer = null;
      postError(req.id, `prepare: codec not supported (${config.codec ?? 'unknown'})`);
      return;
    }

    const width = config.codedWidth ?? 0;
    const height = config.codedHeight ?? 0;
    const durationSecs = computeDurationSecs(chunks);

    const ownedDemuxer = demuxer;
    demuxer = null;
    assets.set(req.assetId, {
      config,
      width,
      height,
      durationSecs,
      maxPtsUs,
      chunks,
      keyframeTimesUs,
      demuxer: ownedDemuxer,
      frameCache: [],
      inflight: null,
    });

    postResponse({
      type: 'prepare',
      id: req.id,
      meta: { width, height, durationSecs },
    });
  } catch (err) {
    if (demuxer) {
      try { demuxer.destroy(); } catch { /* ignore */ }
    }
    const message = err instanceof Error ? err.message : String(err);
    postError(req.id, message);
  }
}

function normalizeKeyframeTimes(times: number[]): void {
  times.sort((a, b) => a - b);
  let write = 0;
  for (const t of times) {
    if (write === 0 || t - times[write - 1] >= 1) {
      times[write] = t;
      write++;
    }
  }
  times.length = write;
}

function computeDurationSecs(chunks: EncodedChunkDesc[]): number | null {
  let maxEndUs = 0;
  for (const chunk of chunks) {
    maxEndUs = Math.max(maxEndUs, chunk.timestamp + chunk.duration);
  }
  return maxEndUs > 0 ? maxEndUs / 1_000_000 : null;
}

function destroyAssetState(st: AssetState): void {
  closeCachedFrames(st);
  try { st.demuxer.destroy(); } catch { /* ignore */ }
}

function closeCachedFrames(st: AssetState): void {
  for (const frame of st.frameCache) {
    try { frame.videoFrame.close(); } catch { /* ignore */ }
  }
  st.frameCache = [];
}

async function handleGetFrame(req: GetFrameRequest): Promise<void> {
  const st = assets.get(req.assetId);
  if (!st) {
    postError(req.id, `asset not prepared: ${req.assetId}`);
    return;
  }

  const previous = st.inflight ?? Promise.resolve();
  const work = previous.catch(() => undefined).then(async () => {
    const requestedUs = Math.max(0, Math.round(req.timeSecs * 1_000_000));
    const targetUs = st.chunks.length > 0
      ? Math.min(requestedUs, st.maxPtsUs)
      : requestedUs;

    return getFrameAtTime(st, req.assetId, targetUs, req.quality);
  });
  st.inflight = work;

  try {
    const frame = await work;
    if (frame) {
      postResponse({ type: 'getFrame', id: req.id, frame }, [frame]);
    } else {
      postResponse({ type: 'getFrame', id: req.id, frame: null });
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    postError(req.id, message);
  } finally {
    if (st.inflight === work) st.inflight = null;
  }
}

async function getFrameAtTime(
  st: AssetState,
  assetId: string,
  targetUs: number,
  quality: VideoPreviewQuality,
): Promise<VideoFrame | null> {
  const cached = cloneCachedFrame(st, targetUs);
  if (cached) {
    return cached;
  }

  const attempts: DecodeAttempt[] = [];
  let keyTimeUs = nearestKeyframeBefore(st.keyframeTimesUs, targetUs);

  while (keyTimeUs >= 0) {
    const { frames, attempt } = await decodeSliceFromKey(
      st,
      keyTimeUs,
      targetUs,
      quality,
    );
    attempts.push(attempt);

    if (frames.length > 0) {
      cacheDecodedFrames(st, frames, targetUs, quality);
      const decoded = cloneCachedFrame(st, targetUs);
      if (decoded) {
        return decoded;
      }
    }

    keyTimeUs = previousKeyframeBefore(st.keyframeTimesUs, keyTimeUs);
  }

  warnDecodeNull(assetId, targetUs, attempts);
  return null;
}

async function decodeSliceFromKey(
  st: AssetState,
  keyTimeUs: number,
  targetUs: number,
  quality: VideoPreviewQuality,
): Promise<{ frames: DecodedFrame[]; attempt: DecodeAttempt }> {
  const keyIdx = keyChunkIndexAtTime(st.chunks, keyTimeUs);
  if (keyIdx < 0) {
    return {
      frames: [],
      attempt: emptyAttempt(keyTimeUs, -1, -1, false, 'key chunk not found'),
    };
  }

  const endIdx = decodeSliceEndIndex(
    st.chunks,
    keyIdx,
    targetUs,
    seekFeedMarginUs(quality),
    DECODE_SLICE_LOOKAHEAD_CHUNKS,
  );
  if (endIdx < keyIdx) {
    return {
      frames: [],
      attempt: emptyAttempt(keyTimeUs, keyIdx, endIdx, false, 'empty decode slice'),
    };
  }

  const coverUs = Math.min(
    st.maxPtsUs,
    targetUs + seekFeedMarginUs(quality),
  );
  const collector: DecodeCollector = {
    best: null,
    outputCount: 0,
    closedCount: 0,
    minUs: Number.POSITIVE_INFINITY,
    maxUs: Number.NEGATIVE_INFINITY,
  };
  let errorMessage: string | null = null;

  const decoder = new VideoDecoder({
    output(frame) {
      collectDecodedFrame(collector, frame, targetUs);
    },
    error(err) {
      errorMessage = err.message;
    },
  });

  try {
    decoder.configure(st.config);
    for (let i = keyIdx; i <= endIdx; i++) {
      decoder.decode(encodedChunkFrom(st.chunks[i]));
      if (decoder.decodeQueueSize >= DECODE_QUEUE_HIGH_WATER) {
        if ((i - keyIdx) % DECODE_YIELD_CHUNK_INTERVAL === 0) {
          await yieldToDecoder();
        }
      } else if ((i & 0x0f) === 0) {
        await yieldToDecoder();
      }
      if (collectorHasFrameAtOrPast(collector, targetUs) && i >= endIdx) {
        break;
      }
    }
  } catch (err) {
    errorMessage = err instanceof Error ? err.message : String(err);
  }

  let flushed = false;
  if (errorMessage === null && !collectorHasFrameAtOrPast(collector, coverUs)) {
    await waitForTargetFrame(decoder, collector, coverUs, OUTPUT_WAIT_TIMEOUT_MS);
  }

  if (errorMessage === null && !collectorHasFrameAtOrPast(collector, coverUs)) {
    flushed = await flushDecoderWithTimeout(decoder, FLUSH_TIMEOUT_MS);
    if (!flushed) {
      errorMessage = `flush timeout after ${FLUSH_TIMEOUT_MS}ms`;
    }
  }

  try { decoder.close(); } catch { /* ignore */ }

  const frames = collector.best ? [collector.best] : [];
  return {
    frames,
    attempt: {
      keyTimeUs,
      keyIdx,
      endIdx,
      decodedCount: collector.outputCount,
      decodedFirstUs: Number.isFinite(collector.minUs) ? collector.minUs : null,
      decodedLastUs: Number.isFinite(collector.maxUs) ? collector.maxUs : null,
      flushed,
      errorMessage,
    },
  };
}

async function waitForTargetFrame(
  decoder: VideoDecoder,
  collector: DecodeCollector,
  targetUs: number,
  timeoutMs: number,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (collectorHasFrameAtOrPast(collector, targetUs) || decoder.decodeQueueSize === 0) {
      return;
    }
    await yieldToDecoder();
  }
}

function yieldToDecoder(): Promise<void> {
  return new Promise<void>((resolve) => setTimeout(resolve, 0));
}

function collectDecodedFrame(
  collector: DecodeCollector,
  frame: VideoFrame,
  targetUs: number,
): void {
  collector.outputCount++;
  collector.minUs = Math.min(collector.minUs, frame.timestamp);
  collector.maxUs = Math.max(collector.maxUs, frame.timestamp);

  const next = { timestamp: frame.timestamp, videoFrame: frame };
  if (!collector.best) {
    collector.best = next;
    return;
  }

  const oldDiff = Math.abs(collector.best.timestamp - targetUs);
  const nextDiff = Math.abs(next.timestamp - targetUs);
  if (nextDiff < oldDiff) {
    try { collector.best.videoFrame.close(); } catch { /* ignore */ }
    collector.closedCount++;
    collector.best = next;
    return;
  }

  try { frame.close(); } catch { /* ignore */ }
  collector.closedCount++;
}

function collectorHasFrameAtOrPast(
  collector: DecodeCollector,
  targetUs: number,
): boolean {
  return collector.maxUs >= targetUs;
}

function emptyAttempt(
  keyTimeUs: number,
  keyIdx: number,
  endIdx: number,
  flushed: boolean,
  errorMessage: string,
): DecodeAttempt {
  return {
    keyTimeUs,
    keyIdx,
    endIdx,
    decodedCount: 0,
    decodedFirstUs: null,
    decodedLastUs: null,
    flushed,
    errorMessage,
  };
}

function keyChunkIndexAtTime(
  chunks: readonly EncodedChunkDesc[],
  keyTimeUs: number,
): number {
  for (let i = 0; i < chunks.length; i++) {
    if (chunks[i].type === 'key' && chunks[i].timestamp === keyTimeUs) return i;
  }
  return chunkIdxAtTime(chunks, keyTimeUs);
}

async function flushDecoderWithTimeout(
  decoder: VideoDecoder,
  timeoutMs: number,
): Promise<boolean> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  try {
    const timeout = Symbol('flush-timeout');
    const result = await Promise.race([
      decoder.flush().then(() => true),
      new Promise<typeof timeout>((resolve) => {
        timer = setTimeout(() => resolve(timeout), timeoutMs);
      }),
    ]);
    return result === true;
  } catch {
    return false;
  } finally {
    if (timer) clearTimeout(timer);
  }
}

function cacheDecodedFrames(
  st: AssetState,
  frames: DecodedFrame[],
  targetUs: number,
  quality: VideoPreviewQuality,
): void {
  const lower = Math.max(0, targetUs - CACHE_BEHIND_TARGET_US);

  for (const frame of frames) {
    const diff = Math.abs(frame.timestamp - targetUs);
    const shouldKeep =
      diff <= CACHE_HIT_TOLERANCE_US ||
      frame.timestamp >= lower;

    if (!shouldKeep) {
      try { frame.videoFrame.close(); } catch { /* ignore */ }
      continue;
    }

    insertCachedFrame(st, frame);
  }

  st.frameCache.sort((a, b) => a.timestamp - b.timestamp);
  trimFrameCache(st, targetUs);
}

function insertCachedFrame(st: AssetState, frame: DecodedFrame): void {
  const duplicate = st.frameCache.find((item) => item.timestamp === frame.timestamp);
  if (duplicate) {
    try { frame.videoFrame.close(); } catch { /* ignore */ }
    return;
  }
  st.frameCache.push(frame);
}

function trimFrameCache(st: AssetState, targetUs: number): void {
  while (st.frameCache.length > MAX_CACHED_FRAMES_PER_ASSET) {
    const first = st.frameCache[0];
    const removeIdx = first.timestamp < targetUs ? 0 : st.frameCache.length - 1;
    const [removed] = st.frameCache.splice(removeIdx, 1);
    try { removed.videoFrame.close(); } catch { /* ignore */ }
  }
}

function cloneCachedFrame(
  st: AssetState,
  targetUs: number,
  toleranceUs = CACHE_HIT_TOLERANCE_US,
): VideoFrame | null {
  const nearest = findNearestCachedFrame(st, targetUs);
  const bestIdx = nearest?.index ?? -1;
  const bestDiff = nearest?.diffUs ?? Infinity;

  if (bestIdx < 0 || bestDiff > toleranceUs) return null;

  try {
    return st.frameCache[bestIdx].videoFrame.clone();
  } catch {
    const [closed] = st.frameCache.splice(bestIdx, 1);
    try { closed.videoFrame.close(); } catch { /* ignore */ }
    return null;
  }
}

function warnDecodeNull(
  assetId: string,
  targetUs: number,
  attempts: readonly DecodeAttempt[],
): void {
  const summary = attempts.map((a) => {
    const range = a.decodedFirstUs === null
      ? 'empty'
      : `${fmtSecs(a.decodedFirstUs)}..${fmtSecs(a.decodedLastUs ?? a.decodedFirstUs)}`;
    return `key=${fmtSecs(a.keyTimeUs)} idx=${a.keyIdx} slice=${a.keyIdx}..${a.endIdx} decoded=${a.decodedCount} range=${range} flushed=${a.flushed}${a.errorMessage ? ` err=${a.errorMessage}` : ''}`;
  }).join(' | ');

  console.warn(
    `[video-decode-worker] decode NULL asset=${assetId} target=${fmtSecs(targetUs)} attempts=${summary || '(none)'}`,
  );
}

function fmtSecs(us: number): string {
  return `${(us / 1_000_000).toFixed(3)}s`;
}

function findNearestCachedFrame(
  st: AssetState,
  targetUs: number,
): { index: number; timestamp: number; diffUs: number } | null {
  let bestIdx = -1;
  let bestDiff = Infinity;

  for (let i = 0; i < st.frameCache.length; i++) {
    const diff = Math.abs(st.frameCache[i].timestamp - targetUs);
    if (diff < bestDiff) {
      bestDiff = diff;
      bestIdx = i;
    }
  }

  if (bestIdx < 0) return null;
  return {
    index: bestIdx,
    timestamp: st.frameCache[bestIdx].timestamp,
    diffUs: bestDiff,
  };
}

async function handleRelease(req: ReleaseRequest): Promise<void> {
  const st = assets.get(req.assetId);
  if (!st) {
    postResponse({ type: 'release', id: req.id });
    return;
  }

  while (st.inflight) {
    try { await st.inflight; } catch { /* ignore */ }
  }
  destroyAssetState(st);
  assets.delete(req.assetId);
  postResponse({ type: 'release', id: req.id });
}
