import {
  compositionFrameCount,
  type AudioPlan,
  type AudioPlanSegment,
  type CompositionInfo,
  type ResourceMeta,
} from '../types';
import { createWasmFaacEncoder, getRendererOrThrow } from '../wasm';
import {
  injectVideoFramesForRender,
  prepareCatalogVideoSources,
} from './video-frame-injector';
import { renderEncodedDrawFrame } from '../draw-ir';
import {
  canUseFaacAudioEncoderFallback,
  installFaacAudioEncoderFallback,
} from './faac-audio-encoder';
import type { IClip } from '@webav/av-cliper';
import type { CanvasKit, ColorSpace, Surface, WebGLOptions } from 'canvaskit-wasm';

/** Map composition-time slice onto source offset inside one core AudioPlan segment. */
function segmentExportSlice(
  segment: AudioPlanSegment,
  startSecs: number,
  durationSecs: number,
): { offsetSecs: number; durationSecs: number } | null {
  const segStart = segment.startMicros / 1_000_000;
  const segEnd = segment.endMicros / 1_000_000;
  const playStart = Math.max(startSecs, segStart);
  const playEnd = Math.min(startSecs + durationSecs, segEnd);
  if (playEnd <= playStart) return null;
  return {
    offsetSecs: playStart - segStart,
    durationSecs: playEnd - playStart,
  };
}

export type ExportProgressStage =
  | 'loading'
  | 'preparing'
  | 'rendering'
  | 'encoding'
  | 'muxing';
type ProgressCallback = (current: number, total: number, stage?: ExportProgressStage) => void;
type CanvasKitGlobal = typeof globalThis & { __canvasKit?: CanvasKit };

export function createSurfaceWithFallback(
  CK: CanvasKit,
  canvas: HTMLCanvasElement | OffscreenCanvas,
  colorSpace?: ColorSpace,
  opts?: WebGLOptions,
): Surface | null {
  if (typeof CK.MakeWebGLCanvasSurface === 'function') {
    try {
      const surface = CK.MakeWebGLCanvasSurface(canvas, colorSpace, opts);
      if (surface) return surface;
    } catch (err) {
      console.warn(
        'CanvasKit: WebGL surface creation failed, falling back to software surface.',
        err,
      );
    }
  }

  if (typeof CK.MakeSWCanvasSurface === 'function') {
    try {
      return CK.MakeSWCanvasSurface(canvas) ?? null;
    } catch (err) {
      console.warn('CanvasKit: software surface creation failed.', err);
      return null;
    }
  }

  return null;
}

async function yieldToBrowser(): Promise<void> {
  await new Promise<void>((resolve) => {
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(() => setTimeout(resolve, 0));
      return;
    }
    setTimeout(resolve, 0);
  });
}

// ── Custom IClip that renders frames on-demand via CanvasKit ──

class ExportClip implements IClip {
  readonly meta: { width: number; height: number; duration: number };
  readonly ready: Promise<{ width: number; height: number; duration: number }>;

  private canvas: HTMLCanvasElement | OffscreenCanvas;
  private jsonlContent: string;
  private comp: CompositionInfo;
  private totalFrames: number;
  private fps: number;
  private sampleRate: number;
  private onProgress: ProgressCallback;
  private audioPlan: AudioPlan;
  private surface: Surface | null;

  constructor(
    canvas: HTMLCanvasElement | OffscreenCanvas,
    jsonlContent: string,
    comp: CompositionInfo,
    onProgress: ProgressCallback,
    audioPlan: AudioPlan,
  ) {
    this.canvas = canvas;
    this.jsonlContent = jsonlContent;
    this.comp = comp;
    this.totalFrames = compositionFrameCount(comp);
    this.fps = comp.fps;
    this.sampleRate = 48000;
    this.onProgress = onProgress;
    this.audioPlan = audioPlan;
    this.surface = null;

    this.meta = {
      width: comp.width,
      height: comp.height,
      duration: Math.round(comp.duration * 1_000_000),
    };
    // Open the persistent host-owned pipeline (issue #8) before any frame
    // renders. Fetches resources, builds catalog, opens the pipeline, then
    // prepares WebCodecs video sources (required for injectVideoFrames).
    this.ready = getRendererOrThrow()
      .open_design(jsonlContent)
      .then((catalogJson) => prepareCatalogVideoSources(catalogJson))
      .then(() => this.meta);
  }

  private getSurface(CK: CanvasKit): Surface {
    this.surface ??= createSurfaceWithFallback(CK, this.canvas);
    if (!this.surface) throw new Error('createExportSurface failed');
    return this.surface;
  }

  async tick(time: number): Promise<{
    video?: ImageBitmap | null;
    audio?: Float32Array[];
    state: 'done' | 'success';
  }> {
    // Ensure the pipeline opened in the constructor is ready before rendering.
    await this.ready;
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

    this.onProgress(frameNum, this.totalFrames, 'rendering');
    if (frameNum === 0 || frameNum % 5 === 0) await yieldToBrowser();

    // One-shot ABI: build_frame_ir returns both OCIR and media plan.
    const result = renderer.build_frame_ir(frameNum);

    await injectVideoFramesForRender({
      mediaPlan: result.mediaPlan,
      quality: 'exact',
    });

    const surface = this.getSurface(CK);

    const ckCanvas = surface.getCanvas();
    renderEncodedDrawFrame(result.ir, ckCanvas, CK, { surface });
    surface.flush();

    const startSecs = timeSecs;
    const durationSecs = 1.0 / this.fps;
    const audioChannels = this.mixAudioForFrame(startSecs, durationSecs);

    const bitmap = await snapshotCanvasToImageBitmap(this.canvas);

    this.onProgress(frameNum + 1, this.totalFrames, 'encoding');

    return { video: bitmap, audio: audioChannels, state: 'success' };

  }

