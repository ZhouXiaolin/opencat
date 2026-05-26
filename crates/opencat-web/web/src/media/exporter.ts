import type { CompositionInfo, ResourceMeta } from '../types';
import { getRendererOrThrow } from '../wasm';
import { injectVideoFramesForRender } from './video-frame-injector';
import { renderEncodedDrawFrame } from '../draw-ir';
import type { IClip } from '@webav/av-cliper';
import type { CanvasKit, Surface } from 'canvaskit-wasm';

type ProgressCallback = (current: number, total: number) => void;
type CanvasKitGlobal = typeof globalThis & { __canvasKit?: CanvasKit };

// ── Custom IClip that renders frames on-demand via CanvasKit ──

class ExportClip implements IClip {
  readonly meta: { width: number; height: number; duration: number };
  readonly ready: Promise<{ width: number; height: number; duration: number }>;

  private canvas: HTMLCanvasElement | OffscreenCanvas;
  private jsonlContent: string;
  private resourceMetaJson: string;
  private comp: CompositionInfo;
  private totalFrames: number;
  private fps: number;
  private sampleRate: number;
  private onProgress: ProgressCallback;
  private audioIds: string[];

  constructor(
    canvas: HTMLCanvasElement | OffscreenCanvas,
    jsonlContent: string,
    resourceMetaJson: string,
    comp: CompositionInfo,
    onProgress: ProgressCallback,
    audioIds: string[],
  ) {
    this.canvas = canvas;
    this.jsonlContent = jsonlContent;
    this.resourceMetaJson = resourceMetaJson;
    this.comp = comp;
    this.totalFrames = comp.frames;
    this.fps = comp.fps;
    this.sampleRate = 48000;
    this.onProgress = onProgress;
    this.audioIds = audioIds;

    this.meta = {
      width: comp.width,
      height: comp.height,
      duration: Math.round((comp.frames / comp.fps) * 1_000_000),
    };
    this.ready = Promise.resolve(this.meta);
  }

  async tick(time: number): Promise<{
    video?: ImageBitmap | null;
    audio?: Float32Array[];
    state: 'done' | 'success';
  }> {
    if (time >= this.meta.duration) {
      return { video: null, audio: [], state: 'done' };
    }

    const frameNum = Math.min(
      Math.floor((time / 1_000_000) * this.fps),
      this.totalFrames - 1,
    );
    const timeSecs = time / 1_000_000;

    const renderer = getRendererOrThrow();
    const CK = (globalThis as CanvasKitGlobal).__canvasKit;
    if (!CK) throw new Error('CanvasKit is not initialized');

    await injectVideoFramesForRender({
      renderer,
      jsonlContent: this.jsonlContent,
      frame: frameNum,
      resourcesJson: this.resourceMetaJson,
      quality: 'exact',
    });

    let surface: Surface | null = null;
    try {
      surface = CK.MakeWebGLCanvasSurface(this.canvas);
      if (!surface) throw new Error('MakeWebGLCanvasSurface failed');

      const ckCanvas = surface.getCanvas();
      const ir = renderer.build_frame_ir(this.jsonlContent, frameNum, this.resourceMetaJson);
      renderEncodedDrawFrame(ir, ckCanvas, CK, { surface });
      surface.flush();

      const startSecs = timeSecs;
      const durationSecs = 1.0 / this.fps;
      const audioChannels = this.mixAudioForFrame(startSecs, durationSecs);

      const bitmap = await snapshotCanvasToImageBitmap(this.canvas);

      this.onProgress(frameNum + 1, this.totalFrames);

      return { video: bitmap, audio: audioChannels, state: 'success' };
    } finally {
      surface?.delete();
    }

  }

  /// Mix audio samples from all audio sources for a time slice.
  private mixAudioForFrame(startSecs: number, durationSecs: number): Float32Array[] {
    if (this.audioIds.length === 0) return [];

    const renderer = getRendererOrThrow();
    const frameSamples = Math.ceil(durationSecs * this.sampleRate);

    const left = new Float32Array(frameSamples);
    const right = new Float32Array(frameSamples);

    for (const audioId of this.audioIds) {
      const jsonStr = renderer.get_audio_samples(
        audioId,
        startSecs,
        durationSecs,
        this.sampleRate,
      );
      try {
        const parsed = JSON.parse(jsonStr);
        if (parsed.samples && parsed.samples.length > 0) {
          for (let i = 0; i < Math.min(parsed.samples.length / 2, frameSamples); i++) {
            left[i] += parsed.samples[i * 2] || 0;
            right[i] += parsed.samples[i * 2 + 1] || 0;
          }
        }
      } catch {
        // skip malformed audio data
      }
    }

    for (let i = 0; i < frameSamples; i++) {
      left[i] = Math.max(-1, Math.min(1, left[i]));
      right[i] = Math.max(-1, Math.min(1, right[i]));
    }

    return [left, right];
  }

  async clone(): Promise<this> {
    return this;
  }

  destroy(): void {}
}

