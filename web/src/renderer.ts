// ── CanvasKitJS DisplayList Mapper ──
// Maps DisplayItem types (Rect, Text, Bitmap, SvgPath, DrawScript, Timeline)
// to CanvasKitJS draw calls.

import type {
  DisplayNodeJson,
  DisplayItemJson,
  DisplayTransformJson,
  DisplayRect,
  RectPaintJson,
  BackgroundFillJson,
  Color4f,
  BorderRadius,
  CanvasCommandJson,
  CompositionInfo,
  DisplayTextGlyphs,
  DisplayGlyphData,
  DisplayGlyphCommand,
  DropShadowJson,
} from './types';

import { recordPicture, drawTransition, setCanvasKitForTransition } from './transition';
import {
  getDecodedFrameRgba,
  resolveVideoTimeSecs,
  getVideoDurationSecs,
} from './video-decoder';
import type { VideoFrameTiming } from './video-decoder';

let CanvasKit: any = null;
let surface: any = null;
let ckCanvas: any = null;

// ── Text unit grouping types ──

interface GlyphSlot {
  lineIndex: number;
  posIndex: number;
  cacheKey: number;
  glyphData: DisplayGlyphData;
  gx: number;
  gy: number;
  byteStart: number;
}

interface UnitGroup {
  unitIndex: number;
  slots: GlyphSlot[];
  bbox: { minX: number; minY: number; maxX: number; maxY: number };
}

// Store loaded images for DrawScript/bitmap rendering
const loadedImages = new Map<string, any>();

// Module-level cache for glyph paths
const glyphPathCache = new Map<number, any>();

// Current frame number for per-frame video image lookup
let currentFrame = 0;
// Current composition FPS (for time-based video frame lookup)
let currentFps = 30;

export function setCurrentFrame(frame: number): void {
  currentFrame = frame;
}

/**
 * Walk display tree, find video bitmap items, decode needed frames,
 * and register them as CanvasKit images for the renderer.
 * Call this before `drawDisplayTree` for frames that need video.
 */
export async function predecodeVideoFramesInTree(
  root: DisplayNodeJson,
  comp: CompositionInfo,
  frame: number,
): Promise<void> {
  const CK = CanvasKit;
  if (!CK) return;

  const videoItems = collectVideoBitmapItems(root);
  if (videoItems.length === 0) return;

  for (const item of videoItems) {
    const assetId = item.assetId as string | undefined;
    if (!assetId) continue;

    const cacheKey = `${assetId}__${frame}`;
    if (loadedImages.has(cacheKey)) continue;

    const timing = (item as any).videoTiming as VideoFrameTiming | undefined;
    const compositionTimeSecs = frame / comp.fps;
    const durationSecs = getVideoDurationSecs(assetId);
    const targetTimeSecs = timing
      ? resolveVideoTimeSecs(compositionTimeSecs, timing, durationSecs)
      : compositionTimeSecs;

  // Decode frame and create CK Image
  try {
    const decoded = await getDecodedFrameRgba(assetId, targetTimeSecs);
    if (!decoded) continue;

    const imageInfo = {
      width: decoded.width,
      height: decoded.height,
      colorType: CK.ColorType.RGBA_8888,
      alphaType: CK.AlphaType.Unpremul,
      colorSpace: CK.ColorSpace.SRGB,
    };
    const ckImage = CK.MakeImage(imageInfo, decoded.rgba, decoded.width * 4);
    if (ckImage) {
      registerImage(cacheKey, ckImage);
    }
  } catch (err) {
      console.warn(`[renderer] failed to decode video frame for ${assetId} at ${targetTimeSecs.toFixed(3)}s:`, err);
    }
  }
}

/** Recursively collect all bitmap display items that have an assetId. */
function collectVideoBitmapItems(node: DisplayNodeJson): DisplayItemJson[] {
  const items: DisplayItemJson[] = [];
  if (node.item?.type === 'bitmap' && node.item?.assetId) {
    items.push(node.item);
  }
  if (node.children) {
    for (const child of node.children) {
      items.push(...collectVideoBitmapItems(child));
    }
  }
  return items;
}

export function registerImage(assetId: string, ckImage: any): void {
  loadedImages.set(assetId, ckImage);
}

// ── Initialization ──

export async function initCanvasKit(): Promise<void> {
  if (CanvasKit) {
    setCanvasKitForTransition(CanvasKit);
    return;
  }
  const mod = await import('canvaskit-wasm/full');
  CanvasKit = await mod.default({
    locateFile: (file: string) => `/canvaskit/${file}`,
  });
  setCanvasKitForTransition(CanvasKit);
}

export function getCanvasKit(): any {
  return CanvasKit;
}

// Backward-compatible drawFrame for exporter
export function drawFrame(
  parsed: { composition: CompositionInfo | null; elements: any[]; elementCount: number },
  frame: number,
  comp: CompositionInfo,
  clearColor?: Color4f | null,
): void {
  if (!CanvasKit || !ckCanvas || !surface) return;

  const w = comp.width;
  const h = comp.height;

  const cc = clearColor || { r: 0, g: 0, b: 0, a: 255 };
  ckCanvas.clear(CanvasKit.Color4f(cc.r / 255, cc.g / 255, cc.b / 255, cc.a / 255));

  const paint = new CanvasKit.Paint();
  const font = new CanvasKit.Font(null, 14);
  const textPaint = new CanvasKit.Paint();
  textPaint.setColor(CanvasKit.Color4f(0.63, 0.63, 0.69, 1.0));

  const info = `${comp.width}×${comp.height} @ ${comp.fps}fps — frame ${frame + 1}/${comp.frames}`;
  ckCanvas.drawText(info, 12, 22, textPaint, font);

  const divs = parsed.elements.filter((e: any) => e.type === 'div' || e.type === 'text').length;
  ckCanvas.drawText(`${parsed.elementCount} elements (${divs} div/text)`, 12, 44, textPaint, font);

  const cx = w / 2;
  const cy = h / 2;
  paint.setStyle(CanvasKit.PaintStyle.Stroke);
  paint.setColor(CanvasKit.Color4f(0.23, 0.23, 0.31, 1.0));
  paint.setStrokeWidth(1);
  ckCanvas.drawLine(cx - 20, cy, cx + 20, cy, paint);
  ckCanvas.drawLine(cx, cy - 20, cx, cy + 20, paint);

  paint.setColor(CanvasKit.Color4f(0.29, 0.29, 0.42, 1.0));
  ckCanvas.drawRect(CanvasKit.LTRBRect(1, 1, w - 1, h - 1), paint);

  paint.setStyle(CanvasKit.PaintStyle.Fill);

  for (const el of parsed.elements) {
    if (el.type === 'div' || el.type === 'tl') {
      const elPaint = new CanvasKit.Paint();
      const hue = (hashCode(el.id || '') % 360) / 360;
      elPaint.setColor(CanvasKit.Color4f(hue * 0.6 + 0.1, 0.4, 0.5, 0.08));
      const rect = parseRect(el.className || '', w, h);
      ckCanvas.drawRect(CanvasKit.LTRBRect(rect.l, rect.t, rect.r, rect.b), elPaint);
      elPaint.delete();
    } else if (el.type === 'text' && el.text) {
      const textSize = extractFontSize(el.className || '');
      const tFont = new CanvasKit.Font(null, textSize);
      const tPaint = new CanvasKit.Paint();
      tPaint.setColor(CanvasKit.Color4f(0.88, 0.88, 0.94, 1.0));
      ckCanvas.drawText(el.text, 24, h / 2, tPaint, tFont);
      tFont.delete();
      tPaint.delete();
    }
  }

  paint.delete();
  font.delete();
  textPaint.delete();

  surface.flush();
}

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