  /// Mix audio samples for a composition-time slice using core AudioPlan.
  /// Each segment maps composition time onto a source offset inside the asset.
  private mixAudioForFrame(startSecs: number, durationSecs: number): Float32Array[] {
    if (this.audioPlan.segments.length === 0) return [];

    const renderer = getRendererOrThrow();
    const frameSamples = Math.ceil(durationSecs * this.sampleRate);

    const left = new Float32Array(frameSamples);
    const right = new Float32Array(frameSamples);

    for (const seg of this.audioPlan.segments) {
      const slice = segmentExportSlice(seg, startSecs, durationSecs);
      if (!slice) continue;

      // Where this segment first overlaps the composition slice, in sample frames.
      const segStartSecs = seg.startMicros / 1_000_000;
      const overlapStartSecs = Math.max(startSecs, segStartSecs);
      const destOffsetSamples = Math.floor(
        (overlapStartSecs - startSecs) * this.sampleRate,
      );

      const jsonStr = renderer.get_audio_samples(
        seg.assetId,
        slice.offsetSecs,
        slice.durationSecs,
        this.sampleRate,
      );
      try {
        const parsed = JSON.parse(jsonStr);
        if (parsed.samples && parsed.samples.length > 0) {
          const available = Math.floor(parsed.samples.length / 2);
          for (let i = 0; i < available; i++) {
            const dst = destOffsetSamples + i;
            if (dst < 0 || dst >= frameSamples) continue;
            left[dst] += parsed.samples[i * 2] || 0;
            right[dst] += parsed.samples[i * 2 + 1] || 0;
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

  destroy(): void {
    try { this.surface?.delete(); } catch { /* ignore CanvasKit cleanup failures */ }
    this.surface = null;
  }
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

function createIsolatedExportCanvas(width: number, height: number): HTMLCanvasElement | OffscreenCanvas {
  if (typeof OffscreenCanvas !== 'undefined') {
    return new OffscreenCanvas(width, height);
  }

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

async function canvasToPngBlob(canvas: HTMLCanvasElement | OffscreenCanvas): Promise<Blob | null> {
  if ('toBlob' in canvas && typeof canvas.toBlob === 'function') {
    return await new Promise<Blob | null>((resolve) => {
      canvas.toBlob(resolve, 'image/png');
    });
  }

  if ('convertToBlob' in canvas && typeof canvas.convertToBlob === 'function') {
    return await canvas.convertToBlob({ type: 'image/png' });
  }

  return null;
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
  audioPlan: AudioPlan = { segments: [] },
): Promise<Uint8Array | null> {
  const { width, height, fps } = comp;
  const totalFrames = compositionFrameCount(comp);

  onProgress(0, totalFrames, 'loading');
  await yieldToBrowser();
  const { Combinator, OffscreenSprite } = await import('@webav/av-cliper');

  onProgress(0, totalFrames, 'preparing');
  await yieldToBrowser();
  const renderCanvas = createExportCanvas(canvas, width, height);
  const clip = new ExportClip(renderCanvas, jsonlContent, comp, onProgress, audioPlan);
  const spr = new OffscreenSprite(clip);

  let hasAudio = audioPlan.segments.length > 0;
  let restoreAudioEncoder: (() => void) | null = null;
  if (hasAudio) {
    const aacSupported = await isAacEncodingSupported();
    if (!aacSupported) {
      if (canUseFaacAudioEncoderFallback()) {
        restoreAudioEncoder = installFaacAudioEncoderFallback({
          createEncoder: createWasmFaacEncoder,
        });
      } else {
        hasAudio = false;
      }
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
    onProgress(Math.round(totalFrames * progress), totalFrames, 'muxing');
  });
  com.on('error', () => { /* keep WebAV error events handled without logging */ });

  onProgress(0, totalFrames, 'encoding');
  await yieldToBrowser();

  await com.addSprite(spr, { main: true });

  onProgress(0, totalFrames, 'muxing');
  await yieldToBrowser();

  try {
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

    return result;
  } finally {
    restoreAudioEncoder?.();
    com.destroy();
  }
}

export async function exportPngFrame(
  jsonlContent: string,
  _canvas: HTMLCanvasElement,
  comp: CompositionInfo,
  frame: number,
  _resourceMeta: Record<string, ResourceMeta>,
): Promise<void> {
  const renderer = getRendererOrThrow();
  const CK = (globalThis as CanvasKitGlobal).__canvasKit;
  if (!CK) throw new Error('CanvasKit is not initialized');

  // Open the persistent host-owned pipeline (issue #8) for this export path.
  const catalogJson = await renderer.open_design(jsonlContent);
  await prepareCatalogVideoSources(catalogJson);

  // One-shot ABI: build_frame_ir returns both OCIR and media plan.
  const result = renderer.build_frame_ir(frame);

  await injectVideoFramesForRender({
    mediaPlan: result.mediaPlan,
    quality: 'exact',
  });

  const canvas = createIsolatedExportCanvas(comp.width, comp.height);
  let surface: Surface | null = null;
  try {
    surface = createSurfaceWithFallback(CK, canvas);
    if (!surface) throw new Error('createExportSurface failed');
    const ckCanvas = surface.getCanvas();

    renderEncodedDrawFrame(result.ir, ckCanvas, CK, { surface });
    surface.flush();

    const blob = await canvasToPngBlob(canvas);

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
