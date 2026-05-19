// ── Video Frame Decoder (WebCodecs + mp4box.js) ──
// Uses mp4box.js for demuxing and WebCodecs VideoDecoder for
// on-demand frame decoding with a small LRU cache.
//
// Flow:
//   1. prepareVideoSource: fetch → mp4box demux → store encoded chunks + metadata
//   2. getDecodedFrameRgba: on-demand decode at target time, with caching
//   3. Cache is a small LRU (max ~30 decoded frames per video)

import { createFile } from 'mp4box';

// ── Types ──

export interface VideoSourceMeta {
  width: number;
  height: number;
  durationSecs: number | null;
}

interface EncodedChunkDesc {
  type: EncodedVideoChunkType;
  timestamp: number;  // microseconds (CTS)
  duration: number;   // microseconds
  data: ArrayBuffer;
}

interface VideoSource {
  encodedChunks: EncodedChunkDesc[];
  codec: string;
  description?: Uint8Array;
  width: number;
  height: number;
  durationSecs: number | null;
  // Persistent decoder (one per video, never closed — aligns with engine)
  decoder: VideoDecoder | null;
  decoderKeyIdx: number; // keyframe index the decoder was last started from
}

interface CachedFrame {
  rgba: Uint8Array;
  timestamp: number; // presentation timestamp in us
}

// ── State ──

const videoSources = new Map<string, VideoSource>();

// LRU cache: url → Map<timestamp_us, CachedFrame>
const decodedCache = new Map<string, Map<number, CachedFrame>>();
const lruKeys: string[] = []; // url:ts strings for LRU eviction
const MAX_CACHE_ENTRIES = 30;

// ── Public API ──

/**
 * Demux a video and store encoded chunks + metadata.
 * Does NOT decode any frames — decoding is done on-demand.
 */
export async function prepareVideoSource(
  url: string,
  buffer: ArrayBuffer,
): Promise<VideoSourceMeta> {
  const existing = videoSources.get(url);
  if (existing) {
    return { width: existing.width, height: existing.height, durationSecs: existing.durationSecs };
  }

  if (typeof VideoDecoder === 'undefined') {
    throw new Error('WebCodecs VideoDecoder not supported');
  }

  // Extract codec description (avcC) from raw mp4 bytes
  const description = extractCodecDescription(buffer);

  // Demux with mp4box.js
  const { encodedChunks, codec, width, height, timescale } =
    await demuxWithMp4Box(buffer);

  // Compute duration from max composition timestamp
  let maxCtsUs = 0;
  for (const chunk of encodedChunks) {
    const endUs = chunk.timestamp + chunk.duration;
    if (endUs > maxCtsUs) maxCtsUs = endUs;
  }
  const durationSecs = maxCtsUs > 0 ? maxCtsUs / 1_000_000 : null;

  const source: VideoSource = {
    encodedChunks,
    codec,
    description,
    width,
    height,
    durationSecs,
    decoder: null,
    decoderKeyIdx: -1,
  };
  videoSources.set(url, source);

  return { width, height, durationSecs };
}

/**
 * Decode and return a raw VideoFrame nearest to targetTimeSecs.
 * Uses a persistent VideoDecoder per source (one per video, never closed).
 * This aligns with the engine's approach of reusing a single ffmpeg instance.
 */