// ── Surface Management ──

export function ensureSurface(canvas: HTMLCanvasElement, width: number, height: number): void {
  if (surface) {
    if (surface.width() === width && surface.height() === height) return;
    surface.delete();
    surface = null;
    ckCanvas = null;
  }
  surface = CanvasKit.MakeWebGLCanvasSurface(canvas);
  if (!surface) {
    surface = CanvasKit.MakeSWCanvasSurface(canvas);
  }
  ckCanvas = surface.getCanvas();
}

export function getSurface() { return surface; }
export function getCkCanvas() { return ckCanvas; }

export function disposeSurface(): void {
  if (surface) {
    surface.delete();
    surface = null;
    ckCanvas = null;
  }
}

// ── Main Draw Entry Point ──

export function drawDisplayTree(
  displayTree: DisplayNodeJson,
  comp: CompositionInfo,
  frame: number,
  clearColor?: Color4f | null,
): void {
  if (!CanvasKit || !ckCanvas || !surface) return;

  setCurrentFrame(frame);
  currentFps = comp.fps;

  const w = comp.width;
  const h = comp.height;

  const cc = clearColor || { r: 0, g: 0, b: 0, a: 255 };
  ckCanvas.clear(CanvasKit.Color4f(cc.r / 255, cc.g / 255, cc.b / 255, cc.a / 255));

  // Apply root transform
  const root = displayTree;
  applyTransform(root.transform);

  // Draw display node recursively
  drawDisplayNode(root);

  // Restore root transform
  ckCanvas.restore();

  surface.flush();
}

function drawDisplayNode(node: DisplayNodeJson): void {
  const { item, opacity, clip, children } = node;

  ckCanvas.save();
  applyTransform(node.transform);

  if (opacity < 1.0) {
    // Use saveLayer for opacity
    const paint = new CanvasKit.Paint();
    paint.setAlphaf(opacity);
    ckCanvas.saveLayer(paint);
    paint.delete();
  }

  if (clip) {
    applyClip(clip);
  }

  // Draw the item
  const isTransition = item.type === 'timeline'
    && item.transition
    && children.length >= 2;

  if (isTransition) {
    drawRectItem(item);
    drawTransitionForNode(item, children);
  } else {
    drawDisplayItem(item);
    for (const child of children) {
      drawDisplayNode(child);
    }
  }

  // Restore clip
  // Restore opacity
  if (opacity < 1.0) {
    ckCanvas.restore();
  }

  ckCanvas.restore();
}

function drawTransitionForNode(
  item: DisplayItemJson,
  children: DisplayNodeJson[],
): void {
  const transition = item.transition!;
  const bounds = item.bounds;

  const drawNodeToPic = (node: DisplayNodeJson): any | null => {
    try {
      return recordPicture(bounds, (recCanvas: any) => {
        const savedCanvas = ckCanvas;
        ckCanvas = recCanvas;
        try {
          drawDisplayNode(node);
        } finally {
          ckCanvas = savedCanvas;
        }
      });
    } catch (err) {
      console.warn('[renderer] failed to record transition picture:', err);
      return null;
    }
  };

  const fromPic = drawNodeToPic(children[0]);
  const toPic = drawNodeToPic(children[1]);

  if (fromPic && toPic) {
    try {
      drawTransition(ckCanvas, fromPic, toPic, transition, bounds);
    } catch (err) {
      console.warn('[renderer] transition draw failed, falling back:', err);
      for (const child of children) {
        try { drawDisplayNode(child); } catch { /* skip */ }
      }
    }
  } else {
    console.warn(`[renderer] transition picture failed: fromPic=${!!fromPic} toPic=${!!toPic}, falling back to direct draw`);
    for (const child of children) {
      try { drawDisplayNode(child); } catch { /* skip */ }
    }
  }

  fromPic?.delete();
  toPic?.delete();
}

function applyTransform(t: DisplayTransformJson): void {
  // Always apply the base translation
  ckCanvas.translate(t.translationX, t.translationY);

  // Early return if no additional transforms (matching Rust's early return)
  if (t.transforms.length === 0) return;

  // Calculate center from bounds (matching Rust's layout_rect_to_skia)
  const centerX = t.bounds.width / 2;
  const centerY = t.bounds.height / 2;

  // REVERSE iteration to match Rust's .rev()
  for (let i = t.transforms.length - 1; i >= 0; i--) {
    const xf = t.transforms[i];
    switch (xf.type) {
      case 'translate':
        // Translate: no pivot
        ckCanvas.translate(xf.x || 0, xf.y || 0);
        break;
      case 'translateX':
        ckCanvas.translate(xf.value || 0, 0);
        break;
      case 'translateY':
        ckCanvas.translate(0, xf.value || 0);
        break;
      case 'scale':
        // T(center) * S * T(-center) — matching Rust's three-step sequence
        ckCanvas.translate(centerX, centerY);
        ckCanvas.scale(xf.value || 1, xf.value || 1);
        ckCanvas.translate(-centerX, -centerY);
        break;
      case 'scaleX':
        ckCanvas.translate(centerX, centerY);
        ckCanvas.scale(xf.value || 1, 1);
        ckCanvas.translate(-centerX, -centerY);
        break;
      case 'scaleY':
        ckCanvas.translate(centerX, centerY);
        ckCanvas.scale(1, xf.value || 1);
        ckCanvas.translate(-centerX, -centerY);
        break;
      case 'rotate':
        // CanvasKit rotate(degrees, px, py) — same as Skia, takes degrees
        ckCanvas.rotate(xf.value || 0, centerX, centerY);
        break;
      case 'skewX':
        ckCanvas.translate(centerX, centerY);
        ckCanvas.skew(Math.tan((xf.value || 0) * Math.PI / 180), 0);
        ckCanvas.translate(-centerX, -centerY);
        break;
      case 'skewY':
        ckCanvas.translate(centerX, centerY);
        ckCanvas.skew(0, Math.tan((xf.value || 0) * Math.PI / 180));
        ckCanvas.translate(-centerX, -centerY);
        break;
      case 'skew':
        ckCanvas.translate(centerX, centerY);
        ckCanvas.skew(
          Math.tan((xf.x || 0) * Math.PI / 180),
          Math.tan((xf.y || 0) * Math.PI / 180),
        );
        ckCanvas.translate(-centerX, -centerY);
        break;
    }
  }
}

