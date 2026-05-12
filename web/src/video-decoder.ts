// ── Video Frame Decoder ──
// Pre-decodes video frames using HTMLVideoElement + OffscreenCanvas,
// then serves them synchronously to the WASM side via decodeVideoFrameSync.

const cachedFrames = new Map<string, Uint8Array>();

// Store video dimensions per URL for dimension queries
const videoDimensions = new Map<string, { width: number; height: number }>();

function frameKey(url: string, frame: number): string {
  return `${url}:${frame}`;
}

/**
 * Pre-decode all frames of a video into RGBA pixel data.
 * Must be called before decodeVideoFrameSync can return frames.
 */
export async function prepareVideoFrames(
  url: string,
  fps: number,
  totalFrames: number,
): Promise<{ width: number; height: number }> {
  // Skip if already decoded
  if (cachedFrames.has(frameKey(url, 0))) {
    const dims = videoDimensions.get(url);
    return dims ?? { width: 0, height: 0 };
  }

  const video = document.createElement('video');
  video.src = url;
  video.preload = 'auto';
  video.crossOrigin = 'anonymous';
  video.muted = true;

  await new Promise<void>((resolve, reject) => {
    video.onloadedmetadata = () => resolve();
    video.onerror = () => reject(new Error(`Failed to load video: ${url}`));
  });

  const width = video.videoWidth;
  const height = video.videoHeight;
  videoDimensions.set(url, { width, height });

  const canvas = new OffscreenCanvas(width, height);
  const ctx = canvas.getContext('2d')!;

  for (let f = 0; f < totalFrames; f++) {
    video.currentTime = f / fps;
    await new Promise<void>((resolve, reject) => {
      video.onseeked = () => resolve();
      video.onerror = () => reject(new Error(`Seek failed at frame ${f}`));
    });
    ctx.drawImage(video, 0, 0);
    const imageData = ctx.getImageData(0, 0, width, height);
    cachedFrames.set(frameKey(url, f), new Uint8Array(imageData.data.buffer.slice(0)));
  }

  return { width, height };
}

/**
 * Synchronous frame lookup — returns RGBA Uint8Array or null.
 * Frames must be preloaded via prepareVideoFrames first.
 */
export function decodeVideoFrameSync(url: string, frame: number): Uint8Array | null {
  return cachedFrames.get(frameKey(url, frame)) ?? null;
}

/**
 * Register global video decode function on window for WASM fallback access.
 * The Rust side can call window.__video_decode_frame_sync(url, frame) via js_sys.
 */
export function registerVideoGlobals(): void {
  (window as any).__video_decode_frame_sync = decodeVideoFrameSync;
}

/**
 * Get cached video dimensions for a previously decoded video.
 */
export function getVideoDimensions(url: string): { width: number; height: number } | null {
  return videoDimensions.get(url) ?? null;
}

/**
 * Clear cached frames for a specific video URL, or all frames if no URL given.
 */
export function clearVideoCache(url?: string): void {
  if (url) {
    for (const key of cachedFrames.keys()) {
      if (key.startsWith(`${url}:`)) {
        cachedFrames.delete(key);
      }
    }
    videoDimensions.delete(url);
  } else {
    cachedFrames.clear();
    videoDimensions.clear();
  }
}