export async function getDecodedVideoFrame(
  url: string,
  targetTimeSecs: number,
): Promise<VideoFrame | null> {
  const source = videoSources.get(url);
  if (!source) return null;

  const targetUs = Math.max(0, targetTimeSecs * 1_000_000);
  let keyIdx = findKeyframeBefore(source.encodedChunks, targetUs);
  const triedKeyframes: number[] = [];

  // Retry loop: if a keyframe fails (e.g., non-IDR Open GOP), try the
  // previous true IDR keyframe. Mirrors ffmpeg's robust seek behavior.
  let result: VideoFrame | null = null;
  while (keyIdx >= 0 && result === null) {
    triedKeyframes.push(keyIdx);

    // Find end chunk within a margin
    const marginUs = 500_000;
    let endIdx = keyIdx;
    for (let i = keyIdx + 1; i < source.encodedChunks.length; i++) {
      if (source.encodedChunks[i].timestamp <= targetUs + marginUs) {
        endIdx = i;
      } else {
        break;
      }
    }

    const chunkSlice = source.encodedChunks.slice(keyIdx, endIdx + 1);
    const firstChunk = chunkSlice[0];

    // Ensure decoder is alive and configured for this keyframe
    if (!source.decoder || source.decoderKeyIdx !== keyIdx) {
      if (source.decoder) {
        try { source.decoder.close(); } catch { /* ignore */ }
        source.decoder = null;
      }
      source.decoder = createConfiguredDecoder(source.codec, source.description);
      if (!source.decoder) return null;
      source.decoderKeyIdx = keyIdx;
    }

    // Decode the chunk range through the persistent decoder
    pendingFrames = [];
    const decodedFrames = await feedAndFlushDecoder(source.decoder, chunkSlice);

    if (decodedFrames.length > 0) {
      // Success — find frame closest to target
      let bestIdx = 0;
      let bestDiff = Infinity;
      for (let i = 0; i < decodedFrames.length; i++) {
        const diff = Math.abs(decodedFrames[i].timestamp - targetUs);
        if (diff < bestDiff) {
          bestDiff = diff;
          bestIdx = i;
        }
      }
      const best = decodedFrames[bestIdx];
      for (let i = 0; i < decodedFrames.length; i++) {
        if (i !== bestIdx) decodedFrames[i].videoFrame.close();
      }
      result = best.videoFrame;
      break;
    }

    // Decode failed — decoder is now closed, destroy it
    try { source.decoder.close(); } catch { /* ignore */ }
    source.decoder = null;
    source.decoderKeyIdx = -1;

    // Check NAL type of the first chunk to understand why
    const firstData = firstChunk ? new Uint8Array(firstChunk.data.slice(0, Math.min(64, firstChunk.data.byteLength))) : null;
    const nalTypes: string[] = [];
    if (firstData && firstData.length >= 9) {
      let offset = 0;
      while (offset + 4 < firstData.length) {
        const len = (firstData[offset] << 24) | (firstData[offset + 1] << 16) | (firstData[offset + 2] << 8) | firstData[offset + 3];
        if (len === 0 || offset + 4 + len > firstData.length) break;
        const nalType = firstData[offset + 4] & 0x1F;
        nalTypes.push(`${nalType}`);
        offset += 4 + len;
      }
    }

    // If first NAL is not IDR (type 5), this is an Open GOP keyframe.
    // WebCodecs requires true IDR after configure/flush. Skip to previous IDR.
    const hasIDR = nalTypes.includes('5');
    const descHex = source.description ? Array.from(source.description.slice(0, 16)).map(b => b.toString(16).padStart(2, '0')).join(' ') : 'none';

    if (!hasIDR && nalTypes.length > 0) {
      console.warn(`[video-decoder] Open GOP keyframe at idx=${keyIdx} (nalTypes=[${nalTypes.join(',')}], no IDR), falling back to previous keyframe`);
    } else {
      const firstHex = firstData ? Array.from(firstData.slice(0, 16)).map(b => b.toString(16).padStart(2, '0')).join(' ') : 'n/a';
      console.warn(`[video-decoder] no frames decoded: keyIdx=${keyIdx}/${source.encodedChunks.length} targetUs=${Math.round(targetUs)} firstTs=${firstChunk?.timestamp} firstType=${firstChunk?.type} dataLen=${firstChunk?.data.byteLength} nalTypes=[${nalTypes.join(',')}] firstHex=${firstHex} descHex=${descHex}`);
    }

    // Find previous keyframe
    let prevKey = -1;
    for (let i = keyIdx - 1; i >= 0; i--) {
      if (source.encodedChunks[i].type === 'key') {
        prevKey = i;
        break;
      }
    }
    if (prevKey < 0) break;
    keyIdx = prevKey;
  }

  return result;
}

