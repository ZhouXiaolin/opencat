// ── Web Resource Loader ──
// Acts as the web-side "ResourceAcquisition" trait.
// Handles: image loading, video decoding via ffmpeg.wasm, audio fetching.

import type { LoadedImage, ResourceRequests } from './types';

let CanvasKit: any = null;
let ffmpeg: any = null;
let ffmpegLoaded = false;

// ── Resource Cache ──

const imageCache = new Map<string, LoadedImage>();

export function setCanvasKit(ck: any): void {
  CanvasKit = ck;
}

export function getCachedImage(assetId: string): LoadedImage | undefined {
  return imageCache.get(assetId);
}

// ── Image Loading ──

export async function loadImage(assetId: string, urlOrPath: string): Promise<LoadedImage> {
  const cached = imageCache.get(assetId);
  if (cached) return cached;

  const resp = await fetch(urlOrPath);
  const blob = await resp.blob();
  const imageBitmap = await createImageBitmap(blob);

  if (!CanvasKit) throw new Error('CanvasKit not initialized');

  const ckImage = CanvasKit.MakeImageFromEncoded(
    new Uint8Array(await blob.arrayBuffer()),
  );

  if (!ckImage) throw new Error(`Failed to decode image: ${urlOrPath}`);

  const loaded: LoadedImage = {
    path: urlOrPath,
    ckImage,
    width: ckImage.width(),
    height: ckImage.height(),
  };
  imageCache.set(assetId, loaded);
  return loaded;
}

export async function loadImages(
  requests: ResourceRequests,
  baseUrl?: string,
): Promise<void> {
  const promises = requests.images.map(async (url) => {
    const assetId = url.startsWith('http') ? `url:${url}` : url;
    if (imageCache.has(assetId)) return;
    try {
      const fullUrl = baseUrl ? new URL(url, baseUrl).href : url;
      await loadImage(assetId, fullUrl);
    } catch (err) {
      console.warn(`[resource] Failed to load image: ${url}`, err);
    }
  });
  await Promise.all(promises);
}

// ── FFmpeg.wasm Video Decoder ──

export async function initFFmpegDecoder(): Promise<void> {
  if (ffmpegLoaded) return;
  const { FFmpeg } = await import('@ffmpeg/ffmpeg');
  const { toBlobURL } = await import('@ffmpeg/util');
  ffmpeg = new FFmpeg();
  ffmpeg.on('log', ({ message }: { message: string }) => {
    if (message.includes('Error') || message.includes('error')) {
      console.warn('[ffmpeg-decoder]', message);
    }
  });
  const baseURL = 'https://unpkg.com/@ffmpeg/core@0.12.10/dist/esm';
  await ffmpeg.load({
    coreURL: await toBlobURL(`${baseURL}/ffmpeg-core.js`, 'text/javascript'),
    wasmURL: await toBlobURL(`${baseURL}/ffmpeg-core.wasm`, 'application/wasm'),
  });
  ffmpegLoaded = true;
}

export async function getFFmpeg(): Promise<any> {
  if (!ffmpegLoaded) await initFFmpegDecoder();
  return ffmpeg;
}

export interface DecodedVideoFrame {
  pixels: Uint8Array;
  width: number;
  height: number;
  pts: number;
}

export async function decodeVideoAtTime(
  videoUrl: string,
  timeSecs: number,
  targetWidth?: number,
  targetHeight?: number,
): Promise<DecodedVideoFrame | null> {
  const ff = await getFFmpeg();

  const inputName = 'input_video.mp4';
  const outputName = `frame_rgba_${Date.now()}.raw`;

  try {
    // Fetch video
    const resp = await fetch(videoUrl);
    const videoData = new Uint8Array(await resp.arrayBuffer());
    await ff.writeFile(inputName, videoData);

    // Seek and decode a single frame
    const args = [
      '-ss', String(timeSecs),
      '-i', inputName,
      '-vframes', '1',
      '-f', 'rawvideo',
      '-pix_fmt', 'rgba',
    ];
    if (targetWidth && targetHeight) {
      args.push('-s', `${targetWidth}x${targetHeight}`);
    }
    args.push('-y', outputName);

    await ff.exec(args);

    const data: Uint8Array = await ff.readFile(outputName);

    // Get video info for dimensions
    const probeArgs = [
      '-i', inputName,
      '-f', 'null',
      '-',
    ];
    // We'll use a simpler approach: read the raw data
    // Raw RGBA: width * height * 4 bytes
    // We don't know dimensions without probing, so let's probe first
    const probe = await ff.exec([
      '-i', inputName,
      '-vf', 'showinfo',
      '-f', 'null',
      '-',
    ]);

    // Cleanup
    await ff.deleteFile(inputName);
    await ff.deleteFile(outputName);

    return {
      pixels: data,
      width: targetWidth || 0,
      height: targetHeight || 0,
      pts: timeSecs,
    };
  } catch (err) {
    console.warn(`[ffmpeg] Failed to decode frame at ${timeSecs}s:`, err);
    // Cleanup on error
    try { await ff.deleteFile(inputName); } catch { }
    try { await ff.deleteFile(outputName); } catch { }
    return null;
  }
}

export async function getVideoMetadata(
  videoUrl: string,
): Promise<{ width: number; height: number; duration: number } | null> {
  const ff = await getFFmpeg();
  const inputName = 'meta_video.mp4';

  try {
    const resp = await fetch(videoUrl);
    const data = new Uint8Array(await resp.arrayBuffer());
    await ff.writeFile(inputName, data);

    const probeResult = await ff.exec([
      '-i', inputName,
      '-f', 'null',
      '-',
    ]);

    await ff.deleteFile(inputName);

    const output = probeResult as unknown as string;
    const wMatch = output.match(/(\d+)x(\d+)/);
    const dMatch = output.match(/Duration: (\d+):(\d+):(\d+\.\d+)/);

    let width = 0, height = 0;
    if (wMatch) {
      width = parseInt(wMatch[1]);
      height = parseInt(wMatch[2]);
    }

    let duration = 0;
    if (dMatch) {
      duration = parseInt(dMatch[1]) * 3600 + parseInt(dMatch[2]) * 60 + parseFloat(dMatch[3]);
    }

    return { width, height, duration };
  } catch (err) {
    console.warn('[ffmpeg] Failed to probe video:', err);
    try { await ff.deleteFile(inputName); } catch { }
    return null;
  }
}

// ── Icon Loading (Lucide SVGs) ──

const iconSvgCache = new Map<string, string>();

export async function loadLucideIcon(name: string): Promise<string | null> {
  const cached = iconSvgCache.get(name);
  if (cached !== undefined) return cached;

  try {
    const resp = await fetch(`/lucide/${name}.svg`);
    if (!resp.ok) {
      iconSvgCache.set(name, '');
      return null;
    }
    const svg = await resp.text();
    iconSvgCache.set(name, svg);
    return svg;
  } catch {
    iconSvgCache.set(name, '');
    return null;
  }
}

export function getCachedIconSvg(name: string): string | undefined {
  return iconSvgCache.get(name);
}

// ── Cleanup ──

export function clearResourceCache(): void {
  imageCache.clear();
  iconSvgCache.clear();
}
