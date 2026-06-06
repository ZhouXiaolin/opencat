import './style.css';
import {
  clearVideoCache,
  compositionFrameCount,
  createSurfaceWithFallback,
  downloadMp4,
  exportMp4,
  exportPngFrame,
  getBlobBytes,
  getRendererOrThrow,
  initWasm,
  injectVideoFramesForRender,
  prepareVideoSource,
  prefetchVideoFramesForRender,
  preloadAssets,
  registerVideoGlobals,
  renderEncodedDrawFrame,
  setWasmBaseUrl,
  setWorkerBaseUrl,
  type CompositionFile,
  type CompositionInfo,
  type ResourceMeta,
  type VideoPreviewQuality,
  type WebRendererInstance,
} from 'opencat.js';
import CanvasKitInit from 'canvaskit-wasm/full';
import type { CanvasKit, Surface } from 'canvaskit-wasm';
import { audioPlaybackWindow, playbackPosition } from './playback';

type CanvasKitGlobal = typeof globalThis & { __canvasKit?: CanvasKit };
type ExportProgressStage = 'loading' | 'preparing' | 'rendering' | 'encoding' | 'muxing';

// --- State ---
let currentComposition: CompositionInfo | null = null;
let currentCompositionSource: string | null = null;
let currentFile: CompositionFile | null = null;
let currentFrame = 0;
let isPlaying = false;
let playRafId: number | null = null;
let playStartTime = 0;
let playStartFrame = 0;
let playAudioLoopIndex = 0;
let isExporting = false;

// --- Resource Metadata for WASM build_frame_ir ---
let resourceMeta: Record<string, ResourceMeta> = {};

// --- DOM refs ---
const fileListEl = document.getElementById('file-list')!;
const wasmStatusEl = document.getElementById('wasm-status')!;
const ckStatusEl = document.getElementById('ck-status')!;
const ffStatusEl = document.getElementById('ff-status')!;
const emptyStateEl = document.getElementById('empty-state')!;
const previewCanvas = document.getElementById('preview-canvas') as HTMLCanvasElement;
const fileInfoEl = document.getElementById('file-info')!;
const frameSlider = document.getElementById('frame-slider') as HTMLInputElement;
const frameLabel = document.getElementById('frame-label')!;
const frameInfoEl = document.getElementById('frame-info')!;
const exportInfoEl = document.getElementById('export-info')!;
const btnPlay = document.getElementById('btn-play')!;
const btnPrev = document.getElementById('btn-prev')!;
const btnNext = document.getElementById('btn-next')!;
const btnFirst = document.getElementById('btn-first')!;
const btnLast = document.getElementById('btn-last')!;
const btnExport = document.getElementById('btn-export')! as HTMLButtonElement;
const btnExportPng = document.getElementById('btn-export-png')! as HTMLButtonElement;
const exportProgress = document.getElementById('export-progress')!;
const exportProgressFill = document.getElementById('export-progress-fill')!;
const previewLoadingEl = document.getElementById('preview-loading')!;
const previewLoadingTextEl = document.getElementById('preview-loading-text')!;

function setPreviewLoading(message: string | null) {
  if (message) {
    previewLoadingTextEl.textContent = message;
    previewLoadingEl.classList.remove('hidden');
  } else {
    previewLoadingEl.classList.add('hidden');
  }
}