function applyClip(clip: { bounds: DisplayRect; borderRadius: BorderRadius }): void {
  const { bounds, borderRadius } = clip;

  if (isUniformRadius(borderRadius) && borderRadius.topLeft > 0) {
    const rrect = CanvasKit.RRectXY(
      CanvasKit.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height),
      borderRadius.topLeft,
      borderRadius.topLeft,
    );
    ckCanvas.clipRRect(rrect, CanvasKit.ClipOp.Intersect, true);
  } else if (hasNonZeroRadius(borderRadius)) {
    const radii = [
      borderRadius.topLeft, borderRadius.topLeft,
      borderRadius.topRight, borderRadius.topRight,
      borderRadius.bottomRight, borderRadius.bottomRight,
      borderRadius.bottomLeft, borderRadius.bottomLeft,
    ];
    const rect = CanvasKit.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height);
    const rrect = Float32Array.of(rect[0], rect[1], rect[2], rect[3], radii[0], radii[1], radii[2], radii[3], radii[4], radii[5], radii[6], radii[7]);
    ckCanvas.clipRRect(rrect, CanvasKit.ClipOp.Intersect, true);
  } else {
    ckCanvas.clipRect(
      CanvasKit.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height),
      CanvasKit.ClipOp.Intersect,
      true,
    );
  }
}

// ── DisplayItem Router ──

function drawDisplayItem(item: DisplayItemJson): void {
  switch (item.type) {
    case 'rect':
      drawRectItem(item);
      break;
    case 'timeline':
      drawRectItem(item); // Timeline renders as rect + transition overlay
      break;
    case 'text':
      drawTextItem(item);
      break;
    case 'bitmap':
      drawBitmapItem(item);
      break;
    case 'drawScript':
      drawScriptItem(item);
      break;
    case 'svgPath':
      drawSvgPathItem(item);
      break;
  }
}

// ── Rect Item ──

function drawRectItem(item: DisplayItemJson): void {
  const { bounds, paint } = item;
  if (!paint) return;

  const b = bounds;

  // Box shadow
  if (paint.boxShadow) {
    drawBoxShadow(b, paint.boxShadow, paint.borderRadius);
  }

  // Background fill
  if (paint.background) {
    const fillPaint = makeFillPaint(paint.background);
    drawRoundRect(b, paint.borderRadius, fillPaint);
    fillPaint.delete();
  }

  // Border
  if (paint.borderWidth && paint.borderWidth > 0 && paint.borderColor) {
    drawBorder(b, paint);
  }

  // Inset shadow
  if (paint.insetShadow) {
    drawInsetShadow(b, paint.insetShadow, paint.borderRadius);
  }
}

// ── Text Item ──

function drawTextItem(item: DisplayItemJson): void {
  if (item.glyphs) {
    drawTextWithGlyphs(item);
    return;
  }

  // Fallback when no glyph data (uses CanvasKit default font)
  drawTextWithCanvasKitFont(item);
}

/// Group glyphs into text units (grapheme or word) and compute per-unit bounding box.
/// Returns an array of UnitGroups, each containing the unit's glyph slots and bbox.
function buildGlyphUnitGroups(
  text: string,
  glyphs: DisplayTextGlyphs,
  granularity: string | undefined,
  textAlign: string,
  boundsWidth: number,
  glyphMap: Map<number, DisplayGlyphData>,
): UnitGroup[] {
  const encoder = new TextEncoder();

  // Segment text into units and get byte ranges
  const units: Array<{ byteStart: number; byteEnd: number }> = [];

  if (granularity === 'word' || granularity === 'words') {
    // Use Rust UnicodeSegmentation::split_word_bounds via WASM,
    // matching engine's describe_text_unit_ranges
    const wordRanges: number[][] =
      ((window as any).__text_word_ranges?.(text) as number[][]) || [];
    for (const [start, end] of wordRanges) {
      units.push({ byteStart: start, byteEnd: end });
    }
  } else {
    // Grapheme-level
    const graphemes: string[] =
      ((window as any).__text_graphemes?.(text) as string[]) || [...text];
    let byteOffset = 0;
    for (const g of graphemes) {
      const byteLen = encoder.encode(g).length;
      units.push({ byteStart: byteOffset, byteEnd: byteOffset + byteLen });
      byteOffset += byteLen;
    }
  }

  // Collect all glyph slots from the layout
  const allSlots: GlyphSlot[] = [];
  for (let li = 0; li < glyphs.lines.length; li++) {
    const line = glyphs.lines[li];
    const xShift = computeTextXShift(line.width, boundsWidth, textAlign);
    for (let pi = 0; pi < line.positions.length; pi++) {
      const pos = line.positions[pi];
      const glyphData = glyphMap.get(pos.cacheKey);
      if (!glyphData) continue;
      allSlots.push({
        lineIndex: li,
        posIndex: pi,
        cacheKey: pos.cacheKey,
        glyphData,
        gx: pos.x + xShift,
        gy: pos.y,
        byteStart: pos.byteStart,
      });
    }
  }

  // Assign glyph slots to their corresponding unit
  const unitGroups: UnitGroup[] = [];
  for (let ui = 0; ui < units.length; ui++) {
    const unit = units[ui];
    const slots = allSlots.filter(
      (s) => s.byteStart >= unit.byteStart && s.byteStart < unit.byteEnd,
    );
    if (slots.length === 0) continue;

    // Compute bounding box of this unit's glyphs
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;

    for (const slot of slots) {
      if (slot.glyphData.kind === 'outline') {
        const path = buildGlyphPathCached(slot.cacheKey, slot.glyphData.commands);
        if (path) {
          const b = path.getBounds(); // Float32Array [left, top, right, bottom]
          const gx = slot.gx + b[0];
          const gy = slot.gy + b[1];
          const gw = b[2] - b[0];
          const gh = b[3] - b[1];
          if (gw > 0 && gh > 0) {
            minX = Math.min(minX, gx);
            minY = Math.min(minY, gy);
            maxX = Math.max(maxX, gx + gw);
            maxY = Math.max(maxY, gy + gh);
          } else {
            minX = Math.min(minX, slot.gx);
            minY = Math.min(minY, slot.gy);
            maxX = Math.max(maxX, slot.gx);
            maxY = Math.max(maxY, slot.gy);
          }
        } else {
          minX = Math.min(minX, slot.gx);
          minY = Math.min(minY, slot.gy);
          maxX = Math.max(maxX, slot.gx);
          maxY = Math.max(maxY, slot.gy);
        }
      } else if (slot.glyphData.kind === 'colorImage') {
        const { placementLeft, placementTop, width, height } = slot.glyphData;
        const ix = slot.gx + (placementLeft || 0);
        const iy = slot.gy - (placementTop || 0);
        const iw = width || 0;
        const ih = height || 0;
        if (iw > 0 && ih > 0) {
          minX = Math.min(minX, ix);
          minY = Math.min(minY, iy);
          maxX = Math.max(maxX, ix + iw);
          maxY = Math.max(maxY, iy + ih);
        } else {
          minX = Math.min(minX, slot.gx);
          minY = Math.min(minY, slot.gy);
          maxX = Math.max(maxX, slot.gx);
          maxY = Math.max(maxY, slot.gy);
        }
      }
    }

    // Fallback if bbox is empty (shouldn't happen with valid glyphs)
    if (slots.length > 0 && (minX === Infinity || minY === Infinity)) {
      minX = slots[0].gx - 5;
      minY = slots[0].gy - 5;
      maxX = slots[0].gx + 5;
      maxY = slots[0].gy + 5;
    }

    unitGroups.push({
      unitIndex: ui,
      slots,
      bbox: { minX, minY, maxX, maxY },
    });
  }

  return unitGroups;
}

