import type { CompositionInfo } from './types';
import { captureFramePixels, ensureSurface, drawFrame } from './renderer';
import { parseJsonl } from './wasm';

type ProgressCallback = (current: number, total: number) => void;

let ffmpeg: any = null;
let loaded = false;

export async function initFFmpeg(): Promise<void> {
  if (loaded) return;
  const { FFmpeg } = await import('@ffmpeg/ffmpeg');
  const { toBlobURL } = await import('@ffmpeg/util');
  ffmpeg = new FFmpeg();
  ffmpeg.on('log', ({ message }: { message: string }) => {
    if (message.includes('Error') || message.includes('error')) {
      console.warn('[ffmpeg]', message);
    }
  });
  const baseURL = 'https://unpkg.com/@ffmpeg/core@0.12.10/dist/esm';
  await ffmpeg.load({
    coreURL: await toBlobURL(`${baseURL}/ffmpeg-core.js`, 'text/javascript'),
    wasmURL: await toBlobURL(`${baseURL}/ffmpeg-core.wasm`, 'application/wasm'),
  });
  loaded = true;
}

export async function exportMp4(
  jsonlContent: string,
  canvas: HTMLCanvasElement,
  comp: CompositionInfo,
  onProgress: ProgressCallback,
): Promise<Uint8Array | null> {
  if (!ffmpeg || !loaded) {
    console.warn('FFmpeg not loaded');
    return null;
  }

  const { width, height, fps, frames } = comp;

  ensureSurface(canvas, width, height);

  const framePromises: Promise<void>[] = [];
  const chunkSize = Math.min(30, frames);

  // Process frames in chunks to avoid memory issues
  for (let start = 0; start < frames; start += chunkSize) {
    const end = Math.min(start + chunkSize, frames);

    // Write frames as raw rgba files to ffmpeg's virtual FS
    for (let f = start; f < end; f++) {
      const parsed = parseJsonl(jsonlContent);
      drawFrame(parsed, f, comp);

      const pixels = captureFramePixels(width, height);
      if (!pixels) continue;

      const fileName = `frame_${String(f).padStart(6, '0')}.rgba`;
      await ffmpeg.writeFile(fileName, pixels);
      onProgress(f + 1, frames);
    }
  }

  // Build the filter for RGBA to YUV conversion
  const filter = `[0:v]format=rgba,setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709[v]`;

  await ffmpeg.exec([
    '-f', 'image2',
    '-framerate', String(fps),
    '-pattern_type', 'glob',
    '-i', 'frame_*.rgba',
    '-vf', filter,
    '-c:v', 'libx264',
    '-pix_fmt', 'yuv420p',
    '-preset', 'fast',
    '-crf', '18',
    '-y',
    'output.mp4',
  ]);

  const data = await ffmpeg.readFile('output.mp4');

  // Cleanup temp files
  for (let f = 0; f < frames; f++) {
    const fileName = `frame_${String(f).padStart(6, '0')}.rgba`;
    try { await ffmpeg.deleteFile(fileName); } catch { /* ignore */ }
  }
  try { await ffmpeg.deleteFile('output.mp4'); } catch { /* ignore */ }

  return data as Uint8Array;
}

export async function exportPngFrame(
  jsonlContent: string,
  canvas: HTMLCanvasElement,
  comp: CompositionInfo,
  frame: number,
): Promise<void> {
  if (!ffmpeg || !loaded) return;

  const { width, height } = comp;
  ensureSurface(canvas, width, height);

  const parsed = parseJsonl(jsonlContent);
  drawFrame(parsed, frame, comp);

  const pixels = captureFramePixels(width, height);
  if (!pixels) return;

  // Use ffmpeg to convert raw rgba to PNG
  const inName = 'frame.rgba';
  const outName = 'frame.png';
  await ffmpeg.writeFile(inName, pixels);
  await ffmpeg.exec([
    '-f', 'rawvideo',
    '-pixel_format', 'rgba',
    '-video_size', `${width}x${height}`,
    '-i', inName,
    '-frames:v', '1',
    '-y',
    outName,
  ]);

  const data = await ffmpeg.readFile(outName);
  await ffmpeg.deleteFile(inName);
  await ffmpeg.deleteFile(outName);

  downloadBlob(new Blob([data], { type: 'image/png' }), `frame_${String(frame).padStart(4, '0')}.png`);
}

function downloadBlob(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

export function downloadMp4(data: Uint8Array, name: string): void {
  const blob = new Blob([data as BlobPart], { type: 'video/mp4' });
  downloadBlob(blob, name.replace(/\.jsonl$/, '') + '.mp4');
}
