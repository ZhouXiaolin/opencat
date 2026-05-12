import type { CompositionInfo, ParsedElement } from './types';
import { ensureSurface, drawDisplayTree } from './renderer';
import { buildFrame, parseJsonl } from './wasm';
import { getScriptEngine } from './script-engine';
import type { IClip } from '@webav/av-cliper';

type ProgressCallback = (current: number, total: number) => void;

// ── Custom IClip that renders frames on-demand via CanvasKit ──

class ExportClip implements IClip {
  readonly meta: { width: number; height: number; duration: number };
  readonly ready: Promise<{ width: number; height: number; duration: number }>;

  private canvas: HTMLCanvasElement;
  private jsonlContent: string;
  private filteredJsonl: string;
  private resourceMetaJson: string;
  private comp: CompositionInfo;
  private totalFrames: number;
  private fps: number;
  private onProgress: ProgressCallback;

  constructor(
    canvas: HTMLCanvasElement,
    jsonlContent: string,
    filteredJsonl: string,
    resourceMetaJson: string,
    comp: CompositionInfo,
    onProgress: ProgressCallback,
  ) {
    this.canvas = canvas;
    this.jsonlContent = jsonlContent;
    this.filteredJsonl = filteredJsonl;
    this.resourceMetaJson = resourceMetaJson;
    this.comp = comp;
    this.totalFrames = comp.frames;
    this.fps = comp.fps;
    this.onProgress = onProgress;

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
    const { width, height } = this.comp;

    if (time >= this.meta.duration) {
      return { video: null, audio: [], state: 'done' };
    }

    const frameNum = Math.min(
      Math.floor((time / 1_000_000) * this.fps),
      this.totalFrames - 1,
    );

    // Execute scripts to collect mutations
    const engine = getScriptEngine();
    engine.setFrameCtx(frameNum + 1, this.totalFrames, this.totalFrames);
    const parsed = parseJsonl(this.jsonlContent);

    for (const el of parsed.elements || []) {
      if (el.id && el.text) {
        (window as any).__text_source_set?.(el.id, el.text);
      }
    }

    const scriptElements = (parsed.elements || []).filter(
      (e: ParsedElement) => e.type === 'script',
    );

    for (const script of scriptElements) {
      if (script.id) {
        (window as any).ctx.__currentCanvasTarget = script.id;
      }
      const source = (script.src || script.content || '') as string;
      if (source) {
        try {
          engine.runScript(source);
        } catch (err) {
          console.error(`Script execution error for element ${script.id}:`, err);
        }
      }
    }

    try {
      (window as any).ctx.__flushTimelines?.();
    } catch (err) {
      console.error('Timeline flush error:', err);
    }

    const mutationsJson = engine.collectJson();

    // Build display tree and render to canvas
    const result = buildFrame(this.filteredJsonl, frameNum, this.resourceMetaJson, mutationsJson);
    ensureSurface(this.canvas, width, height);
    drawDisplayTree(result.root, this.comp, frameNum);

    // Read canvas as PNG blob then create ImageBitmap
    const blob = await new Promise<Blob | null>((resolve) => {
      this.canvas.toBlob(resolve, 'image/png');
    });
    if (!blob) {
      return { video: null, audio: [], state: 'success' };
    }

    const bitmap = await createImageBitmap(blob);

    this.onProgress(frameNum + 1, this.totalFrames);

    return { video: bitmap, audio: [], state: 'success' };
  }

  async clone(): Promise<this> {
    return this;
  }

  destroy(): void {}
}

// ── Public API ──

export async function initFFmpeg(): Promise<void> {
  // No-op: FFmpeg.wasm has been removed (replaced by WebAV/WebCodecs)
}

export async function exportMp4(
  jsonlContent: string,
  canvas: HTMLCanvasElement,
  comp: CompositionInfo,
  resourceMeta: Record<string, { width: number; height: number; kind: string; durationSecs?: number }>,
  onProgress: ProgressCallback,
): Promise<Uint8Array | null> {
  const { width, height, fps } = comp;
  const resourceMetaJson = JSON.stringify(resourceMeta);

  // Pre-filter script elements once
  const filteredJsonl = jsonlContent
    .split('\n')
    .filter(line => {
      const trimmed = line.trim();
      if (!trimmed) return false;
      try {
        const obj = JSON.parse(trimmed);
        return obj.type !== 'script';
      } catch { return true; }
    })
    .join('\n');

  // Dynamically import Combinator + OffscreenSprite only when needed
  const { Combinator, OffscreenSprite } = await import('@webav/av-cliper');

  const clip = new ExportClip(canvas, jsonlContent, filteredJsonl, resourceMetaJson, comp, onProgress);
  const spr = new OffscreenSprite(clip);

  const com = new Combinator({
    width,
    height,
    fps,
    bgColor: '#000',
    videoCodec: 'avc1.42E032',
    audio: false,
  });

  // Track encoding progress
  com.on('OutputProgress', (progress) => {
    const pct = Math.round(progress * 100);
    onProgress(Math.round(comp.frames * progress), comp.frames);
  });
  com.on('error', (err) => {
    console.error('[export] Combinator error:', err);
  });

  await com.addSprite(spr, { main: true });

  // Collect output stream chunks
  const reader = com.output().getReader();
  const chunks: Uint8Array[] = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (value) chunks.push(value);
  }

  if (chunks.length === 0) return null;

  // Merge into single buffer
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
  resourceMeta: Record<string, { width: number; height: number; kind: string; durationSecs?: number }>,
): Promise<void> {
  const { width, height } = comp;
  const resourceMetaJson = JSON.stringify(resourceMeta);

  const filteredJsonl = jsonlContent
    .split('\n')
    .filter(line => {
      const trimmed = line.trim();
      if (!trimmed) return false;
      try {
        const obj = JSON.parse(trimmed);
        return obj.type !== 'script';
      } catch { return true; }
    })
    .join('\n');

  ensureSurface(canvas, width, height);

  // Execute scripts to collect mutations
  const engine = getScriptEngine();
  engine.setFrameCtx(frame + 1, comp.frames, comp.frames);
  const parsed = parseJsonl(jsonlContent);

  for (const el of parsed.elements || []) {
    if (el.id && el.text) {
      (window as any).__text_source_set?.(el.id, el.text);
    }
  }

  const scriptElements = (parsed.elements || []).filter(
    (e: ParsedElement) => e.type === 'script',
  );

  for (const script of scriptElements) {
    if (script.id) {
      (window as any).ctx.__currentCanvasTarget = script.id;
    }
    const source = (script.src || script.content || '') as string;
    if (source) {
      try {
        engine.runScript(source);
      } catch (err) {
        console.error(`Script execution error for element ${script.id}:`, err);
      }
    }
  }

  try {
    (window as any).ctx.__flushTimelines?.();
  } catch (err) {
    console.error('Timeline flush error:', err);
  }

  const mutationsJson = engine.collectJson();

  const result = buildFrame(filteredJsonl, frame, resourceMetaJson, mutationsJson);
  drawDisplayTree(result.root, comp, frame);

  const blob = await new Promise<Blob | null>((resolve) => {
    canvas.toBlob(resolve, 'image/png');
  });
  if (!blob) return;

  downloadBlob(blob, `frame_${String(frame).padStart(4, '0')}.png`);
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
