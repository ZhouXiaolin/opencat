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
} from './types';

let CanvasKit: any = null;
let surface: any = null;
let ckCanvas: any = null;

// Store loaded images for DrawScript/bitmap rendering
const loadedImages = new Map<string, any>();

export function registerImage(assetId: string, ckImage: any): void {
  loadedImages.set(assetId, ckImage);
}

// ── Initialization ──

export async function initCanvasKit(): Promise<void> {
  if (CanvasKit) return;
  const mod = await import('canvaskit-wasm/full');
  CanvasKit = await mod.default({
    locateFile: (file: string) => `/canvaskit/${file}`,
  });
}

export function getCanvasKit(): any {
  return CanvasKit;
}

// Backward-compatible drawFrame for exporter
export function drawFrame(
  parsed: { composition: CompositionInfo | null; elements: any[]; elementCount: number },
  frame: number,
  comp: CompositionInfo,
): void {
  if (!CanvasKit || !ckCanvas || !surface) return;

  const w = comp.width;
  const h = comp.height;

  ckCanvas.clear(CanvasKit.Color4f(0.06, 0.06, 0.09, 1.0));

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
): void {
  if (!CanvasKit || !ckCanvas || !surface) return;

  const w = comp.width;
  const h = comp.height;

  ckCanvas.clear(CanvasKit.Color4f(0.06, 0.06, 0.09, 1.0));

  // Apply root transform
  const root = displayTree;
  applyTransform(root.transform);

  // Draw display node recursively
  drawDisplayNode(root);

  // Draw overlay info
  drawDebugOverlay(comp, frame);

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
  drawDisplayItem(item);

  // Draw children
  for (const child of children) {
    drawDisplayNode(child);
  }

  // Restore clip
  // Restore opacity
  if (opacity < 1.0) {
    ckCanvas.restore();
  }

  ckCanvas.restore();
}