/**
 * Get RGBA pixels for a video frame (uses canvas extraction).
 * Used for WASM injection and fallback paths.
 */
export async function getDecodedFrameRgba(
  url: string,
  targetTimeSecs: number,
): Promise<{ rgba: Uint8Array; width: number; height: number } | null> {
  const source = videoSources.get(url);
  if (!source) return null;

  const targetUs = Math.max(0, targetTimeSecs * 1_000_000);

  // Check RGBA cache
  const cached = getCachedFrame(url, targetUs);
  if (cached) {
    return { rgba: cached.rgba, width: source.width, height: source.height };
  }

  // Decode via getDecodedVideoFrame (single frame, no batch extraction)
  const frame = await getDecodedVideoFrame(url, targetTimeSecs);
  if (!frame) return null;

  // Extract RGBA via canvas (one-time cost for this frame)
  const offscreen = new OffscreenCanvas(source.width, source.height);
  const ctx = offscreen.getContext('2d', { willReadFrequently: true })!;
  ctx.drawImage(frame, 0, 0);
  const imageData = ctx.getImageData(0, 0, source.width, source.height);
  const rgba = new Uint8Array(imageData.data.buffer.slice(0));
  frame.close();

  // Cache
  putCachedFrame(url, Math.round(targetUs), { rgba, timestamp: Math.round(targetUs) });

  return { rgba, width: source.width, height: source.height };
}

export function registerVideoGlobals(): void {
  // No-op — kept for API compat with main.ts import.
}

export function getVideoDimensions(url: string): { width: number; height: number } | null {
  const source = videoSources.get(url);
  return source ? { width: source.width, height: source.height } : null;
}

export function getVideoDurationSecs(url: string): number | null {
  return videoSources.get(url)?.durationSecs ?? null;
}

export function clearVideoCache(url?: string): void {
  if (url) {
    const source = videoSources.get(url);
    if (source?.decoder) {
      try { source.decoder.close(); } catch { /* ignore */ }
    }
    videoSources.delete(url);
    decodedCache.delete(url);
  } else {
    for (const source of videoSources.values()) {
      if (source.decoder) {
        try { source.decoder.close(); } catch { /* ignore */ }
      }
    }
    videoSources.clear();
    decodedCache.clear();
  }
  lruKeys.length = 0;
}

// ── Time resolution (matching Rust resolve_time_secs) ──

export interface VideoFrameTiming {
  mediaOffsetSecs: number;
  playbackRate: number;
  looping: boolean;
}

export function resolveVideoTimeSecs(
  compositionTimeSecs: number,
  timing: VideoFrameTiming,
  durationSecs: number | null,
): number {
  const ct = Math.max(0, compositionTimeSecs);
  const localTime = timing.mediaOffsetSecs + ct * timing.playbackRate;

  if (!timing.looping) {
    return clampVideoTime(localTime, durationSecs);
  }

  if (durationSecs !== null && durationSecs > timing.mediaOffsetSecs) {
    const playable = durationSecs - timing.mediaOffsetSecs;
    const wrapped = (ct * timing.playbackRate) % playable;
    return timing.mediaOffsetSecs + wrapped;
  }

  return clampVideoTime(localTime, durationSecs);
}

// ── LRU cache ──

function cacheKey(url: string, timestamp: number): string {
  return `${url}::${timestamp}`;
}

