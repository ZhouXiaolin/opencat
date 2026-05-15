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
  };
  videoSources.set(url, source);

  return { width, height, durationSecs };
}

/**
 * Decode and return a raw VideoFrame nearest to targetTimeSecs.
 * Caller is responsible for closing the returned VideoFrame.
 * No canvas extraction, no RGBA — use with createImageBitmap for GPU path.
 */
export async function getDecodedVideoFrame(
  url: string,
  targetTimeSecs: number,
): Promise<VideoFrame | null> {
  const source = videoSources.get(url);
  if (!source) return null;

  const targetUs = Math.max(0, targetTimeSecs * 1_000_000);

  // Find keyframe before target
  const keyIdx = findKeyframeBefore(source.encodedChunks, targetUs);

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

  // Decode the range
  const decodedFrames = await decodeChunkRange(
    source.codec,
    source.description,
    source.encodedChunks.slice(keyIdx, endIdx + 1),
  );

  if (decodedFrames.length === 0) return null;

  // Find frame closest to target
  let bestIdx = 0;
  let bestDiff = Infinity;
  for (let i = 0; i < decodedFrames.length; i++) {
    const diff = Math.abs(decodedFrames[i].timestamp - targetUs);
    if (diff < bestDiff) {
      bestDiff = diff;
      bestIdx = i;
    }
  }

  // Close all other frames, return best
  const best = decodedFrames[bestIdx];
  for (let i = 0; i < decodedFrames.length; i++) {
    if (i !== bestIdx) decodedFrames[i].videoFrame.close();
  }

  return best.videoFrame;
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

/**
 * Synchronous lookup in the decode cache (for backward compat).
 */
export function decodeVideoFrameSync(url: string, frame: number): Uint8Array | null {
  return null; // on-demand mode — sync lookup is no longer supported
}

export function registerVideoGlobals(): void {
  (window as any).__video_decode_frame_sync = decodeVideoFrameSync;
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
    videoSources.delete(url);
    decodedCache.delete(url);
  } else {
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

function decodeChunkRange(
  codec: string,
  description: Uint8Array | undefined,
  chunks: EncodedChunkDesc[],
): Promise<DecodedFrame[]> {
  return new Promise((resolve, reject) => {
    const frames: DecodedFrame[] = [];

    const decoder = new VideoDecoder({
      output(frame: VideoFrame) {
        frames.push({ timestamp: frame.timestamp, videoFrame: frame });
      },
      error(err: Error) {
        console.warn(`[video-decoder] decode error: ${err.message}`);
        // Don't reject — collect whatever frames we got
      },
    });

    const config: VideoDecoderConfig = { codec };
    if (description && description.length > 0) {
      config.description = description;
    }

    try {
      decoder.configure(config);
    } catch (err: any) {
      reject(new Error(`VideoDecoder configure failed: ${err.message}`));
      return;
    }

    for (const chunk of chunks) {
      const encodedChunk = new EncodedVideoChunk({
        type: chunk.type,
        timestamp: chunk.timestamp,
        duration: chunk.duration,
        // Copy buffer — EncodedVideoChunk transfers ownership (detaches original)
        data: chunk.data.slice(0) as ArrayBuffer,
      });
      try {
        decoder.decode(encodedChunk);
      } catch (err: any) {
        console.warn(`[video-decoder] chunk decode error: ${err.message}`);
      }
    }

    decoder.flush().then(() => {
      // Sort by timestamp (presentation order)
      frames.sort((a, b) => a.timestamp - b.timestamp);
      resolve(frames);
      decoder.close();
    }).catch((err) => {
      // Still resolve with whatever frames we got
      frames.sort((a, b) => a.timestamp - b.timestamp);
      resolve(frames);
      decoder.close();
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
