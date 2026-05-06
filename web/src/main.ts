import './style.css';
import { initWasm, parseJsonl, getCompositionInfo, collectResources, buildFrame } from './wasm';
import {
  initCanvasKit,
  ensureSurface,
  disposeSurface,
  captureFramePixels,
  getCanvasKit,
  getCkCanvas,
  getSurface,
  drawDisplayTree,
} from './renderer';
import {
  initFFmpeg as initFFmpegExport,
  exportMp4,
  exportPngFrame,
  downloadMp4,
} from './exporter';
import { loadImages, setCanvasKit, getCachedImage } from './resource';
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

    await loadFileList();
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
 * 过滤掉 JSONL 中带有 `path` 字段的元素（本地文件路径，Web 端无法解析）
 */
function stripLocalPathElements(jsonlContent: string): string {
  return jsonlContent
    .split('\n')
    .filter(line => {
      const trimmed = line.trim();
      if (!trimmed) return false;
      try {
        const obj = JSON.parse(trimmed);
        return !obj.path;
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

async function preloadResources(jsonlContent: string): Promise<void> {
  resourceMeta = {};

  // Parse JSONL to extract resource references
  const lines = jsonlContent.split('\n').filter(line => line.trim());
  const imageUrls = new Set<string>();
  const videoUrls = new Set<string>();

  for (const line of lines) {
    try {
      const obj = JSON.parse(line);
      // Check for image sources
      if (obj.src && typeof obj.src === 'string') {
        const ext = obj.src.split('.').pop()?.toLowerCase();
        if (ext === 'png' || ext === 'jpg' || ext === 'jpeg' || ext === 'gif' || ext === 'webp') {
          imageUrls.add(obj.src);
        } else if (ext === 'mp4' || ext === 'webm' || ext === 'mov') {
          videoUrls.add(obj.src);
        }
      }
      // Check for script elements (they might reference resources via ctx)
      if (obj.type === 'script') {
        // Scripts can reference resources dynamically
        // We'll collect any string that looks like a URL
        const source = obj.scriptSource || obj.source || obj.script || '';
        const urlMatches = source.match(/['"`]([^'"`]*\.(png|jpg|jpeg|gif|webp|mp4|webm|mov))['"`]/gi);
        if (urlMatches) {
          for (const match of urlMatches) {
            const url = match.replace(/['"`]/g, '');
            const ext = url.split('.').pop()?.toLowerCase();
            if (ext === 'png' || ext === 'jpg' || ext === 'jpeg' || ext === 'gif' || ext === 'webp') {
              imageUrls.add(url);
            } else if (ext === 'mp4' || ext === 'webm' || ext === 'mov') {
              videoUrls.add(url);
            }
          }
        }
      }
    } catch {
      // Skip invalid JSON lines
    }
  }

  // Load images and record metadata
  const imagePromises = Array.from(imageUrls).map(async (url) => {
    try {
      const assetId = url.startsWith('http') ? `url:${url}` : url;
      // Load the image - loadImages will handle URL construction
      await loadImages({ images: [url], videos: [], audios: [], icons: [] });
      // Get the cached image to extract dimensions
      const cached = getCachedImage(assetId);
      if (cached) {
        resourceMeta[url] = {
          width: cached.width,
          height: cached.height,
          kind: 'image',
        };
      }
    } catch (err) {
      console.warn(`Failed to preload image: ${url}`, err);
    }
  });

  // For videos, use placeholder dimensions (actual decoding happens on-demand)
  for (const url of videoUrls) {
    resourceMeta[url] = {
      width: 1920,  // Default placeholder
      height: 1080,
      kind: 'video',
      durationSecs: 10,  // Default placeholder
    };
  }

  await Promise.all(imagePromises);
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

    if (!previewCanvas.width) {
      const maxW = Math.min(comp.width, 780);
      const scale = maxW / comp.width;
      previewCanvas.width = comp.width;
      previewCanvas.height = comp.height;
      previewCanvas.style.width = `${maxW}px`;
      previewCanvas.style.height = `${comp.height * scale}px`;
    }

    frameSlider.max = String(comp.frames - 1);
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

    // Preload resources to collect metadata
    await preloadResources(currentJsonlContent);

    await renderFrameAsync(0);
  } catch (err) {
    fileInfoEl.textContent = `Error loading ${file.name}: ${err}`;
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
  const CK = getCanvasKit();
  const canvas = getCkCanvas();
  const surface = getSurface();

  if (!CK || !canvas || !surface) {
    throw new Error('CanvasKit not initialized');
  }

  // Step 1: Execute scripts to collect mutations
  const engine = getScriptEngine();
  engine.setFrameCtx(frame + 1, comp.frames, comp.frames);
  const parsed = parseJsonl(currentJsonlContent!);
  const scriptElements = (parsed.elements || []).filter(
    (e: ParsedElement) => e.type === 'script'
  );

  for (const script of scriptElements) {
    const source = (script.src || script.content || '') as string;
    if (source) {
      try {
        engine.runScript(source);
      } catch (err) {
        console.error(`Script execution error for element ${script.id}:`, err);
      }
    }
  }

  const mutationsJson = engine.collectJson();
  const resourceMetaJson = JSON.stringify(resourceMeta);

  // Strip script elements from JSONL before WASM buildFrame,
  // since JS already handled them and WASM may not recognize all script field names
  const filteredJsonl = stripScriptElements(currentJsonlContent!);

  // Step 2: WASM build display tree
  const result = buildFrame(filteredJsonl, frame, resourceMetaJson, mutationsJson);

  // Step 3: Render via CanvasKit
  canvas.clear(CK.Color4f(0.06, 0.06, 0.09, 1.0));
  drawDisplayTree(result.root, comp, frame);
  surface.flush();

  // Update frame info
  frameLabel.textContent = `${frame + 1} / ${comp.frames}`;
  frameSlider.value = String(frame);
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
  canvas.clear(CK.Color4f(0.06, 0.06, 0.09, 1.0));

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
  const f = currentFrame + 1;
  const total = currentComposition.frames;
  frameLabel.textContent = `${f} / ${total}`;
  frameSlider.value = String(currentFrame);
  frameInfoEl.textContent = `Frame ${f}/${total}`;
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
  currentFrame = parseInt(frameSlider.value, 10);
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
  ffStatusEl.textContent = 'FFmpeg loading...';
  ffStatusEl.className = 'status-badge loading';

  try {
    await initFFmpegExport();
    ffStatusEl.textContent = 'FFmpeg ready';
    ffStatusEl.className = 'status-badge ready';

    const comp = currentComposition;
    exportInfoEl.textContent = 'Encoding MP4...';

    const data = await exportMp4(currentJsonlContent, previewCanvas, comp, (current, total) => {
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
    ffStatusEl.textContent = `FFmpeg error: ${err}`;
    ffStatusEl.className = 'status-badge error';
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
  ffStatusEl.textContent = 'FFmpeg loading...';
  ffStatusEl.className = 'status-badge loading';

  try {
    await initFFmpegExport();
    ffStatusEl.textContent = 'FFmpeg ready';
    ffStatusEl.className = 'status-badge ready';

    await exportPngFrame(currentJsonlContent, previewCanvas, currentComposition, currentFrame);
  } catch (err) {
    ffStatusEl.textContent = `FFmpeg error: ${err}`;
    ffStatusEl.className = 'status-badge error';
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

// --- Init FFmpeg on idle after boot ---
setTimeout(() => {
  initFFmpegExport().then(() => {
    ffStatusEl.textContent = 'FFmpeg ready';
    ffStatusEl.className = 'status-badge ready';
  }).catch(() => {
    ffStatusEl.textContent = 'FFmpeg click-to-load';
    ffStatusEl.className = 'status-badge';
  });
}, 2000);

// --- Boot ---
boot();
