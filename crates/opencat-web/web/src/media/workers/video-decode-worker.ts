// Web Worker that owns web-demuxer packet tables and WebCodecs frame decode.
// Realtime preview keeps a small sliding cache ahead of the playhead with one
// persistent VideoDecoder session per asset. Exact/scrubbing requests still use
// finite keyframe-based slices so export and seek accuracy stay deterministic.

/// <reference lib="webworker" />
/// <reference lib="dom" />

import { WebDemuxer } from 'web-demuxer';

import type {
  ErrorResponse,
  GetFrameRequest,
  PrefetchFrameRequest,
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
  realtimeCacheWindowUs,
  seekFeedMarginUs,
  shouldCacheDecodedFrame,
  shouldStartRealtimePump,
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
const REALTIME_CACHE_BEHIND_FRAMES = 3;
const REALTIME_CACHE_LOW_WATER_FRAMES = 10;
const REALTIME_CACHE_HIGH_WATER_FRAMES = 24;
const REALTIME_PRIME_TIMEOUT_MS = 1200;
const REALTIME_SEEK_RESET_MIN_US = 1_500_000;

interface DecodedFrame {
  timestamp: number;
  videoFrame: VideoFrame;
}

interface DecodeCollector {
  frames: DecodedFrame[];
  outputCount: number;
  closedCount: number;
  minUs: number;
  maxUs: number;
}

interface RealtimeDecodeSession {
  decoder: VideoDecoder;
  keyTimeUs: number;
  nextChunkIdx: number;
  targetUs: number;
  frameDurationUs: number;
  closed: boolean;
  pumping: Promise<void> | null;
  errorMessage: string | null;
  outputCount: number;
  inputCount: number;
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
  realtime: RealtimeDecodeSession | null;
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
    case 'prefetchFrame':
      void handlePrefetchFrame(req);
      return;
    case 'release':
      void handleRelease(req);
      return;
    default: {
      const _exhaustive: never = req;
      void _exhaustive;
      return;
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
      realtime: null,
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
  closeRealtimeSession(st);
  closeCachedFrames(st);
  try { st.demuxer.destroy(); } catch { /* ignore */ }
}

function closeCachedFrames(st: AssetState): void {
  for (const frame of st.frameCache) {
    try { frame.videoFrame.close(); } catch { /* ignore */ }
  }
  st.frameCache = [];
}

function closeRealtimeSession(st: AssetState): void {
  const session = st.realtime;
  if (!session) return;
  session.closed = true;
  st.realtime = null;
  try { session.decoder.close(); } catch { /* ignore already-closed decoder */ }
}

async function handleGetFrame(req: GetFrameRequest): Promise<void> {
  const st = assets.get(req.assetId);
  if (!st) {
    postError(req.id, `asset not prepared: ${req.assetId}`);
    return;
  }

  const targetUs = requestTargetUs(st, req.timeSecs);
  if (req.quality === 'realtime') {
    try {
      const frame = await getRealtimeFrameAtTime(st, targetUs);
      if (frame) {
        postResponse({ type: 'getFrame', id: req.id, frame }, [frame]);
      } else {
        postResponse({ type: 'getFrame', id: req.id, frame: null });
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      postError(req.id, message);
    }
    return;
  }

  const previous = st.inflight ?? Promise.resolve();
  const work = previous.catch(() => undefined).then(async () => {
    closeRealtimeSession(st);
    return getFrameAtTime(st, targetUs, req.quality);
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

async function handlePrefetchFrame(req: PrefetchFrameRequest): Promise<void> {
  const st = assets.get(req.assetId);
  if (!st) {
    postError(req.id, `asset not prepared: ${req.assetId}`);
    return;
  }

  const targetUs = requestTargetUs(st, req.timeSecs);
  try {
    if (req.quality === 'realtime') {
      ensureRealtimeSession(st, targetUs);
      trimRealtimeCache(st, targetUs);
      startRealtimePump(st, true);
    } else {
      const previous = st.inflight ?? Promise.resolve();
      const work = previous.catch(() => undefined).then(async () => {
        closeRealtimeSession(st);
        const frame = await getFrameAtTime(st, targetUs, req.quality);
        try { frame?.close(); } catch { /* ignore */ }
      });
      st.inflight = work;
      try {
        await work;
      } finally {
        if (st.inflight === work) st.inflight = null;
      }
    }
    postResponse({ type: 'prefetchFrame', id: req.id, ok: true });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    postError(req.id, message);
  }
}

function requestTargetUs(st: AssetState, timeSecs: number): number {
  const requestedUs = Math.max(0, Math.round(timeSecs * 1_000_000));
  return st.chunks.length > 0
    ? Math.min(requestedUs, st.maxPtsUs)
    : requestedUs;
}

async function getRealtimeFrameAtTime(
  st: AssetState,
  targetUs: number,
): Promise<VideoFrame | null> {
  const cached = cloneCachedFrame(st, targetUs);
  if (cached) {
    ensureRealtimeSession(st, targetUs);
    trimRealtimeCache(st, targetUs);
    startRealtimePump(st);
    return cached;
  }

  const hadSession = !!st.realtime;
  const reset = shouldResetRealtimeSession(st, targetUs);
  const session = ensureRealtimeSession(st, targetUs);
  if (!session) return null;

  trimRealtimeCache(st, targetUs);
  startRealtimePump(st, true);

  if (reset || !hadSession || session.outputCount === 0) {
    const primed = await waitForRealtimeFrame(st, targetUs, REALTIME_PRIME_TIMEOUT_MS);
    if (primed) return primed;
  }

  const fallbackToleranceUs = Math.max(
    CACHE_HIT_TOLERANCE_US,
    session.frameDurationUs * 2,
  );
  return cloneCachedFrame(st, targetUs, fallbackToleranceUs);
}

function shouldResetRealtimeSession(st: AssetState, targetUs: number): boolean {
  const session = st.realtime;
  if (!session || session.closed) return true;
  const resetDistanceUs = Math.max(
    REALTIME_SEEK_RESET_MIN_US,
    session.frameDurationUs * REALTIME_CACHE_HIGH_WATER_FRAMES,
  );
  if (targetUs < session.keyTimeUs - CACHE_HIT_TOLERANCE_US) return true;
  return Math.abs(targetUs - session.targetUs) > resetDistanceUs;
}

function ensureRealtimeSession(
  st: AssetState,
  targetUs: number,
): RealtimeDecodeSession | null {
  const needsReset = shouldResetRealtimeSession(st, targetUs);
  if (needsReset) {
    closeRealtimeSession(st);
    closeCachedFrames(st);
  }

  if (st.realtime && !st.realtime.closed) {
    st.realtime.targetUs = targetUs;
    return st.realtime;
  }

  const keyTimeUs = nearestKeyframeBefore(st.keyframeTimesUs, targetUs);
  const keyIdx = keyChunkIndexAtTime(st.chunks, keyTimeUs);
  if (keyIdx < 0) return null;

  const frameDurationUs = estimateFrameDurationUs(st);
  let session!: RealtimeDecodeSession;
  const decoder = new VideoDecoder({
    output(frame) {
      collectRealtimeFrame(st, session, frame);
    },
    error(err) {
      session.errorMessage = err.message;
    },
  });

  try {
    decoder.configure(st.config);
  } catch {
    try { decoder.close(); } catch { /* ignore */ }
    return null;
  }

  session = {
    decoder,
    keyTimeUs,
    nextChunkIdx: keyIdx,
    targetUs,
    frameDurationUs,
    closed: false,
    pumping: null,
    errorMessage: null,
    outputCount: 0,
    inputCount: 0,
  };
  st.realtime = session;

  return session;
}

function estimateFrameDurationUs(st: AssetState): number {
  for (const chunk of st.chunks) {
    if (chunk.duration > 0) return Math.max(1, chunk.duration);
  }
  if (st.durationSecs && st.chunks.length > 0) {
    return Math.max(1, Math.round((st.durationSecs * 1_000_000) / st.chunks.length));
  }
  return 33_333;
}

function collectRealtimeFrame(
  st: AssetState,
  session: RealtimeDecodeSession,
  frame: VideoFrame,
): void {
  if (session.closed || st.realtime !== session) {
    try { frame.close(); } catch { /* ignore */ }
    return;
  }

  session.outputCount++;
  insertCachedFrame(st, {
    timestamp: frame.timestamp,
    videoFrame: frame,
  });
  st.frameCache.sort((a, b) => a.timestamp - b.timestamp);
  trimRealtimeCache(st, session.targetUs);
}

function startRealtimePump(
  st: AssetState,
  force = false,
): void {
  const session = st.realtime;
  if (!session || session.closed || session.pumping) return;
  const aheadFrames = realtimeAheadFrameCount(st, session.targetUs);
  if (!shouldStartRealtimePump(
    aheadFrames,
    REALTIME_CACHE_LOW_WATER_FRAMES,
    REALTIME_CACHE_HIGH_WATER_FRAMES,
    force,
  )) {
    return;
  }

  session.pumping = pumpRealtimeWindow(st, session)
    .catch((err) => {
      session.errorMessage = err instanceof Error ? err.message : String(err);
    })
    .finally(() => {
      if (st.realtime === session) session.pumping = null;
    });
}

async function pumpRealtimeWindow(
  st: AssetState,
  session: RealtimeDecodeSession,
): Promise<void> {
  while (!session.closed && st.realtime === session) {
    trimRealtimeCache(st, session.targetUs);
    if (session.errorMessage || session.nextChunkIdx >= st.chunks.length) break;

    const aheadFrames = realtimeAheadFrameCount(st, session.targetUs);
    if (aheadFrames >= REALTIME_CACHE_HIGH_WATER_FRAMES) break;

    let fed = 0;
    while (
      session.nextChunkIdx < st.chunks.length &&
      session.decoder.decodeQueueSize < DECODE_QUEUE_HIGH_WATER &&
      realtimeAheadFrameCount(st, session.targetUs) < REALTIME_CACHE_HIGH_WATER_FRAMES
    ) {
      session.decoder.decode(encodedChunkFrom(st.chunks[session.nextChunkIdx]));
      session.nextChunkIdx++;
      session.inputCount++;
      fed++;
      if (fed >= DECODE_YIELD_CHUNK_INTERVAL) break;
    }

    await yieldToDecoder();

    if (fed === 0 && session.decoder.decodeQueueSize === 0) break;
  }
}

async function waitForRealtimeFrame(
  st: AssetState,
  targetUs: number,
  timeoutMs: number,
): Promise<VideoFrame | null> {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    const cached = cloneCachedFrame(st, targetUs);
    if (cached) return cached;
    startRealtimePump(st, true);
    await yieldToDecoder();
  }
  return cloneCachedFrame(st, targetUs, Math.max(CACHE_HIT_TOLERANCE_US, estimateFrameDurationUs(st)));
}

function realtimeAheadFrameCount(st: AssetState, targetUs: number): number {
  const toleranceUs = st.realtime?.frameDurationUs ?? CACHE_HIT_TOLERANCE_US;
  return st.frameCache.filter((frame) => frame.timestamp >= targetUs - toleranceUs).length;
}

function trimRealtimeCache(st: AssetState, targetUs: number): void {
  const frameDurationUs = st.realtime?.frameDurationUs ?? estimateFrameDurationUs(st);
  const { minUs, maxUs } = realtimeCacheWindowUs(
    targetUs,
    frameDurationUs,
    REALTIME_CACHE_BEHIND_FRAMES,
    REALTIME_CACHE_HIGH_WATER_FRAMES,
  );

  for (let i = st.frameCache.length - 1; i >= 0; i--) {
    const frame = st.frameCache[i];
    if (frame.timestamp >= minUs && frame.timestamp <= maxUs) continue;
    const [removed] = st.frameCache.splice(i, 1);
    try { removed.videoFrame.close(); } catch { /* ignore */ }
  }

  while (st.frameCache.length > REALTIME_CACHE_BEHIND_FRAMES + REALTIME_CACHE_HIGH_WATER_FRAMES + 4) {
    const farthestIdx = farthestCachedFrameIndex(st, targetUs);
    const [removed] = st.frameCache.splice(farthestIdx, 1);
    try { removed.videoFrame.close(); } catch { /* ignore */ }
  }
}

function farthestCachedFrameIndex(st: AssetState, targetUs: number): number {
  let idx = 0;
  let distance = -1;
  for (let i = 0; i < st.frameCache.length; i++) {
    const nextDistance = Math.abs(st.frameCache[i].timestamp - targetUs);
    if (nextDistance > distance) {
      distance = nextDistance;
      idx = i;
    }
  }
  return idx;
}

async function getFrameAtTime(
  st: AssetState,
  targetUs: number,
  quality: VideoPreviewQuality,
): Promise<VideoFrame | null> {
  const cached = cloneCachedFrame(st, targetUs);
  if (cached) return cached;

  let keyTimeUs = nearestKeyframeBefore(st.keyframeTimesUs, targetUs);

  while (keyTimeUs >= 0) {
    const frames = await decodeSliceFromKey(
      st,
      keyTimeUs,
      targetUs,
      quality,
    );

    if (frames.length > 0) {
      cacheDecodedFrames(
        st,
        frames,
        targetUs,
        Math.min(st.maxPtsUs, targetUs + seekFeedMarginUs(quality)),
      );
      const decoded = cloneCachedFrame(st, targetUs);
      if (decoded) return decoded;
    }

    keyTimeUs = previousKeyframeBefore(st.keyframeTimesUs, keyTimeUs);
  }

  return null;
}

async function decodeSliceFromKey(
  st: AssetState,
  keyTimeUs: number,
  targetUs: number,
  quality: VideoPreviewQuality,
): Promise<DecodedFrame[]> {
  const keyIdx = keyChunkIndexAtTime(st.chunks, keyTimeUs);
  if (keyIdx < 0) return [];

  const endIdx = decodeSliceEndIndex(
    st.chunks,
    keyIdx,
    targetUs,
    seekFeedMarginUs(quality),
    DECODE_SLICE_LOOKAHEAD_CHUNKS,
  );
  if (endIdx < keyIdx) return [];

  const coverUs = Math.min(
    st.maxPtsUs,
    targetUs + seekFeedMarginUs(quality),
  );
  const collector: DecodeCollector = {
    frames: [],
    outputCount: 0,
    closedCount: 0,
    minUs: Number.POSITIVE_INFINITY,
    maxUs: Number.NEGATIVE_INFINITY,
  };
  let errorMessage: string | null = null;

  const decoder = new VideoDecoder({
    output(frame) {
      collectDecodedFrame(collector, frame, targetUs, coverUs);
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

  if (errorMessage === null && !collectorHasFrameAtOrPast(collector, coverUs)) {
    await waitForTargetFrame(decoder, collector, coverUs, OUTPUT_WAIT_TIMEOUT_MS);
  }

  if (errorMessage === null && !collectorHasFrameAtOrPast(collector, coverUs)) {
    const flushed = await flushDecoderWithTimeout(decoder, FLUSH_TIMEOUT_MS);
    if (!flushed) {
      errorMessage = `flush timeout after ${FLUSH_TIMEOUT_MS}ms`;
    }
  }

  try { decoder.close(); } catch { /* ignore */ }

  return collector.frames;
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
  coverUs: number,
): void {
  collector.outputCount++;
  collector.minUs = Math.min(collector.minUs, frame.timestamp);
  collector.maxUs = Math.max(collector.maxUs, frame.timestamp);

  if (
    shouldCacheDecodedFrame(
      frame.timestamp,
      targetUs,
      coverUs,
      CACHE_BEHIND_TARGET_US,
      CACHE_HIT_TOLERANCE_US,
    )
  ) {
    collector.frames.push({ timestamp: frame.timestamp, videoFrame: frame });
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
  coverUs: number,
): void {
  for (const frame of frames) {
    if (
      !shouldCacheDecodedFrame(
        frame.timestamp,
        targetUs,
        coverUs,
        CACHE_BEHIND_TARGET_US,
        CACHE_HIT_TOLERANCE_US,
      )
    ) {
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