function getCachedFrame(url: string, targetUs: number): CachedFrame | null {
  const cache = decodedCache.get(url);
  if (!cache || cache.size === 0) return null;

  // Find closest cached frame within a reasonable tolerance
  let best: CachedFrame | null = null;
  let bestDiff = Infinity;
  const toleranceUs = 100_000; // 100ms tolerance

  for (const [ts, frame] of cache) {
    const diff = Math.abs(ts - targetUs);
    if (diff < toleranceUs && diff < bestDiff) {
      bestDiff = diff;
      best = frame;
    }
  }

  if (best) {
    // Bump in LRU
    touchLru(url, best.timestamp);
  }

  return best;
}

function putCachedFrame(url: string, timestamp: number, frame: CachedFrame): void {
  let cache = decodedCache.get(url);
  if (!cache) {
    cache = new Map();
    decodedCache.set(url, cache);
  }

  const old = cache.get(timestamp);
  if (old) {
    // Replace existing entry, bump LRU
    cache.set(timestamp, frame);
    touchLru(url, timestamp);
    return;
  }

  // Evict if at capacity
  while (lruKeys.length >= MAX_CACHE_ENTRIES) {
    const oldest = lruKeys.shift()!;
    const [evictUrl, evictTsStr] = oldest.split('::');
    const evictTs = Number(evictTsStr);
    const evictCache = decodedCache.get(evictUrl);
    if (evictCache) {
      evictCache.delete(evictTs);
      if (evictCache.size === 0) {
        decodedCache.delete(evictUrl);
      }
    }
  }

  cache.set(timestamp, frame);
  lruKeys.push(cacheKey(url, timestamp));
}

function touchLru(url: string, timestamp: number): void {
  const key = cacheKey(url, timestamp);
  const idx = lruKeys.indexOf(key);
  if (idx >= 0) {
    lruKeys.splice(idx, 1);
  }
  lruKeys.push(key);
}

// ── On-demand decode ──

interface DecodedFrame {
  timestamp: number; // microseconds
  videoFrame: VideoFrame;
}

// ── Persistent decoder helpers (aligns with engine: one decoder per video) ──

// Shared accumulator for frames from the persistent decoder output callback.
// Cleared before each decode batch, populated during decode+flush.
let pendingFrames: DecodedFrame[] = [];

function createConfiguredDecoder(
  codec: string,
  description: Uint8Array | undefined,
): VideoDecoder | null {
  const decoder = new VideoDecoder({
    output(frame: VideoFrame) {
      pendingFrames.push({ timestamp: frame.timestamp, videoFrame: frame });
    },
    error(err: Error) {
      console.warn(`[video-decoder] persistent decoder error: ${err.message}`);
    },
  });

  const config: VideoDecoderConfig = { codec };
  if (description && description.length > 0) {
    config.description = description;
  }

  try {
    decoder.configure(config);
  } catch (err: any) {
    console.warn(`[video-decoder] configure failed: ${err.message}`);
    try { decoder.close(); } catch { /* ignore */ }
    return null;
  }

  return decoder;
}

function feedAndFlushDecoder(
  decoder: VideoDecoder,
  chunks: EncodedChunkDesc[],
): Promise<DecodedFrame[]> {
  return new Promise((resolve) => {
    let errored = false;
    let settled = false;

    const finish = () => {
      if (settled) return;
      settled = true;
      const frames = pendingFrames.slice(); // snapshot
      frames.sort((a, b) => a.timestamp - b.timestamp);
      resolve(frames);
    };

    // Feed chunks
    for (const chunk of chunks) {
      if (errored) break;
      try {
        const encodedChunk = new EncodedVideoChunk({
          type: chunk.type,
          timestamp: chunk.timestamp,
          duration: chunk.duration,
          data: chunk.data.slice(0) as ArrayBuffer,
        });
        decoder.decode(encodedChunk);
      } catch (err: any) {
        console.warn(`[video-decoder] chunk decode error: ${err.message}`);
        errored = true;
        break;
      }
    }

    // Timeout guard
    const TIMEOUT_MS = 8000;
    const timer = setTimeout(() => {
      console.warn('[video-decoder] decode timed out');
      finish();
    }, TIMEOUT_MS);

    decoder.flush().then(() => {
      clearTimeout(timer);
      finish();
    }).catch(() => {
      clearTimeout(timer);
      finish();
    });
  });
}