/// Draw text using cosmic-text glyph rasterization data (paths + color images).
/// This matches the desktop engine's per-unit pivot-based transform approach.
function drawTextWithGlyphs(item: DisplayItemJson): void {
  const { bounds, style, glyphs, dropShadow } = item;
  if (!style || !glyphs) return;

  const CK = CanvasKit;
  const canvas = ckCanvas;

  const textColor = style.color;
  const fillPaint = new CK.Paint();
  fillPaint.setStyle(CK.PaintStyle.Fill);
  fillPaint.setAntiAlias(true);
  fillPaint.setColor(
    CK.Color4f(textColor.r / 255, textColor.g / 255, textColor.b / 255, textColor.a / 255),
  );

  // Build lookup map from cache_key -> glyph data
  const glyphMap = new Map<number, DisplayGlyphData>();
  for (const entry of glyphs.entries) {
    glyphMap.set(entry.cacheKey, entry.data);
  }

  // Parse text unit overrides
  const rawText = (item as any).text || '';
  const text = style.textTransform === 'uppercase' ? rawText.toUpperCase() : rawText;
  const unitOverrides = (item as any).textUnitOverrides;
  const overrides = unitOverrides?.overrides ?? null;
  const granularity = unitOverrides?.granularity;
  const textAlign = style.textAlign || 'left';

  // Group glyphs by unit
  const unitGroups = buildGlyphUnitGroups(
    text,
    glyphs,
    granularity,
    textAlign,
    bounds.width,
    glyphMap,
  );

  canvas.save();

  // Render each unit
  for (const group of unitGroups) {
    const override = overrides?.[group.unitIndex];
    const hasNonDefault =
      override &&
      ((override.translateX ?? 0) !== 0 ||
        (override.translateY ?? 0) !== 0 ||
        (override.scale ?? 1) !== 1 ||
        (override.rotationDeg ?? 0) !== 0 ||
        (override.opacity ?? 1) < 1 ||
        override.color != null);

    if (!hasNonDefault) {
      for (const slot of group.slots) {
        drawSingleGlyph(slot, fillPaint, dropShadow, CK, canvas, 1.0);
      }
      continue;
    }

    // Engine-style: T(pivot + trans) -> R -> S -> T(-pivot)
    const bx = group.bbox;
    const pivotX = (bx.minX + bx.maxX) / 2;
    const pivotY = (bx.minY + bx.maxY) / 2;
    const transX = override.translateX ?? 0;
    const transY = override.translateY ?? 0;
    const scale = override.scale ?? 1;
    const rotationDeg = override.rotationDeg ?? 0;
    const opacity = override.opacity ?? 1;

    canvas.save();
    canvas.translate(transX, transY);
    if (rotationDeg !== 0) canvas.rotate(rotationDeg, pivotX, pivotY);
    if (scale !== 1) canvas.scale(scale, scale, pivotX, pivotY);

    // Opacity via saveLayer
    if (opacity < 1) {
      const alphaPaint = new CK.Paint();
      alphaPaint.setAlphaf(opacity);
      canvas.saveLayer(alphaPaint);
      alphaPaint.delete();
    }

    // Use override color or default
    let unitPaint = fillPaint;
    let ownsUnitPaint = false;
    if (override?.color) {
      const c = override.color;
      unitPaint = new CK.Paint();
      unitPaint.setStyle(CK.PaintStyle.Fill);
      unitPaint.setAntiAlias(true);
      unitPaint.setColor(CK.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));
      ownsUnitPaint = true;
    }

    for (const slot of group.slots) {
      drawSingleGlyph(slot, unitPaint, dropShadow, CK, canvas, 1.0);
    }

    if (ownsUnitPaint) unitPaint.delete();
    if (opacity < 1) canvas.restore();
    canvas.restore();
  }

  canvas.restore();
  fillPaint.delete();
}

/// Draw a single glyph at its layout position (path or color image).
function drawSingleGlyph(
  slot: GlyphSlot,
  paint: any,
  dropShadow: DropShadowJson | null | undefined,
  CK: any,
  canvas: any,
  _opacity: number,
): void {
  const glyphData = slot.glyphData;

  if (glyphData.kind === 'outline') {
    const path = buildGlyphPathCached(slot.cacheKey, glyphData.commands);
    if (!path) return;
    canvas.save();
    canvas.translate(slot.gx, slot.gy);
    if (dropShadow) {
      drawGlyphDropShadow(path, dropShadow);
    }
    canvas.drawPath(path, paint);
    canvas.restore();
  } else if (glyphData.kind === 'colorImage') {
    const { rgba, width, height, placementLeft, placementTop } = glyphData;
    const image = buildColorImage(rgba, width, height);
    if (image) {
      canvas.save();
      canvas.translate(slot.gx, slot.gy);
      canvas.drawImage(image, placementLeft || 0, -(placementTop || 0));
      canvas.restore();
      image.delete();
    }
  }
}

/// Build a cached glyph path from outline commands.
function buildGlyphPathCached(cacheKey: number, commands: DisplayGlyphCommand[]): any {
  let path = glyphPathCache.get(cacheKey);
  if (!path) {
    path = buildGlyphPath(commands);
    if (path) {
      glyphPathCache.set(cacheKey, path);
    }
  }
  return path;
}

