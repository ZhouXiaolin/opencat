import './style.css';
import { initWasm, parseJsonl, getCompositionInfo } from './wasm';
import { initCanvasKit, ensureSurface, drawFrame, disposeSurface } from './renderer';
import { initFFmpeg, exportMp4, exportPngFrame, downloadMp4 } from './exporter';
import type { CompositionInfo, JsonlFile } from './types';

// --- State ---
let currentComposition: CompositionInfo | null = null;
let currentJsonlContent: string | null = null;
let currentFile: JsonlFile | null = null;
let currentFrame = 0;
let isPlaying = false;
let playInterval: ReturnType<typeof setInterval> | null = null;
let isExporting = false;

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
    const knownFiles = [
      'morph.jsonl',
      'opencat-promo.jsonl',
      'animation_showcase.jsonl',
    ];
    for (const name of knownFiles) {
      const item = document.createElement('div');
      item.className = 'file-item';
      item.textContent = name;
      item.addEventListener('click', () => loadJsonl({ name, path: `/json/${name}` }));
      fileListEl.appendChild(item);
    }
  }
}

// --- Load JSONL ---
async function loadJsonl(file: JsonlFile) {
  try {
    const resp = await fetch(file.path);
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
    currentJsonlContent = await resp.text();
    currentFile = file;

    const comp = getCompositionInfo(currentJsonlContent);
    if (!comp) {
      fileInfoEl.textContent = `Invalid JSONL: ${file.name}`;
      return;
    }

    const wasLoaded = currentComposition !== null;
    currentComposition = comp;
    currentFrame = 0;

    fileInfoEl.textContent = `${file.name} (${comp.width}×${comp.height} @ ${comp.fps}fps, ${comp.frames} frames)`;
    emptyStateEl.style.display = 'none';
    previewCanvas.style.display = 'block';

    if (!wasLoaded) {
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
  const parsed = parseJsonl(currentJsonlContent);
  if (!parsed.composition) {
    renderPending = false;
    return;
  }

  ensureSurface(previewCanvas, comp.width, comp.height);
  drawFrame(parsed, frame, comp);
  renderPending = false;

  if (renderQueuedFrame >= 0) {
    const nextFrame = renderQueuedFrame;
    renderQueuedFrame = -1;
    renderFrameAsync(nextFrame);
  }
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
    await initFFmpeg();
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
    await initFFmpeg();
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

// --- Init FFmpeg on idle after boot ---
setTimeout(() => {
  initFFmpeg().then(() => {
    ffStatusEl.textContent = 'FFmpeg ready';
    ffStatusEl.className = 'status-badge ready';
  }).catch(() => {
    ffStatusEl.textContent = 'FFmpeg click-to-load';
    ffStatusEl.className = 'status-badge';
  });
}, 2000);

// --- Boot ---
boot();
