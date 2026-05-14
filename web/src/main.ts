import './style.css';
import {
  initWasm,
  parseJsonl,
  getCompositionInfo,
  collectResources,
  buildFrame,
  preloadAssets,
  clearBlobs,
} from './wasm';
import {
  initCanvasKit,
  ensureSurface,
  disposeSurface,
  getCanvasKit,
  getCkCanvas,
  getSurface,
  drawDisplayTree,
  registerImage,
} from './renderer';
import {
  exportMp4,
  exportPngFrame,
  downloadMp4,
} from './exporter';
import { decodeImageFromBlob, setCanvasKit } from './resource';
import { getScriptEngine } from './script-engine';
import type { CompositionInfo, JsonlFile, ParsedResult, ParsedElement } from './types';

// --- State ---
let currentComposition: CompositionInfo | null = null;
let currentJsonlContent: string | null = null;
let currentFile: JsonlFile | null = null;
let currentFrame = 0;
let isPlaying = false;
let playInterval: ReturnType<typeof setInterval> | null = null;
let isExporting = false;

// --- Resource Metadata for WASM build_frame ---
interface ResourceMeta {
  width: number;
  height: number;
  kind: string;
  durationSecs?: number;
}
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
    await initWasm();
    wasmStatusEl.textContent = 'WASM ready';
    wasmStatusEl.className = 'status-badge ready';

    ckStatusEl.textContent = 'CanvasKit loading...';
    ckStatusEl.className = 'status-badge loading';
    await initCanvasKit();
    ckStatusEl.textContent = 'CanvasKit ready';
    ckStatusEl.className = 'status-badge ready';

    setCanvasKit(getCanvasKit());

    // Initialize shared script engine (loads wasm bridge + core JS runtimes once)
    await getScriptEngine().init();

    // Load file list first (fast, local)
    await loadFileList();

    // Export uses WebCodecs (hardware-accelerated), no preloading needed
    ffStatusEl.textContent = 'WebCodecs ready';
    ffStatusEl.className = 'status-badge ready';
  } catch (err) {
    wasmStatusEl.textContent = `Bootstrap error: ${err}`;
    wasmStatusEl.className = 'status-badge error';
  }
}

// --- File list ---
async function loadFileList() {
  try {
    const resp = await fetch('/json/');
    const text = await resp.text();
    const parser = new DOMParser();
    const doc = parser.parseFromString(text, 'text/html');
    const links = Array.from(doc.querySelectorAll('a'));
    const jsonlFiles: JsonlFile[] = links
      .map((a) => a.getAttribute('href'))
      .filter((h): h is string => !!h && h.endsWith('.jsonl'))
      .map((h) => ({
        name: h.replace(/^\/+/, ''),
        path: `/json/${h.replace(/^\/+/, '')}`,
      }));

    if (jsonlFiles.length === 0) {
      fileListEl.innerHTML = '<p class="hint">No .jsonl files found</p>';
      return;
    }

    fileListEl.innerHTML = '';
    for (const file of jsonlFiles) {
      const item = document.createElement('div');
      item.className = 'file-item';
      item.textContent = file.name;
      item.addEventListener('click', () => loadJsonl(file));
      fileListEl.appendChild(item);
    }

    if (jsonlFiles.length > 0) {
      loadJsonl(jsonlFiles[0]);
    }
  } catch {
    fileListEl.innerHTML = '<p class="hint">Cannot list files. Try known files:</p>';
    const knownFiles = ['morph.jsonl', 'opencat-promo.jsonl', 'animation_showcase.jsonl'];
    for (const name of knownFiles) {
      const item = document.createElement('div');
      item.className = 'file-item';
      item.textContent = name;
      item.addEventListener('click', () => loadJsonl({ name, path: `/json/${name}` }));
      fileListEl.appendChild(item);
    }
  }
}

// --- Helpers ---

/**
 * Extract the root display item's solid background color for canvas clear.
 * Returns null if the root has no solid background fill.
 */
function extractRootBackground(root: any): { r: number; g: number; b: number; a: number } | null {
  const item = root?.item;
  if (!item) return null;
  if (item.type !== 'rect' && item.type !== 'timeline') return null;
  const bg = item?.paint?.background;
  if (!bg || bg.type !== 'solid' || !bg.color) return null;
  return bg.color;
}

/**
 * 过滤掉 JSONL 中带有 `path` 字段的非媒体元素（本地文件路径，Web 端无法解析）。
 * 保留 image/video/audio 等媒体类型 — 它们的 path 是可通过 HTTP 获取的 URL。
 */