/// Fallback: draw text using CanvasKit's built-in font rendering.
function drawTextWithCanvasKitFont(item: DisplayItemJson): void {
  const { bounds, text, style, dropShadow } = item;
  if (!text || !style) return;

  const fontSize = style.textPx || 16;
  const font = new CanvasKit.Font(null, fontSize);

  const textPaint = new CanvasKit.Paint();
  const c = style.color;
  textPaint.setColor(CanvasKit.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));

  // Text alignment
  let x = bounds.x;
  if (style.textAlign === 'center' || style.textAlign === 'right') {
    const glyphs = font.getGlyphIDs(text);
    const widths = font.getGlyphWidths(glyphs);
    let textWidth = 0;
    for (let i = 0; i < widths.length; i++) textWidth += widths[i];
    if (style.textAlign === 'center') {
      x = bounds.x + (bounds.width - textWidth) / 2;
    } else {
      x = bounds.x + bounds.width - textWidth;
    }
  }

  // Drop shadow on text
  if (dropShadow) {
    const shadowPaint = new CanvasKit.Paint();
    const sc = dropShadow.color;
    shadowPaint.setColor(CanvasKit.Color4f(sc.r / 255, sc.g / 255, sc.b / 255, sc.a / 255));
    ckCanvas.drawText(
      text,
      x + (dropShadow.offsetX || 0),
      bounds.y + fontSize + (dropShadow.offsetY || 0),
      shadowPaint,
      font,
    );
    shadowPaint.delete();
  }

  // Transform text
  const displayText = style.textTransform === 'uppercase' ? text.toUpperCase() : text;

  ckCanvas.drawText(displayText, x, bounds.y + fontSize, textPaint, font);

  textPaint.delete();
  font.delete();
}

// ── Glyph rendering helpers ──

/// Build a CanvasKit Path from cosmic-text outline commands.
/// Y coordinates are negated because cosmic-text uses Y-up
/// (font space), while Skia/CanvasKit uses Y-down (screen space).
function buildGlyphPath(commands: DisplayGlyphCommand[]): any {
  const parts: string[] = [];
  for (const cmd of commands) {
    switch (cmd.type) {
      case 'moveTo':
        parts.push(`M${cmd.x},${-cmd.y}`);
        break;
      case 'lineTo':
        parts.push(`L${cmd.x},${-cmd.y}`);
        break;
      case 'quadTo':
        parts.push(`Q${cmd.cx},${-cmd.cy},${cmd.x},${-cmd.y}`);
        break;
      case 'curveTo':
        parts.push(`C${cmd.c1x},${-cmd.c1y},${cmd.c2x},${-cmd.c2y},${cmd.x},${-cmd.y}`);
        break;
      case 'close':
        parts.push('Z');
        break;
    }
  }
  return CanvasKit.Path.MakeFromSVGString(parts.join(' '));
}

/// Build a CanvasKit Image from raw RGBA pixel data (e.g., emoji glyphs).
function buildColorImage(rgba: number[], width: number, height: number): any {
  if (rgba.length === 0 || width === 0 || height === 0) return null;
  const imageInfo: any = {
    width,
    height,
    colorType: CanvasKit.ColorType.RGBA_8888,
    alphaType: CanvasKit.AlphaType.Unpremul,
    colorSpace: CanvasKit.ColorSpace.SRGB,
  };
  return CanvasKit.MakeImage(imageInfo, new Uint8Array(rgba), width * 4);
}

/// Draw a drop shadow for a glyph path.
function drawGlyphDropShadow(path: any, shadow: DropShadowJson): void {
  const CK = CanvasKit;
  const paint = new CK.Paint();
  const c = shadow.color;
  paint.setColor(CK.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));
  paint.setStyle(CK.PaintStyle.Fill);
  paint.setAntiAlias(true);
  if (shadow.blurSigma > 0) {
    paint.setMaskFilter(
      CK.MaskFilter.MakeBlur(CK.BlurStyle.Normal, shadow.blurSigma, false),
    );
  }
  ckCanvas.save();
  ckCanvas.translate(shadow.offsetX || 0, shadow.offsetY || 0);
  ckCanvas.drawPath(path, paint);
  ckCanvas.restore();
  paint.delete();
}

/// Compute horizontal shift for text alignment.
function computeTextXShift(
  lineWidth: number,
  containerWidth: number,
  align: string,
): number {
  switch (align) {
    case 'center':
      return (containerWidth - lineWidth) / 2;
    case 'right':
      return containerWidth - lineWidth;
    default:
      return 0;
  }
}

// ── Bitmap Item ──

function drawBitmapItem(item: DisplayItemJson): void {
  const { bounds, assetId, paint, objectFit } = item;
  if (!assetId || !bounds) return;

  // For video frames, use per-frame composite key
  const hasVideoTiming = !!(item as any).videoTiming;
  const lookupKey = hasVideoTiming
    ? `${assetId}__${currentFrame}`
    : assetId;

  const img = loadedImages.get(lookupKey);
  if (!img) {
    if (hasVideoTiming) {
      console.log(`[renderer] video bitmap NOT FOUND: ${lookupKey} (currentFrame=${currentFrame}, assetId=${assetId})`);
    }
    // Fallback: try direct assetId (for static images)
    const fallback = loadedImages.get(assetId);
    if (!fallback) return;
    drawBitmapToBounds(fallback, bounds, paint, objectFit);
    return;
  }

  drawBitmapToBounds(img, bounds, paint, objectFit);
}

function drawBitmapToBounds(
  img: any,
  bounds: DisplayRect,
  paint: RectPaintJson | undefined,
  objectFit: string | undefined,
): void {
  const srcW = img.width();
  const srcH = img.height();

  let dst = bounds;
  if (objectFit && objectFit !== 'fill') {
    const srcAspect = srcW / srcH;
    const dstAspect = bounds.width / bounds.height;

    if (objectFit === 'contain') {
      if (srcAspect > dstAspect) {
        const h = bounds.width / srcAspect;
        dst = { ...bounds, y: bounds.y + (bounds.height - h) / 2, height: h };
      } else {
        const w = bounds.height * srcAspect;
        dst = { ...bounds, x: bounds.x + (bounds.width - w) / 2, width: w };
      }
    } else if (objectFit === 'cover') {
      if (srcAspect > dstAspect) {
        const h = bounds.width / srcAspect;
        dst = { ...bounds, y: bounds.y + (bounds.height - h) / 2, height: h };
      } else {
        const w = bounds.height * srcAspect;
        dst = { ...bounds, x: bounds.x + (bounds.width - w) / 2, width: w };
      }
    }
  }

  if (paint?.borderRadius && hasNonZeroRadius(paint.borderRadius)) {
    ckCanvas.save();
    applyClip({ bounds, borderRadius: paint.borderRadius });
  }

  const srcRect = CanvasKit.XYWHRect(0, 0, srcW, srcH);
  const dstRect = CanvasKit.XYWHRect(dst.x, dst.y, dst.width, dst.height);

  const imgPaint = new CanvasKit.Paint();
  imgPaint.setAlphaf(1.0);
  imgPaint.setAntiAlias(true);
  ckCanvas.drawImageRect(img, srcRect, dstRect, imgPaint);
  imgPaint.delete();

  if (paint?.borderRadius && hasNonZeroRadius(paint.borderRadius)) {
    ckCanvas.restore();
  }
}