function findKeyframeBefore(chunks: EncodedChunkDesc[], targetUs: number): number {
  let keyIdx = 0;
  for (let i = 0; i < chunks.length; i++) {
    if (chunks[i].type === 'key' && chunks[i].timestamp <= targetUs) {
      keyIdx = i;
    }
    if (chunks[i].timestamp > targetUs) break;
  }
  return keyIdx;
}

// ── Time helpers ──

function clampVideoTime(timeSecs: number, durationSecs: number | null): number {
  const clamped = Math.max(0, timeSecs);
  if (durationSecs !== null && durationSecs > 0) {
    return Math.min(clamped, durationSecs);
  }
  return clamped;
}

// ── Codec Description Extraction ──
// (unchanged from original — walks MP4 box tree to find avcC/hvcC)

function extractCodecDescription(mp4Data: ArrayBuffer): Uint8Array | undefined {
  const view = new DataView(mp4Data);
  const moovOffset = findBox(view, 0, 'moov');
  if (moovOffset < 0) return undefined;
  return findAvcCInMoov(view, moovOffset);
}

function findBox(view: DataView, startOffset: number, targetType: string): number {
  let offset = startOffset;
  while (offset + 8 <= view.byteLength) {
    let size = view.getUint32(offset);
    const type = readFourCC(view, offset + 4);
    if (size === 0) size = view.byteLength - offset;
    if (size < 8) return -1;
    if (type === targetType) return offset;
    offset += size;
  }
  return -1;
}

function findAvcCInMoov(view: DataView, moovOffset: number): Uint8Array | undefined {
  let moovSize = view.getUint32(moovOffset);
  if (moovSize === 0) moovSize = view.byteLength - moovOffset;

  let offset = moovOffset + 8;
  const moovEnd = moovOffset + moovSize;

  while (offset + 8 <= moovEnd) {
    let size = view.getUint32(offset);
    if (size === 0) size = moovEnd - offset;
    if (size < 8) { offset += size; continue; }
    const type = readFourCC(view, offset + 4);

    if (type === 'trak') {
      const result = findAvcCInTrak(view, offset, offset + size);
      if (result) return result;
    }
    offset += size;
  }
  return undefined;
}

function findAvcCInTrak(view: DataView, trakStart: number, trakEnd: number): Uint8Array | undefined {
  let offset = trakStart + 8;
  while (offset + 8 <= trakEnd) {
    let size = view.getUint32(offset);
    if (size === 0) size = trakEnd - offset;
    if (size < 8) { offset += size; continue; }
    const type = readFourCC(view, offset + 4);

    if (type === 'mdia') {
      return findAvcCInMdia(view, offset, offset + size);
    }
    offset += size;
  }
  return undefined;
}

function findAvcCInMdia(view: DataView, mdiaStart: number, mdiaEnd: number): Uint8Array | undefined {
  let offset = mdiaStart + 8;
  while (offset + 8 <= mdiaEnd) {
    let size = view.getUint32(offset);
    if (size === 0) size = mdiaEnd - offset;
    if (size < 8) { offset += size; continue; }
    const type = readFourCC(view, offset + 4);

    if (type === 'minf') {
      return findAvcCInMinf(view, offset, offset + size);
    }
    offset += size;
  }
  return undefined;
}

function findAvcCInMinf(view: DataView, minfStart: number, minfEnd: number): Uint8Array | undefined {
  let offset = minfStart + 8;
  while (offset + 8 <= minfEnd) {
    let size = view.getUint32(offset);
    if (size === 0) size = minfEnd - offset;
    if (size < 8) { offset += size; continue; }
    const type = readFourCC(view, offset + 4);

    if (type === 'stbl') {
      return findAvcCInStbl(view, offset, offset + size);
    }
    offset += size;
  }
  return undefined;
}