function stripLocalPathElements(jsonlContent: string): string {
  return jsonlContent
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

/**
 * 从 JSONL 中剥离 type: script 的元素（JS 已处理，避免 WASM 重复处理报错）
 */
function stripScriptElements(jsonlContent: string): string {
  return jsonlContent
    .split('\n')
    .filter(line => {
      const trimmed = line.trim();
      if (!trimmed) return false;
      try {
        const obj = JSON.parse(trimmed);
        return obj.type !== 'script';
      } catch {
        return true;
      }
    })
    .join('\n');
}

// --- Resource Preloading ---

async function preloadResources(
  jsonlContent: string,
  onProgress?: (loaded: number, total: number) => void,
): Promise<void> {
  resourceMeta = {};

  // 列举一下要预下载的资源数量（仅用于进度显示），实际下载由 Rust 一次性处理。
  const requests = collectResources(jsonlContent);
  const totalAssets = requests.images.length + requests.videos.length + requests.audios.length;
  onProgress?.(0, totalAssets);

  // 清空上一轮的 BlobStore，让 Rust 重新填。
  clearBlobs();

  // 把整个下载 + 元数据探测交给 Rust：一次 await 出 catalog JSON。
  const catalogJson = await preloadAssets(jsonlContent);
  const catalog = JSON.parse(catalogJson) as Record<string, ResourceMeta>;

  // 元数据直接来自 Rust 的 nom-exif / imagesize 探测结果（dims + duration 准确）。
  resourceMeta = catalog;

  // 对图片资源：从 wasm BlobStore 取字节、CanvasKit 解码、注册到 renderer。
  let decoded = 0;
  for (const [assetId, meta] of Object.entries(catalog)) {
    if (meta.kind === 'image') {
      const loaded = decodeImageFromBlob(assetId);
      if (loaded) {
        registerImage(assetId, loaded.ckImage);
      }
    }
    decoded++;
    onProgress?.(decoded, totalAssets);
  }
}

// --- Download Progress Canvas Overlay ---

/** Draw a black canvas with download progress text centered. */
function drawDownloadProgress(loaded: number, total: number): void {
  const CK = getCanvasKit();
  const canvas = getCkCanvas();
  const surface = getSurface();
  if (!CK || !canvas || !surface || !currentComposition) return;

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
}

// --- Load JSONL ---
async function loadJsonl(file: JsonlFile) {
  try {
    const resp = await fetch(file.path);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    currentJsonlContent = stripLocalPathElements(await resp.text());
    currentFile = file;

    const comp = getCompositionInfo(currentJsonlContent);
    if (!comp) {
      fileInfoEl.textContent = `Invalid JSONL: ${file.name}`;
      return;
    }

    currentComposition = comp;
    currentFrame = 0;

    fileInfoEl.textContent = `${file.name} (${comp.width}×${comp.height} @ ${comp.fps}fps, ${comp.frames} frames)`;
    emptyStateEl.style.display = 'none';
    previewCanvas.style.display = 'block';

    {
      // Always resize canvas to match the composition dimensions
      const maxW = Math.min(comp.width, 780);
      const scale = maxW / comp.width;
      previewCanvas.width = comp.width;
      previewCanvas.height = comp.height;
      previewCanvas.style.width = `${maxW}px`;
      previewCanvas.style.height = `${comp.height * scale}px`;
    }

    frameSlider.max = String((comp.frames - 1) / comp.fps);
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

    ensureSurface(previewCanvas, comp.width, comp.height);

    // Block first render until image assets are downloaded.
    // During download the canvas shows a black background with progress text.
    try {
      await preloadResources(currentJsonlContent, (done, total) => {
        drawDownloadProgress(done, total);
      });
    } finally {
    }

    await renderFrameAsync(0);
  } catch (err) {
    fileInfoEl.textContent = `Error loading ${file.name}: ${err}`;
    setPreviewLoading(null);
  }
}

// --- Render ---
let renderPending = false;
let renderQueuedFrame = -1;

async function renderFrameAsync(frame: number) {
  if (!currentJsonlContent || !currentComposition) return;

  if (renderPending) {
    renderQueuedFrame = frame;
    return;
  }

  renderPending = true;
  const comp = currentComposition;
  ensureSurface(previewCanvas, comp.width, comp.height);

  try {
    // Use the full pipeline: script execution → build_frame → drawDisplayTree
    await renderFrameWithPipeline(frame, comp);
  } catch (err) {
    console.error('Pipeline render error, falling back:', err);
    // Fallback to simple renderer
    const parsed = parseJsonl(currentJsonlContent);
    if (!parsed.composition) {
      renderPending = false;
      return;
    }
    drawFallbackFrame(parsed, frame, comp);
  }

  renderPending = false;

  if (renderQueuedFrame >= 0) {
    const nextFrame = renderQueuedFrame;
    renderQueuedFrame = -1;
    renderFrameAsync(nextFrame);
  }
}

// --- Full Pipeline Renderer ---

async function renderFrameWithPipeline(frame: number, comp: CompositionInfo): Promise<void> {
  // Step 1: Execute scripts to collect mutations (shared by both paths)
  const engine = getScriptEngine();
  engine.setFrameCtx(frame + 1, comp.frames, comp.frames);
  const parsed = parseJsonl(currentJsonlContent!);

  // Pre-register text sources from JSONL elements so that script features
  // like splitText can resolve text content before the wasm pipeline runs.
  for (const el of parsed.elements || []) {
    if (el.id && el.text) {
      (window as any).__text_source_set?.(el.id, el.text);
    }
  }

  const scriptElements = (parsed.elements || []).filter(
    (e: ParsedElement) => e.type === 'script'
  );

  for (const script of scriptElements) {
    // Set the canvas target to the script element's own ID so that
    // ctx.getCanvas() in the script resolves to this element.
    // This mirrors what the desktop engine does in ScriptRunner::run_into()
    // where ctx.__currentCanvasTarget = current_node_id is set per-element.
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

  // Flush all pending animation timelines after script execution.
  // In the desktop engine this happens automatically after each
  // script element's run_frame(), flushing timelines that were
  // queued via ctx.timeline() / ctx.to() / ctx.from() etc.
  // Without this, animated values are never recorded as mutations.
  try {
    (window as any).ctx.__flushTimelines?.();
  } catch (err) {
    console.error('Timeline flush error:', err);
  }

  const mutationsJson = engine.collectJson();
  const resourceMetaJson = JSON.stringify(resourceMeta);

  // Strip script elements from JSONL before WASM buildFrame,
  // since JS already handled them and WASM may not recognize all script field names
  const filteredJsonl = stripScriptElements(currentJsonlContent!);

  // Step 2: WASM build display tree
  const result = buildFrame(filteredJsonl, frame, resourceMetaJson, mutationsJson);

  // Step 3: Render via CanvasKit
  const rootBg = extractRootBackground(result.root);

  const CK = getCanvasKit();
  const canvas = getCkCanvas();
  const surface = getSurface();

  if (CK && canvas && surface) {
    drawDisplayTree(result.root, comp, frame, rootBg);
    surface.flush();
  }

  // Update frame info
  frameLabel.textContent = `${(frame / comp.fps).toFixed(2)}s / ${((comp.frames - 1) / comp.fps).toFixed(2)}s`;
  frameSlider.value = String(frame / comp.fps);
}

// --- Fallback renderer (when WASM buildDisplayTree not available) ---

function drawFallbackFrame(
  parsed: { composition: CompositionInfo | null; elements: any[]; elementCount: number },
  frame: number,
  comp: CompositionInfo,
): void {
  const CK = getCanvasKit();
  const canvas = getCkCanvas();
  const surf = getSurface();
  if (!CK || !canvas || !surf) return;

  const w = comp.width;
  const h = comp.height;
  canvas.clear(CK.Color4f(0, 0, 0, 0));

  const font = new CK.Font(null, 14);
  const textPaint = new CK.Paint();
  textPaint.setColor(CK.Color4f(0.63, 0.63, 0.69, 1.0));

  const info = `${comp.width}×${comp.height} @ ${comp.fps}fps — frame ${frame + 1}/${comp.frames}`;
  canvas.drawText(info, 12, 22, textPaint, font);

  const cx = w / 2;
  const cy = h / 2;

  const strokePaint = new CK.Paint();
  strokePaint.setStyle(CK.PaintStyle.Stroke);
  strokePaint.setColor(CK.Color4f(0.23, 0.23, 0.31, 1.0));
  strokePaint.setStrokeWidth(1);
  canvas.drawLine(cx - 20, cy, cx + 20, cy, strokePaint);
  canvas.drawLine(cx, cy - 20, cx, cy + 20, strokePaint);

  strokePaint.setColor(CK.Color4f(0.29, 0.29, 0.42, 1.0));
  canvas.drawRect(CK.XYWHRect(1, 1, w - 1, h - 1), strokePaint);
  strokePaint.delete();

  const divCount = parsed.elements.filter((e: any) => e.type === 'div' || e.type === 'tl').length;
  canvas.drawText(`${parsed.elementCount} elements (${divCount} div/text)`, 12, 44, textPaint, font);

  for (const el of parsed.elements) {
    if (el.type === 'div' || el.type === 'tl') {
      const elPaint = new CK.Paint();
      const hue = (hashCode(el.id || '') % 360) / 360;
      elPaint.setColor(CK.Color4f(hue * 0.6 + 0.1, 0.4, 0.5, 0.08));
      const rect = parseRect(el.className || '', w, h);
      canvas.drawRect(CK.XYWHRect(rect.l, rect.t, rect.r - rect.l, rect.b - rect.t), elPaint);
      elPaint.delete();
    } else if (el.type === 'text' && el.text) {
      const textSize = extractFontSize(el.className || '');
      const tFont = new CK.Font(null, textSize);
      const tPaint = new CK.Paint();
      tPaint.setColor(CK.Color4f(0.88, 0.88, 0.94, 1.0));
      canvas.drawText(el.text, 24, h / 2, tPaint, tFont);
      tFont.delete();
      tPaint.delete();
    }
  }

  font.delete();
  textPaint.delete();

  surf.flush();
}

function updateFrameInfo() {
  if (!currentComposition) return;
  const fps = currentComposition.fps;
  const currentTime = currentFrame / fps;
  const totalTime = (currentComposition.frames - 1) / fps;
  frameLabel.textContent = `${currentTime.toFixed(2)}s / ${totalTime.toFixed(2)}s`;
  frameSlider.value = String(currentFrame / fps);
  frameInfoEl.textContent = `Frame ${currentFrame + 1}/${currentComposition.frames} | Time ${currentTime.toFixed(2)}s`;
}

// --- Playback ---
function play() {
  if (!currentComposition || isPlaying) return;
  isPlaying = true;
  btnPlay.textContent = '⏸';
  playInterval = setInterval(() => {
    if (!currentComposition) return;
    currentFrame++;
    if (currentFrame >= currentComposition.frames) {
      currentFrame = 0;
    }
    renderFrameAsync(currentFrame);
    updateFrameInfo();
  }, 1000 / currentComposition.fps);
}

function pause() {
  isPlaying = false;
  btnPlay.textContent = '▶';
  if (playInterval) {
    clearInterval(playInterval);
    playInterval = null;
  }
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
  if (currentFrame < currentComposition.frames - 1) currentFrame++;
  renderFrameAsync(currentFrame);
  updateFrameInfo();
});
btnLast.addEventListener('click', () => {
  if (!currentComposition) return;
  pause();
  currentFrame = currentComposition.frames - 1;
  renderFrameAsync(currentFrame);
  updateFrameInfo();
});
frameSlider.addEventListener('input', () => {
  if (!currentComposition) return;
  pause();
  const time = parseFloat(frameSlider.value);
  currentFrame = Math.round(time * currentComposition.fps);
  renderFrameAsync(currentFrame);
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
async function handleExport() {
  if (!currentJsonlContent || !currentComposition || !currentFile) return;
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

    const data = await exportMp4(currentJsonlContent, previewCanvas, comp, resourceMeta, (current, total) => {
      const pct = Math.round((current / total) * 100);
      exportProgressFill.style.width = `${pct}%`;
      btnExport.textContent = `⏳ ${current}/${total}`;
    });

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
  if (!currentJsonlContent || !currentComposition || !currentFile) return;
  if (isExporting) return;

  isExporting = true;
  btnExportPng.disabled = true;

  try {
    await exportPngFrame(currentJsonlContent, previewCanvas, currentComposition, currentFrame, resourceMeta);
  } catch (err) {
    console.error('PNG export error:', err);
  } finally {
    isExporting = false;
    btnExportPng.disabled = false;
  }
}

btnExport.addEventListener('click', handleExport);
btnExportPng.addEventListener('click', handleExportPng);

// --- Helpers (for fallback rendering) ---

function hashCode(s: string): number {
  let hash = 0;
  for (let i = 0; i < s.length; i++) {
    hash = ((hash << 5) - hash) + s.charCodeAt(i);
    hash |= 0;
  }
  return hash;
}

function parseRect(className: string, canvasW: number, canvasH: number): { l: number; t: number; r: number; b: number } {
  let l = 0, t = 0, r = canvasW, b = canvasH;
  const wMatch = className.match(/w-\[(\d+)px\]/);
  const hMatch = className.match(/h-\[(\d+)px\]/);
  const insetMatch = className.match(/inset-(\d+)/);
  const leftMatch = className.match(/left-\[(\d+)px\]/);
  const topMatch = className.match(/top-\[(\d+)px\]/);

  if (wMatch) r = l + parseInt(wMatch[1]);
  if (hMatch) b = t + parseInt(hMatch[1]);
  if (leftMatch) { l = parseInt(leftMatch[1]); r = l + (wMatch ? parseInt(wMatch[1]) : canvasW - l); }
  if (topMatch) { t = parseInt(topMatch[1]); b = t + (hMatch ? parseInt(hMatch[1]) : canvasH - t); }
  if (insetMatch) { const v = parseInt(insetMatch[1]); l = v; t = v; r = canvasW - v; b = canvasH - v; }

  return { l, t, r, b };
}

function extractFontSize(className: string): number {
  const m = className.match(/text-\[(\d+)px\]/);
  return m ? parseInt(m[1]) : 16;
}

// --- Boot ---
boot();