// ── SVG Path Item ──

function drawSvgPathItem(item: DisplayItemJson): void {
  const { bounds, pathData, viewBox } = item;
  // WASM serializes SVG paint as `item.paint` (Rust SvgPathDisplayItem.paint),
  // while the TS type also has an `svgPaint` field for backward compatibility.
  const svgPaint = (item as any).paint || item.svgPaint;
  if (!pathData || pathData.length === 0) return;

  const combinedSvg = pathData.join(' ');
  const path = CanvasKit.Path.MakeFromSVGString(combinedSvg);
  if (!path) return;
  const vb = viewBox || [0, 0, 100, 100];

  // Scale to bounds, accounting for viewBox origin offset.
  // viewBox = [minX, minY, width, height] maps to bounds.
  const scaleX = bounds.width / (vb[2] || 100);
  const scaleY = bounds.height / (vb[3] || 100);

  ckCanvas.save();
  ckCanvas.translate(bounds.x, bounds.y);
  ckCanvas.scale(scaleX, scaleY);
  ckCanvas.translate(-vb[0], -vb[1]);

  if (svgPaint?.fill) {
    const fillPaint = makeFillPaint(svgPaint.fill);
    path.setFillType(CanvasKit.FillType.Winding);
    ckCanvas.drawPath(path, fillPaint);
    fillPaint.delete();
  }

  if (svgPaint?.strokeWidth && svgPaint.strokeWidth > 0 && svgPaint.strokeColor) {
    const strokePaint = new CanvasKit.Paint();
    const sc = svgPaint.strokeColor;
    strokePaint.setColor(CanvasKit.Color4f(sc.r / 255, sc.g / 255, sc.b / 255, sc.a / 255));
    strokePaint.setStyle(CanvasKit.PaintStyle.Stroke);
    strokePaint.setStrokeWidth(svgPaint.strokeWidth);
    if (svgPaint.strokeDasharray) {
      strokePaint.setPathEffect(
        CanvasKit.PathEffect.MakeDash(
          [svgPaint.strokeDasharray],
          svgPaint.strokeDashoffset || 0,
        ),
      );
    }
    ckCanvas.drawPath(path, strokePaint);
    strokePaint.delete();
  }

  path.delete();
  ckCanvas.restore();
}

// ── DrawScript Item (CanvasCommands) ──

function drawScriptItem(item: DisplayItemJson): void {
  const { commands, bounds, dropShadow } = item;
  if (!commands || commands.length === 0) return;

  ckCanvas.save();
  ckCanvas.translate(bounds.x, bounds.y);

  // State tracking for DrawScript
  const state: {
    fillColor?: Color4f;
    strokeColor?: Color4f;
    lineWidth: number;
    globalAlpha: number;
    lineCap: string;
    lineJoin: string;
    pathStr: string;
  } = {
    lineWidth: 1,
    globalAlpha: 1,
    lineCap: 'butt',
    lineJoin: 'miter',
    pathStr: '',
  };

  for (const cmd of commands) {
    executeCanvasCommand(cmd, state);
  }

  ckCanvas.restore();
}

// ── CanvasCommand Executor ──