function applyTransform(t: DisplayTransformJson): void {
  ckCanvas.translate(t.translationX, t.translationY);

  for (const xf of t.transforms) {
    switch (xf.type) {
      case 'translate':
        ckCanvas.translate(xf.x || 0, xf.y || 0);
        break;
      case 'translateX':
        ckCanvas.translate(xf.value || 0, 0);
        break;
      case 'translateY':
        ckCanvas.translate(0, xf.value || 0);
        break;
      case 'scale':
        ckCanvas.scale(xf.value || 1, xf.value || 1);
        break;
      case 'scaleX':
        ckCanvas.scale(xf.value || 1, 1);
        break;
      case 'scaleY':
        ckCanvas.scale(1, xf.value || 1);
        break;
      case 'rotateDeg':
        ckCanvas.rotate((xf.value || 0), 0, 0);
        break;
      case 'skewXDeg':
        ckCanvas.skew(Math.tan((xf.value || 0) * Math.PI / 180), 0);
        break;
      case 'skewYDeg':
        ckCanvas.skew(0, Math.tan((xf.value || 0) * Math.PI / 180));
        break;
      case 'skewDeg':
        ckCanvas.skew(
          Math.tan((xf.x || 0) * Math.PI / 180),
          Math.tan((xf.y || 0) * Math.PI / 180),
        );
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
    const rrect = CanvasKit.RRect(
      CanvasKit.XYWHRect(bounds.x, bounds.y, bounds.width, bounds.height),
      radii,
    );
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

  ckCanvas.drawText(
    displayText,
    x,
    bounds.y + fontSize,
    textPaint,
    font,
  );

  textPaint.delete();
  font.delete();
}

// ── Bitmap Item ──

function drawBitmapItem(item: DisplayItemJson): void {
  const { bounds, assetId, paint, objectFit } = item;
  if (!assetId || !bounds) return;

  const img = loadedImages.get(assetId);
  if (!img) return;

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

  // Clip to bounds with border radius
  if (paint?.borderRadius && hasNonZeroRadius(paint.borderRadius)) {
    ckCanvas.save();
    applyClip({ bounds, borderRadius: paint.borderRadius });
  }

  const srcRect = CanvasKit.XYWHRect(0, 0, srcW, srcH);
  const dstRect = CanvasKit.XYWHRect(dst.x, dst.y, dst.width, dst.height);
  const sampling = new CanvasKit.SamplingOptions(CanvasKit.FilterMode.Linear);

  const paint2 = new CanvasKit.Paint();
  paint2.setAlphaf(1.0);
  paint2.setAntiAlias(true);
  ckCanvas.drawImageRect(img, srcRect, dstRect, sampling, paint2);
  paint2.delete();

  if (paint?.borderRadius && hasNonZeroRadius(paint.borderRadius)) {
    ckCanvas.restore();
  }
}

// ── SVG Path Item ──

function drawSvgPathItem(item: DisplayItemJson): void {
  const { bounds, pathData, viewBox, svgPaint } = item;
  if (!pathData || pathData.length === 0) return;

  // SVG path syntax allows multiple subpaths in a single string (each `M` starts a new contour),
  // so concatenate all entries and parse once. CanvasKit Path has no `addPath`; only PathBuilder does.
  const combinedSvg = pathData.join(' ');
  const path = CanvasKit.Path.MakeFromSVGString(combinedSvg);
  if (!path) return;
  const vb = viewBox || [0, 0, 100, 100];

  // Scale to bounds
  const scaleX = bounds.width / (vb[2] || 100);
  const scaleY = bounds.height / (vb[3] || 100);

  ckCanvas.save();
  ckCanvas.translate(bounds.x, bounds.y);
  ckCanvas.scale(scaleX, scaleY);

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
    path?: any;
  } = {
    lineWidth: 1,
    globalAlpha: 1,
    lineCap: 'butt',
    lineJoin: 'miter',
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
    path?: any;
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
      if (state.path) {
        state.path.delete();
      }
      state.path = new CK.Path();
      break;
    case 'moveTo':
      if (state.path) state.path.moveTo(cmd.x as number || 0, cmd.y as number || 0);
      break;
    case 'lineTo':
      if (state.path) state.path.lineTo(cmd.x as number || 0, cmd.y as number || 0);
      break;
    case 'quadTo':
      if (state.path) state.path.quadTo(
        cmd.cx as number || 0, cmd.cy as number || 0,
        cmd.x as number || 0, cmd.y as number || 0,
      );
      break;
    case 'cubicTo':
      if (state.path) state.path.cubicTo(
        cmd.c1x as number || 0, cmd.c1y as number || 0,
        cmd.c2x as number || 0, cmd.c2y as number || 0,
        cmd.x as number || 0, cmd.y as number || 0,
      );
      break;
    case 'closePath':
      if (state.path) state.path.close();
      break;
    case 'fillPath': {
      if (state.path) {
        const paint = makeStateFillPaint(state);
        ckCanvas.drawPath(state.path, paint);
        paint.delete();
      }
      break;
    }
    case 'strokePath': {
      if (state.path) {
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
        ckCanvas.drawPath(state.path, paint);
        paint.delete();
      }
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
          const sampling = new CK.SamplingOptions(CK.FilterMode.Linear);
          ckCanvas.drawImageRect(img, srcRect, dstRect, sampling, paint);
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

  if (fill.type === 'solid' && fill.color) {
    paint.setColor(CanvasKit.Color4f(
      fill.color.r / 255,
      fill.color.g / 255,
      fill.color.b / 255,
      fill.color.a / 255,
    ));
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
      const rrect = CanvasKit.RRect(
        CanvasKit.XYWHRect(b.x, b.y, b.width, b.height),
        radii,
      );
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

// ── Debug Overlay ──

function drawDebugOverlay(comp: CompositionInfo, frame: number): void {
  const paint = new CanvasKit.Paint();
  const font = new CanvasKit.Font(null, 13);
  const textPaint = new CanvasKit.Paint();
  textPaint.setColor(CanvasKit.Color4f(0.63, 0.63, 0.69, 1.0));

  const info = `${comp.width}×${comp.height} @ ${comp.fps}fps — frame ${frame + 1}/${comp.frames}`;
  ckCanvas.drawText(info, 12, 18, textPaint, font);

  paint.delete();
  font.delete();
  textPaint.delete();
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