// --- Boot ---
async function boot() {
  try {
    wasmStatusEl.textContent = 'WASM loading...';
    wasmStatusEl.className = 'status-badge loading';
    setWasmBaseUrl('/wasm/');
    setWorkerBaseUrl('/wasm/');
    await initWasm();
    wasmStatusEl.textContent = 'WASM ready';
    wasmStatusEl.className = 'status-badge ready';

    ckStatusEl.textContent = 'CanvasKit loading...';
    ckStatusEl.className = 'status-badge loading';
    const CK = await CanvasKitInit({ locateFile: (f: string) => '/canvaskit/' + f });
    (globalThis as CanvasKitGlobal).__canvasKit = CK;
    ckStatusEl.textContent = 'CanvasKit ready';
    ckStatusEl.className = 'status-badge ready';

    // Register video decode globals
    registerVideoGlobals();

    // Load file list
    await loadFileList();

    ffStatusEl.textContent = 'WebCodecs ready';
    ffStatusEl.className = 'status-badge ready';
  } catch (err) {
    wasmStatusEl.textContent = `Bootstrap error: ${err}`;
    wasmStatusEl.className = 'status-badge error';
  }
}

// --- File list ---
const COMPOSITION_FILE_EXTENSIONS = ['.jsonl', '.xml'];

async function loadFileList() {
  try {
    const resp = await fetch('/json/');
    const text = await resp.text();
    const parser = new DOMParser();
    const doc = parser.parseFromString(text, 'text/html');
    const links = Array.from(doc.querySelectorAll('a'));
    const compositionFiles: CompositionFile[] = links
      .map((a) => a.getAttribute('href'))
      .filter((h): h is string => !!h && COMPOSITION_FILE_EXTENSIONS.some((ext) => h.endsWith(ext)))
      .map((h) => ({
        name: h.replace(/^\/+/, ''),
        path: `/json/${h.replace(/^\/+/, '')}`,
      }));

    if (compositionFiles.length === 0) {
      fileListEl.innerHTML = '<p class="hint">No composition files found</p>';
      return;
    }

    fileListEl.innerHTML = '';
    for (const file of compositionFiles) {
      const item = document.createElement('div');
      item.className = 'file-item';
      item.textContent = file.name;
      item.addEventListener('click', () => loadComposition(file));
      fileListEl.appendChild(item);
    }

    if (compositionFiles.length > 0) {
      loadComposition(compositionFiles[0]);
    }
  } catch {
    fileListEl.innerHTML = '<p class="hint">Cannot list files. Try known files:</p>';
    const knownFiles = ['profile-showcase.xml', 'morph.jsonl', 'opencat-promo.jsonl', 'animation_showcase.jsonl'];
    for (const name of knownFiles) {
      const item = document.createElement('div');
      item.className = 'file-item';
      item.textContent = name;
      item.addEventListener('click', () => loadComposition({ name, path: `/json/${name}` }));
      fileListEl.appendChild(item);
    }
  }
}

// --- Helpers ---