function executeCanvasCommand(
  cmd: CanvasCommandJson,
  state: {
    fillColor?: Color4f;
    strokeColor?: Color4f;
    lineWidth: number;
    globalAlpha: number;
    lineCap: string;
    lineJoin: string;
    pathStr: string;
  },
): void {
  const CK = CanvasKit;

  switch (cmd.type) {
    case 'save':
      ckCanvas.save();
      break;
    case 'restore':
      ckCanvas.restore();
      break;
    case 'translate':
      ckCanvas.translate(cmd.x as number || 0, cmd.y as number || 0);
      break;
    case 'scale':
      ckCanvas.scale(cmd.x as number || 1, cmd.y as number || 1);
      break;
    case 'rotate':
      ckCanvas.rotate(cmd.degrees as number || 0, 0, 0);
      break;

    case 'setFillStyle': {
      const c = cmd.color as Color4f;
      state.fillColor = c;
      break;
    }
    case 'setStrokeStyle': {
      const c = cmd.color as Color4f;
      state.strokeColor = c;
      break;
    }
    case 'setLineWidth':
      state.lineWidth = cmd.width as number || 1;
      break;
    case 'setGlobalAlpha':
      state.globalAlpha = cmd.alpha as number || 1;
      break;

    case 'fillRect': {
      const paint = new CK.Paint();
      paint.setStyle(CK.PaintStyle.Fill);
      if (state.fillColor) {
        paint.setColor(CK.Color4f(
          state.fillColor.r / 255,
          state.fillColor.g / 255,
          state.fillColor.b / 255,
          (state.fillColor.a / 255) * state.globalAlpha,
        ));
      }
      ckCanvas.drawRect(
        CK.XYWHRect(cmd.x as number || 0, cmd.y as number || 0, cmd.width as number || 0, cmd.height as number || 0),
        paint,
      );
      paint.delete();
      break;
    }

    case 'fillRRect': {
      const paint = makeStateFillPaint(state);
      const rrect = CK.RRectXY(
        CK.XYWHRect(cmd.x as number || 0, cmd.y as number || 0, cmd.width as number || 0, cmd.height as number || 0),
        cmd.radius as number || 0,
        cmd.radius as number || 0,
      );
      ckCanvas.drawRRect(rrect, paint);
      paint.delete();
      break;
    }

    case 'strokeRect': {
      const paint = new CK.Paint();
      paint.setStyle(CK.PaintStyle.Stroke);
      paint.setStrokeWidth(state.lineWidth);
      if (state.strokeColor) {
        paint.setColor(CK.Color4f(
          state.strokeColor.r / 255,
          state.strokeColor.g / 255,
          state.strokeColor.b / 255,
          (state.strokeColor.a / 255) * state.globalAlpha,
        ));
      }
      ckCanvas.drawRect(
        CK.XYWHRect(cmd.x as number || 0, cmd.y as number || 0, cmd.width as number || 0, cmd.height as number || 0),
        paint,
      );
      paint.delete();
      break;
    }

    case 'fillCircle': {
      const paint = makeStateFillPaint(state);
      ckCanvas.drawCircle(cmd.cx as number || 0, cmd.cy as number || 0, cmd.radius as number || 0, paint);
      paint.delete();
      break;
    }

    case 'strokeCircle': {
      const paint = new CK.Paint();
      paint.setStyle(CK.PaintStyle.Stroke);
      paint.setStrokeWidth(state.lineWidth);
      if (state.strokeColor) {
        paint.setColor(CK.Color4f(
          state.strokeColor.r / 255,
          state.strokeColor.g / 255,
          state.strokeColor.b / 255,
          (state.strokeColor.a / 255) * state.globalAlpha,
        ));
      }
      ckCanvas.drawCircle(cmd.cx as number || 0, cmd.cy as number || 0, cmd.radius as number || 0, paint);
      paint.delete();
      break;
    }

    case 'drawLine': {
      const paint = new CK.Paint();
      paint.setStyle(CK.PaintStyle.Stroke);
      paint.setStrokeWidth(state.lineWidth);
      if (state.strokeColor) {
        paint.setColor(CK.Color4f(
          state.strokeColor.r / 255,
          state.strokeColor.g / 255,
          state.strokeColor.b / 255,
          (state.strokeColor.a / 255) * state.globalAlpha,
        ));
      }
      ckCanvas.drawLine(cmd.x0 as number || 0, cmd.y0 as number || 0, cmd.x1 as number || 1, cmd.y1 as number || 1, paint);
      paint.delete();
      break;
    }

    case 'drawText': {
      const fontSize = cmd.fontSize as number || 16;
      const font = new CK.Font(null, fontSize);
      const paint = makeStateFillPaint(state);
      paint.setAntiAlias(cmd.antiAlias as boolean !== false);
      if (cmd.stroke as boolean) {
        paint.setStyle(CK.PaintStyle.Stroke);
        paint.setStrokeWidth(cmd.strokeWidth as number || 1);
      }
      ckCanvas.drawText(cmd.text as string || '', cmd.x as number || 0, cmd.y as number || 0, paint, font);
      font.delete();
      paint.delete();
      break;
    }

    case 'beginPath':
      state.pathStr = '';
      break;
    case 'moveTo':
      state.pathStr += `M${cmd.x as number || 0},${cmd.y as number || 0} `;
      break;
    case 'lineTo':
      state.pathStr += `L${cmd.x as number || 0},${cmd.y as number || 0} `;
      break;
    case 'quadTo':
      state.pathStr += `Q${cmd.cx as number || 0},${cmd.cy as number || 0},${cmd.x as number || 0},${cmd.y as number || 0} `;
      break;
    case 'cubicTo':
      state.pathStr += `C${cmd.c1x as number || 0},${cmd.c1y as number || 0},${cmd.c2x as number || 0},${cmd.c2y as number || 0},${cmd.x as number || 0},${cmd.y as number || 0} `;
      break;
    case 'closePath':
      state.pathStr += 'Z ';
      break;
    case 'fillPath': {
      if (state.pathStr) {
        const path = CK.Path.MakeFromSVGString(state.pathStr);
        if (path) {
          const paint = makeStateFillPaint(state);
          ckCanvas.drawPath(path, paint);
          paint.delete();
          path.delete();
        }
      }
      state.pathStr = '';
      break;
    }
    case 'strokePath': {
      if (state.pathStr) {
        const path = CK.Path.MakeFromSVGString(state.pathStr);
        if (path) {
          const paint = new CK.Paint();
          paint.setStyle(CK.PaintStyle.Stroke);
          paint.setStrokeWidth(state.lineWidth);
          if (state.strokeColor) {
            paint.setColor(CK.Color4f(
              state.strokeColor.r / 255,
              state.strokeColor.g / 255,
              state.strokeColor.b / 255,
              (state.strokeColor.a / 255) * state.globalAlpha,
            ));
          }
          ckCanvas.drawPath(path, paint);
          paint.delete();
          path.delete();
        }
      }
      state.pathStr = '';
      break;
    }

    case 'drawImage': {
      const assetId = cmd.assetId as string;
      const img = loadedImages.get(assetId);
      if (img) {
        const paint = new CK.Paint();
        paint.setAlphaf(cmd.alpha as number || 1.0);
        const dstRect = CK.XYWHRect(
          cmd.x as number || 0,
          cmd.y as number || 0,
          cmd.width as number || img.width(),
          cmd.height as number || img.height(),
        );
        if (cmd.srcRect) {
          const srcArr = cmd.srcRect as number[];
          const srcRect = CK.XYWHRect(srcArr[0], srcArr[1], srcArr[2], srcArr[3]);
          ckCanvas.drawImageRect(img, srcRect, dstRect, paint);
        } else {
          ckCanvas.drawImage(img, cmd.x as number || 0, cmd.y as number || 0, paint);
        }
        paint.delete();
      }
      break;
    }

    case 'drawImageSimple': {
      const assetId = cmd.assetId as string;
      const img = loadedImages.get(assetId);
      if (img) {
        const paint = new CK.Paint();
        paint.setAlphaf(cmd.alpha as number || 1.0);
        ckCanvas.drawImage(img, cmd.x as number || 0, cmd.y as number || 0, paint);
        paint.delete();
      }
      break;
    }

    case 'clipRect':
      ckCanvas.clipRect(
        CK.XYWHRect(cmd.x as number || 0, cmd.y as number || 0, cmd.width as number || 0, cmd.height as number || 0),
        CK.ClipOp.Intersect,
        cmd.antiAlias as boolean !== false,
      );
      break;

    case 'clear': {
      const color = cmd.color as Color4f | undefined;
      if (color) {
        ckCanvas.clear(CK.Color4f(color.r / 255, color.g / 255, color.b / 255, color.a / 255));
      }
      break;
    }
  }
}

// ── Paint Helpers ──