function findAvcCInStbl(view: DataView, stblStart: number, stblEnd: number): Uint8Array | undefined {
  let offset = stblStart + 8;
  while (offset + 8 <= stblEnd) {
    let size = view.getUint32(offset);
    if (size === 0) size = stblEnd - offset;
    if (size < 8) { offset += size; continue; }
    const type = readFourCC(view, offset + 4);

    if (type === 'stsd') {
      return findAvcCInStsd(view, offset, offset + size);
    }
    offset += size;
  }
  return undefined;
}

function findAvcCInStsd(view: DataView, stsdStart: number, stsdEnd: number): Uint8Array | undefined {
  let offset = stsdStart + 16;

  while (offset + 8 <= stsdEnd) {
    const entrySize = view.getUint32(offset);
    if (entrySize < 8) { offset += entrySize || 4; continue; }
    const entryType = readFourCC(view, offset + 4);

    if (entryType === 'avc1' || entryType === 'avc3' ||
        entryType === 'hvc1' || entryType === 'hev1') {
      const boxStart = offset + 8 + 78;
      let innerOffset = boxStart;

      while (innerOffset + 8 <= offset + entrySize) {
        const innerSize = view.getUint32(innerOffset);
        if (innerSize < 8) break;
        const innerType = readFourCC(view, innerOffset + 4);

        if (innerType === 'avcC' || innerType === 'hvcC') {
          const dataLen = innerSize - 8;
          const data = new Uint8Array(view.buffer.slice(innerOffset + 8, innerOffset + 8 + dataLen));
          return data;
        }
        innerOffset += innerSize;
      }
    }
    offset += entrySize;
  }
  return undefined;
}

function readFourCC(view: DataView, offset: number): string {
  return String.fromCharCode(
    view.getUint8(offset),
    view.getUint8(offset + 1),
    view.getUint8(offset + 2),
    view.getUint8(offset + 3),
  );
}

// ── Mp4Box Demux ──

function demuxWithMp4Box(
  mp4Data: ArrayBuffer,
): Promise<{
  encodedChunks: EncodedChunkDesc[];
  codec: string;
  width: number;
  height: number;
  timescale: number;
}> {
  return new Promise((resolve, reject) => {
    const chunks: EncodedChunkDesc[] = [];
    let resolved = false;

    const file = createFile();

    file.onSamples = (_id: number, _user: any, samples: any[]) => {
      for (const sample of samples) {
        chunks.push({
          type: sample.is_sync ? 'key' : 'delta',
          timestamp: Math.round(
            (sample.cts / sample.timescale) * 1_000_000,
          ),
          duration: Math.round(
            (sample.duration / sample.timescale) * 1_000_000,
          ),
          data: sample.data instanceof ArrayBuffer
            ? sample.data
            : new Uint8Array(sample.data).buffer,
        });
      }
    };

    file.onReady = (info: any) => {
      if (resolved) return;
      const videoTrack = info.tracks?.find(
        (t: any) => t.type === 'video' || t.video,
      );
      if (!videoTrack) {
        reject(new Error('No video track found'));
        resolved = true;
        return;
      }

      const codec = videoTrack.codec || '';
      const width =
        videoTrack.video?.width || videoTrack.track_width || 0;
      const height =
        videoTrack.video?.height || videoTrack.track_height || 0;
      const timescale = videoTrack.timescale || 1;

      file.setExtractionOptions(videoTrack.id, null, {
        nbSamples: Infinity,
      });
      file.start();

      resolved = true;
      resolve({ encodedChunks: chunks, codec, width, height, timescale });
    };

    file.onError = (e: any) => {
      if (resolved) return;
      resolved = true;
      reject(new Error(`mp4box error: ${e}`));
    };

    const buf = mp4Data as any;
    buf.fileStart = 0;
    file.appendBuffer(buf);
    file.flush();
  });
}