function parseCompInfo(compositionSource: string): CompositionInfo | null {
  const trimmedContent = compositionSource.trim();
  if (trimmedContent.startsWith('<')) {
    const rootTag = trimmedContent.match(/<opencat\b([^>]*)>/i);
    if (!rootTag) return null;
    const attrs = new Map<string, string>();
    for (const match of rootTag[1].matchAll(/([\w:-]+)\s*=\s*(["'])(.*?)\2/g)) {
      attrs.set(match[1], match[3]);
    }
    const width = Number(attrs.get('width'));
    const height = Number(attrs.get('height'));
    const fps = Number(attrs.get('fps'));
    const duration = Number(attrs.get('duration'));
    if (Number.isFinite(width) && Number.isFinite(height) && Number.isFinite(fps) && Number.isFinite(duration)) {
      return { width, height, fps, duration };
    }
    return null;
  }

  for (const line of compositionSource.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      const obj = JSON.parse(trimmed);
      if (obj.type === 'composition' && obj.width && obj.height && obj.fps && obj.duration) {
        return {
          width: obj.width,
          height: obj.height,
          fps: obj.fps,
          duration: obj.duration,
        };
      }
    } catch {}
  }
  return null;
}

/**
 * 过滤掉 JSONL 中带有 `path` 字段的非媒体元素（本地文件路径，Web 端无法解析）。
 * 保留 image/video/audio 等媒体类型 — 它们的 path 是可通过 HTTP 获取的 URL。
 */
function stripLocalPathElements(compositionSource: string): string {
  if (compositionSource.trim().startsWith('<')) {
    return compositionSource;
  }

  return compositionSource
    .split('\n')
    .filter(line => {
      const trimmed = line.trim();
      if (!trimmed) return false;
      try {
        const obj = JSON.parse(trimmed);
        if (obj.path) {
          const mediaTypes = ['image', 'video', 'audio'];
          return mediaTypes.includes(obj.type);
        }
        return true;
      } catch {
        return true;
      }
    })
    .join('\n');
}

// --- Resource Preloading ---

async function preloadResources(
  compositionSource: string,
  onProgress?: (loaded: number, total: number) => void,
): Promise<void> {
  resourceMeta = {};

  const catalogJson = await preloadAssets(compositionSource);
  const catalog = JSON.parse(catalogJson) as Record<string, ResourceMeta>;
  resourceMeta = catalog;

  const renderer = getRendererOrThrow();
  renderer.clear_image_blobs();

  const totalAssets = Object.keys(catalog).length;
  onProgress?.(0, totalAssets);
  let decoded = 0;

  for (const [assetId, meta] of Object.entries(catalog)) {
    if (meta.kind === 'image') {
      const raw = getBlobBytes(assetId);
      if (raw) {
        renderer.inject_image_bytes(assetId, raw);
      }
    } else if (meta.kind === 'video') {
      const raw = getBlobBytes(assetId);
      if (raw) {
        try {
          const videoBuf = new Uint8Array(raw).buffer;
          await prepareVideoSource(
            assetId,
            videoBuf,
          );
        } catch { /* ignore */ }
      }
    } else if (meta.kind === 'audio') {
      const raw = getBlobBytes(assetId);
      if (raw) {
        try {
          await renderer.decode_audio_file(assetId, raw);
        } catch { /* ignore */ }
      }
    }

    decoded++;
    onProgress?.(decoded, totalAssets);
  }
}

// --- Download Progress Canvas Overlay ---

function drawDownloadProgress(loaded: number, total: number): void {
  const CK = (globalThis as CanvasKitGlobal).__canvasKit;
  if (!CK || !currentComposition) return;

  const surface = createSurfaceWithFallback(CK, previewCanvas);
  if (!surface) return;
  const canvas = surface.getCanvas();

  const w = currentComposition.width;
  const h = currentComposition.height;

  canvas.clear(CK.BLACK);

  const text = `Downloading ${loaded} / ${total} images...`;
  const font = new CK.Font(null, 24);
  const paint = new CK.Paint();
  paint.setColor(CK.Color4f(0.7, 0.7, 0.7, 1.0));
  paint.setAntiAlias(true);

  const glyphs = font.getGlyphIDs(text);
  const widths = font.getGlyphWidths(glyphs);
  let textWidth = 0;
  for (let i = 0; i < widths.length; i++) textWidth += widths[i];
  canvas.drawText(text, (w - textWidth) / 2, h / 2, paint, font);

  font.delete();
  paint.delete();
  surface.flush();
  surface.delete();
}

// --- Load Composition ---
async function loadComposition(file: CompositionFile) {
  try {
    const resp = await fetch(file.path);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    currentCompositionSource = stripLocalPathElements(await resp.text());
    currentFile = file;

    // Worker-side decoder pool needs to be reset when switching files. SkImages
    // are per-frame so nothing to clean on this side.
    await clearVideoCache();

    const comp = parseCompInfo(currentCompositionSource);
    if (!comp) {
      fileInfoEl.textContent = `Invalid composition: ${file.name}`;
      return;
    }

    currentComposition = comp;
    currentFrame = 0;

    const totalFrames = compositionFrameCount(comp);
    fileInfoEl.textContent = `${file.name} (${comp.width}×${comp.height} @ ${comp.fps}fps, ${comp.duration.toFixed(2)}s)`;
    emptyStateEl.style.display = 'none';
    previewCanvas.style.display = 'block';

    {
      const maxW = Math.min(comp.width, 780);
      const scale = maxW / comp.width;
      previewCanvas.width = comp.width;
      previewCanvas.height = comp.height;
      previewCanvas.style.width = `${maxW}px`;
      previewCanvas.style.height = `${comp.height * scale}px`;
    }

    frameSlider.max = String((totalFrames - 1) / comp.fps);
    frameSlider.step = String(1 / comp.fps);
    frameSlider.value = '0';
    updateFrameInfo();

    document.querySelectorAll('.file-item').forEach((el) => el.classList.remove('active'));
    const items = document.querySelectorAll('.file-item');
    for (const item of items) {
      if (item.textContent === file.name) {
        item.classList.add('active');
        break;
      }
    }

    await preloadResources(currentCompositionSource, (done, total) => {
      drawDownloadProgress(done, total);
    });

    await renderFrameAsync(0);
  } catch (err) {
    fileInfoEl.textContent = `Error loading ${file.name}: ${err}`;
    setPreviewLoading(null);
  }
}

// --- Render ---
let renderPending = false;
let renderQueuedFrame = -1;
let renderQueuedQuality: VideoPreviewQuality = 'realtime';

async function renderFrameAsync(frame: number, quality: VideoPreviewQuality = 'realtime') {
  if (!currentCompositionSource || !currentComposition) return;

  if (renderPending) {
    renderQueuedFrame = frame;
    renderQueuedQuality = quality;
    return;
  }

  renderPending = true;
  const comp = currentComposition;

  try {
    await renderFrameWithPipeline(frame, comp, quality);
  } catch { /* ignore */ }

  renderPending = false;

  if (renderQueuedFrame >= 0) {
    const nextFrame = renderQueuedFrame;
    const nextQuality = renderQueuedQuality;
    renderQueuedFrame = -1;
    renderQueuedQuality = 'realtime';
    renderFrameAsync(nextFrame, nextQuality);
  }
}

async function renderFrameWithPipeline(
  frame: number,
  comp: CompositionInfo,
  quality: VideoPreviewQuality,
): Promise<void> {
  const renderer = getRendererOrThrow();
  const CK = (globalThis as CanvasKitGlobal).__canvasKit;
  if (!CK) throw new Error('CanvasKit is not initialized');
  const resourceMetaJson = JSON.stringify(resourceMeta);

  await injectVideoFramesForRender({
    renderer,
    jsonlContent: currentCompositionSource!,
    frame,
    resourcesJson: resourceMetaJson,
    quality,
  });

  let surface: Surface | null = null;
  try {
    surface = createSurfaceWithFallback(CK, previewCanvas);
    if (!surface) throw new Error('MakeWebGLCanvasSurface failed');

    const ckCanvas = surface.getCanvas();
    const ir = renderer.build_frame_ir(currentCompositionSource!, frame, resourceMetaJson);
    renderEncodedDrawFrame(ir, ckCanvas, CK, { surface });
    surface.flush();
    surface.flush();
  } finally {
    surface?.delete();
  }

  const totalFrames = compositionFrameCount(comp);
  frameLabel.textContent = `${(frame / comp.fps).toFixed(2)}s / ${((totalFrames - 1) / comp.fps).toFixed(2)}s`;
  frameSlider.value = String(frame / comp.fps);
}

function updateFrameInfo() {
  if (!currentComposition) return;
  const fps = currentComposition.fps;
  const totalFrames = compositionFrameCount(currentComposition);
  const currentTime = currentFrame / fps;
  const totalTime = (totalFrames - 1) / fps;
  frameLabel.textContent = `${currentTime.toFixed(2)}s / ${totalTime.toFixed(2)}s`;
  frameSlider.value = String(currentFrame / fps);
  frameInfoEl.textContent = `Frame ${currentFrame + 1}/${totalFrames} | Time ${currentTime.toFixed(2)}s`;
}

// --- Playback ---
function hasAudioSources(): boolean {
  for (const [, meta] of Object.entries(resourceMeta)) {
    if (meta.kind === 'audio') return true;
  }
  return false;
}

function audioResourceIds(): string[] {
  return Object.entries(resourceMeta)
    .filter(([, meta]) => meta.kind === 'audio')
    .map(([id]) => id);
}

function schedulePreviewAudio(
  renderer: WebRendererInstance,
  frame: number,
): void {
  if (!currentComposition) return;

  const audioIds = audioResourceIds();
  if (audioIds.length === 0) return;

  const { offsetSecs, durationSecs } = audioPlaybackWindow(
    frame,
    currentComposition.fps,
    compositionFrameCount(currentComposition),
  );

  for (const id of audioIds) {
    try {
      renderer.play_audio_at(id, offsetSecs, durationSecs);
    } catch { /* ignore */ }
  }
}

function prefetchPreviewVideoFrame(frame: number): void {
  if (!currentCompositionSource || !currentComposition) return;

  try {
    const renderer = getRendererOrThrow();
    const resourcesJson = JSON.stringify(resourceMeta);
    void prefetchVideoFramesForRender({
      renderer,
      jsonlContent: currentCompositionSource,
      frame,
      resourcesJson,
      quality: 'realtime',
    }).catch(() => { /* ignore */ });
  } catch { /* ignore */ }
}

function play() {
  if (!currentComposition || isPlaying) return;
  isPlaying = true;
  btnPlay.textContent = '⏸';

  const renderer = getRendererOrThrow();
  const useAudioClock = hasAudioSources();
  playAudioLoopIndex = 0;
  schedulePreviewAudio(renderer, currentFrame);
  prefetchPreviewVideoFrame(currentFrame);
  playStartFrame = currentFrame;
  playStartTime = useAudioClock ? renderer.audio_context_time() : performance.now() / 1000;

  function tick() {
    if (!isPlaying || !currentComposition) return;

    const elapsed = useAudioClock
      ? renderer.audio_context_time() - playStartTime
      : performance.now() / 1000 - playStartTime;

    const compFps = currentComposition.fps;
    const compFrames = compositionFrameCount(currentComposition);
    const position = playbackPosition(playStartFrame, elapsed, compFps, compFrames);
    const frame = position.frame;

    if (useAudioClock && position.loopIndex !== playAudioLoopIndex) {
      playAudioLoopIndex = position.loopIndex;
      try {
        renderer.stop_audio();
      } catch { /* ignore */ }
      schedulePreviewAudio(renderer, frame);
    }

    if (frame !== currentFrame) {
      currentFrame = frame;
      prefetchPreviewVideoFrame(frame);
      renderFrameAsync(frame);
      updateFrameInfo();
    }

    playRafId = requestAnimationFrame(tick);
  }

  playRafId = requestAnimationFrame(tick);
}

function pause() {
  isPlaying = false;
  btnPlay.textContent = '▶';
  if (playRafId !== null) {
    cancelAnimationFrame(playRafId);
    playRafId = null;
  }
  try {
    getRendererOrThrow().stop_audio();
  } catch { /* ignore */ }
}

function togglePlay() {
  if (isPlaying) pause();
  else play();
}

btnPlay.addEventListener('click', togglePlay);
btnFirst.addEventListener('click', () => {
  if (!currentComposition) return;
  pause();
  currentFrame = 0;
  renderFrameAsync(0);
  updateFrameInfo();
});
btnPrev.addEventListener('click', () => {
  if (!currentComposition) return;
  pause();
  if (currentFrame > 0) currentFrame--;
  renderFrameAsync(currentFrame);
  updateFrameInfo();
});
btnNext.addEventListener('click', () => {
  if (!currentComposition) return;
  pause();
  const totalFrames = compositionFrameCount(currentComposition);
  if (currentFrame < totalFrames - 1) currentFrame++;
  renderFrameAsync(currentFrame);
  updateFrameInfo();
});
btnLast.addEventListener('click', () => {
  if (!currentComposition) return;
  pause();
  currentFrame = compositionFrameCount(currentComposition) - 1;
  renderFrameAsync(currentFrame);
  updateFrameInfo();
});
frameSlider.addEventListener('input', () => {
  if (!currentComposition) return;
  pause();
  const time = parseFloat(frameSlider.value);
  currentFrame = Math.min(
    compositionFrameCount(currentComposition) - 1,
    Math.round(time * currentComposition.fps),
  );
  renderFrameAsync(currentFrame, 'scrubbing');
  updateFrameInfo();
});

document.addEventListener('keydown', (e: KeyboardEvent) => {
  if (e.key === ' ') {
    e.preventDefault();
    togglePlay();
  }
  if (e.key === 'ArrowRight') btnNext.click();
  if (e.key === 'ArrowLeft') btnPrev.click();
});

// --- Export ---
function exportStageLabel(stage?: ExportProgressStage): string {
  switch (stage) {
    case 'loading':
      return 'Loading encoder...';
    case 'preparing':
      return 'Preparing export...';
    case 'rendering':
      return 'Rendering frames...';
    case 'encoding':
      return 'Encoding MP4...';
    case 'muxing':
      return 'Muxing MP4...';
    default:
      return 'Exporting MP4...';
  }
}

async function handleExport() {
  if (!currentCompositionSource || !currentComposition || !currentFile) return;
  if (isExporting) return;

  isExporting = true;
  btnExport.disabled = true;
  btnExport.textContent = '⏳ Exporting...';
  btnExport.classList.add('exporting');
  exportProgress.classList.remove('hidden');
  exportProgressFill.style.width = '0%';

  try {
    const comp = currentComposition;
    exportInfoEl.textContent = 'Encoding MP4...';

    const audioIds = Object.entries(resourceMeta)
      .filter(([, meta]) => meta.kind === 'audio')
      .map(([id]) => id);

    const data = await exportMp4(currentCompositionSource, previewCanvas, comp, resourceMeta, (
      current: number,
      total: number,
      stage?: ExportProgressStage,
    ) => {
      const pct = Math.round((current / total) * 100);
      exportProgressFill.style.width = `${pct}%`;
      btnExport.textContent = `⏳ ${current}/${total}`;
      exportInfoEl.textContent = exportStageLabel(stage);
    }, audioIds);

    if (data) {
      downloadMp4(data, currentFile.name);
      exportInfoEl.textContent = 'Export complete!';
    } else {
      exportInfoEl.textContent = 'Export failed';
    }
  } catch (err) {
    exportInfoEl.textContent = `Export error: ${err}`;
  } finally {
    isExporting = false;
    btnExport.disabled = false;
    btnExport.textContent = '⬇ Export';
    btnExport.classList.remove('exporting');
    setTimeout(() => {
      exportProgress.classList.add('hidden');
      exportInfoEl.textContent = '';
    }, 3000);
  }
}

async function handleExportPng() {
  if (!currentCompositionSource || !currentComposition || !currentFile) return;
  if (isExporting) return;

  isExporting = true;
  btnExportPng.disabled = true;

  try {
    await exportPngFrame(currentCompositionSource, previewCanvas, currentComposition, currentFrame, resourceMeta);
  } catch (err) {
    console.error('PNG export failed:', err);
    exportInfoEl.textContent = `PNG export error: ${err}`;
  } finally {
    isExporting = false;
    btnExportPng.disabled = false;
  }
}

btnExport.addEventListener('click', handleExport);
btnExportPng.addEventListener('click', handleExportPng);

// --- Boot ---
boot();