function makeFillPaint(fill: BackgroundFillJson): any {
  const paint = new CanvasKit.Paint();
  paint.setStyle(CanvasKit.PaintStyle.Fill);
  paint.setAntiAlias(true);

  if (fill.type === 'solid') {
    const r = fill.r ?? fill.color?.r ?? 0;
    const g = fill.g ?? fill.color?.g ?? 0;
    const b = fill.b ?? fill.color?.b ?? 0;
    const a = fill.a ?? fill.color?.a ?? 255;
    paint.setColor(CanvasKit.Color4f(r / 255, g / 255, b / 255, a / 255));
  } else if (fill.type === 'linearGradient' && fill.from && fill.to) {
    // Simple gradient
    const from = fill.from;
    const to = fill.to;
    const colors = [
      CanvasKit.Color4f(from.r / 255, from.g / 255, from.b / 255, from.a / 255),
      CanvasKit.Color4f(to.r / 255, to.g / 255, to.b / 255, to.a / 255),
    ];
    if (fill.via) {
      const via = fill.via;
      colors.splice(1, 0,
        CanvasKit.Color4f(via.r / 255, via.g / 255, via.b / 255, via.a / 255),
      );
    }
    const pos = fill.via ? [0, 0.5, 1] : [0, 1];
    const shader = CanvasKit.Shader.MakeLinearGradient(
      [0, 0], fill.direction === 'toRight' ? [1, 0] : [0, 1],
      colors, pos, CanvasKit.TileMode.Clamp,
    );
    paint.setShader(shader);
  }

  return paint;
}

function makeStateFillPaint(state: { fillColor?: Color4f; globalAlpha: number }): any {
  const paint = new CanvasKit.Paint();
  paint.setStyle(CanvasKit.PaintStyle.Fill);
  paint.setAntiAlias(true);
  if (state.fillColor) {
    paint.setColor(CanvasKit.Color4f(
      state.fillColor.r / 255,
      state.fillColor.g / 255,
      state.fillColor.b / 255,
      (state.fillColor.a / 255) * state.globalAlpha,
    ));
  }
  return paint;
}

function drawRoundRect(b: DisplayRect, br: BorderRadius, paint: any): void {
  if (hasNonZeroRadius(br)) {
    if (isUniformRadius(br)) {
      const rrect = CanvasKit.RRectXY(
        CanvasKit.XYWHRect(b.x, b.y, b.width, b.height),
        br.topLeft, br.topLeft,
      );
      ckCanvas.drawRRect(rrect, paint);
    } else {
      const radii = [
        br.topLeft, br.topLeft,
        br.topRight, br.topRight,
        br.bottomRight, br.bottomRight,
        br.bottomLeft, br.bottomLeft,
      ];
      const rect = CanvasKit.XYWHRect(b.x, b.y, b.width, b.height);
      const rrect = Float32Array.of(rect[0], rect[1], rect[2], rect[3], radii[0], radii[1], radii[2], radii[3], radii[4], radii[5], radii[6], radii[7]);
      ckCanvas.drawRRect(rrect, paint);
    }
  } else {
    ckCanvas.drawRect(CanvasKit.XYWHRect(b.x, b.y, b.width, b.height), paint);
  }
}

function drawBorder(b: DisplayRect, paint: RectPaintJson): void {
  const stroke = new CanvasKit.Paint();
  stroke.setStyle(CanvasKit.PaintStyle.Stroke);
  stroke.setAntiAlias(true);
  const borderColor = paint.borderColor!;
  stroke.setColor(CanvasKit.Color4f(
    borderColor.r / 255,
    borderColor.g / 255,
    borderColor.b / 255,
    borderColor.a / 255,
  ));

  const bw = paint.borderWidth || 1;
  stroke.setStrokeWidth(bw);

  // Handle non-uniform borders
  if (paint.borderTopWidth || paint.borderRightWidth || paint.borderBottomWidth || paint.borderLeftWidth) {
    // For individual borders, draw each side separately
    // (Simplified: just draw the full rect with the max width)
    const maxBw = Math.max(
      paint.borderTopWidth || bw,
      paint.borderRightWidth || bw,
      paint.borderBottomWidth || bw,
      paint.borderLeftWidth || bw,
    );
    stroke.setStrokeWidth(maxBw);
  }

  const inset = bw / 2;
  drawRoundRect(
    { x: b.x + inset, y: b.y + inset, width: b.width - bw, height: b.height - bw },
    paint.borderRadius,
    stroke,
  );
  stroke.delete();
}

function drawBoxShadow(b: DisplayRect, shadow: { offsetX: number; offsetY: number; blurSigma: number; spread: number; color: Color4f }, br: BorderRadius): void {
  const paint = new CanvasKit.Paint();
  const c = shadow.color;
  paint.setColor(CanvasKit.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));

  if (shadow.blurSigma > 0) {
    paint.setMaskFilter(CanvasKit.MaskFilter.MakeBlur(
      CanvasKit.BlurStyle.Normal,
      shadow.blurSigma,
      false,
    ));
  }

  const spread = shadow.spread || 0;
  const shadowRect = {
    x: b.x + (shadow.offsetX || 0) - spread,
    y: b.y + (shadow.offsetY || 0) - spread,
    width: b.width + spread * 2,
    height: b.height + spread * 2,
  };

  drawRoundRect(shadowRect, br, paint);
  paint.delete();
}

function drawInsetShadow(b: DisplayRect, shadow: { offsetX: number; offsetY: number; blurSigma: number; spread: number; color: Color4f }, br: BorderRadius): void {
  // Inset shadow: draw a larger rect with the inner area clipped out
  // Simplified: just draw a darker area on the edges
  const paint = new CanvasKit.Paint();
  const c = shadow.color;
  paint.setColor(CanvasKit.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255));

  const offsetX = shadow.offsetX || 0;
  const offsetY = shadow.offsetY || 0;
  const blur = shadow.blurSigma || 0;

  ckCanvas.save();
  // Clip to bounds
  applyClip({ bounds: b, borderRadius: br });
  // Draw a shadow rect slightly offset
  const shadowRect = {
    x: b.x + offsetX - blur,
    y: b.y + offsetY - blur,
    width: b.width + blur * 2,
    height: b.height + blur * 2,
  };
  drawRoundRect(shadowRect, br, paint);
  ckCanvas.restore();

  paint.delete();
}

// ── Frame Capture ──

export function captureFramePixels(w: number, h: number): Uint8Array | null {
  if (!surface || !CanvasKit) return null;
  const image = surface.makeImageSnapshot();
  if (!image) return null;
  const pixels = image.readPixels(0, 0, {
    width: w,
    height: h,
    colorType: CanvasKit.ColorType.RGBA_8888,
    alphaType: CanvasKit.AlphaType.Unpremul,
    colorSpace: CanvasKit.ColorSpace.SRGB,
  });
  image.delete();
  return pixels;
}

// ── Helpers ──

export function color4fToCkColor(c: Color4f): Float32Array {
  return CanvasKit.Color4f(c.r / 255, c.g / 255, c.b / 255, c.a / 255);
}

function isUniformRadius(br: BorderRadius): boolean {
  return br.topLeft === br.topRight
    && br.topLeft === br.bottomRight
    && br.topLeft === br.bottomLeft;
}

function hasNonZeroRadius(br: BorderRadius): boolean {
  return br.topLeft > 0 || br.topRight > 0 || br.bottomRight > 0 || br.bottomLeft > 0;
}