export async function snapshotCanvasToImageBitmap(
  canvas: HTMLCanvasElement | OffscreenCanvas,
): Promise<ImageBitmap | null> {
  const transferable = canvas as OffscreenCanvas;
  if (typeof transferable.transferToImageBitmap === 'function') {
    return transferable.transferToImageBitmap();
  }

  if (typeof createImageBitmap === 'function') {
    try {
      return await createImageBitmap(canvas);
    } catch {
      // Safari historically lacks this overload; fall through to Blob fallback.
    }
  }

  if (typeof createImageBitmap !== 'function') return null;

  if ('toBlob' in canvas && typeof canvas.toBlob === 'function') {
    const blob = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob(resolve, 'image/png');
    });
    return blob ? createImageBitmap(blob) : null;
  }

  if ('convertToBlob' in canvas && typeof canvas.convertToBlob === 'function') {
    const blob = await canvas.convertToBlob({ type: 'image/png' });
    return createImageBitmap(blob);
  }

  return null;
}

function createExportCanvas(
  source: HTMLCanvasElement,
  width: number,
  height: number,
): HTMLCanvasElement | OffscreenCanvas {
  if (typeof OffscreenCanvas !== 'undefined') {
    return new OffscreenCanvas(width, height);
  }

  if (source.width === width && source.height === height) {
    return source;
  }

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

// ── Public API ──

async function isAacEncodingSupported(): Promise<boolean> {
  if (typeof AudioEncoder === 'undefined') return false;
  if (!('isConfigSupported' in AudioEncoder)) return false;
  try {
    const support = await (AudioEncoder as any).isConfigSupported({
      codec: 'mp4a.40.2',
      sampleRate: 48000,
      numberOfChannels: 2,
      bitrate: 128000,
    });
    return support?.supported === true;
  } catch {
    return false;
  }
}

export async function initFFmpeg(): Promise<void> {
  // No-op: FFmpeg.wasm has been removed (replaced by WebAV/WebCodecs)
}

export async function exportMp4(
  jsonlContent: string,
  canvas: HTMLCanvasElement,
  comp: CompositionInfo,
  resourceMeta: Record<string, ResourceMeta>,
  onProgress: ProgressCallback,
  audioIds: string[],
): Promise<Uint8Array | null> {
  const { width, height, fps } = comp;
  const resourceMetaJson = JSON.stringify(resourceMeta);

  const { Combinator, OffscreenSprite } = await import('@webav/av-cliper');

  const renderCanvas = createExportCanvas(canvas, width, height);
  const clip = new ExportClip(renderCanvas, jsonlContent, resourceMetaJson, comp, onProgress, audioIds);
  const spr = new OffscreenSprite(clip);

  let hasAudio = audioIds.length > 0;
  if (hasAudio) {
    const aacSupported = await isAacEncodingSupported();
    if (!aacSupported) {
      hasAudio = false;
    }
  }

  const com = new Combinator({
    width,
    height,
    fps,
    bgColor: '#000',
    videoCodec: 'avc1.42E032',
    ...(hasAudio ? { audio: true as const } : { audio: false as const }),
  } as any);

  com.on('OutputProgress', (progress) => {
    const pct = Math.round(progress * 100);
    onProgress(Math.round(comp.frames * progress), comp.frames);
  });
  com.on('error', () => { /* keep WebAV error events handled without logging */ });

  await com.addSprite(spr, { main: true });

  const reader = com.output().getReader();
  const chunks: Uint8Array[] = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (value) chunks.push(value);
  }

  if (chunks.length === 0) return null;

  const totalLen = chunks.reduce((s, c) => s + c.length, 0);
  const result = new Uint8Array(totalLen);
  let offset = 0;
  for (const c of chunks) {
    result.set(c, offset);
    offset += c.length;
  }

  com.destroy();
  return result;
}

export async function exportPngFrame(
  jsonlContent: string,
  canvas: HTMLCanvasElement,
  comp: CompositionInfo,
  frame: number,
  resourceMeta: Record<string, ResourceMeta>,
): Promise<void> {
  const resourceMetaJson = JSON.stringify(resourceMeta);

  const renderer = getRendererOrThrow();
  const CK = (globalThis as CanvasKitGlobal).__canvasKit;
  if (!CK) throw new Error('CanvasKit is not initialized');

  await injectVideoFramesForRender({
    renderer,
    jsonlContent,
    frame,
    resourcesJson: resourceMetaJson,
    quality: 'exact',
  });

  let surface: Surface | null = null;
  try {
    surface = CK.MakeWebGLCanvasSurface(canvas);
    if (!surface) throw new Error('MakeWebGLCanvasSurface failed');
    const ckCanvas = surface.getCanvas();

    const ir = renderer.build_frame_ir(jsonlContent, frame, resourceMetaJson);
    renderEncodedDrawFrame(ir, ckCanvas, CK, { surface });
    surface.flush();

    const blob = await new Promise<Blob | null>((resolve) => {
      canvas.toBlob(resolve, 'image/png');
    });

    if (!blob) return;

    downloadBlob(blob, `frame_${String(frame).padStart(4, '0')}.png`);
  } finally {
    surface?.delete();
  }
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
  downloadBlob(blob, name.replace(/\.(jsonl|xml)$/i, '') + '.mp4');
}
