import type { ParsedResult, CompositionInfo } from './types';

let CanvasKit: any = null;
let surface: any = null;
let ckCanvas: any = null;

export async function initCanvasKit(): Promise<void> {
  if (CanvasKit) return;
  const mod = await import('canvaskit-wasm/full');
  CanvasKit = await mod.default({
    locateFile: (file: string) => `/canvaskit/${file}`,
  });
}

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
  surface.width = () => width;
  surface.height = () => height;
  ckCanvas = surface.getCanvas();
  ckCanvas.Gd = surface.Gd;
}

export function drawFrame(
  parsed: ParsedResult,
  frame: number,
  _comp: CompositionInfo,
): void {
  if (!CanvasKit || !ckCanvas || !surface) return;

  const comp = parsed.composition;
  if (!comp) return;

  const w = comp.width;
  const h = comp.height;

  ckCanvas.clear(CanvasKit.Color4f(0.06, 0.06, 0.09, 1.0));

  const paint = new CanvasKit.Paint();
  const font = new CanvasKit.Font(null, 14);
  const textPaint = new CanvasKit.Paint();
  textPaint.setColor(CanvasKit.Color4f(0.63, 0.63, 0.69, 1.0));

  // Info text
  const info = `${comp.width}×${comp.height} @ ${comp.fps}fps — frame ${frame + 1}/${comp.frames}`;
  ckCanvas.drawText(info, 12, 22, textPaint, font);

  const divs = parsed.elements.filter((e: any) => e.type === 'div' || e.type === 'text').length;
  ckCanvas.drawText(`${parsed.elementCount} elements (${divs} div/text)`, 12, 44, textPaint, font);

  // Center crosshair
  const cx = w / 2;
  const cy = h / 2;
  paint.setStyle(CanvasKit.PaintStyle.Stroke);
  paint.setColor(CanvasKit.Color4f(0.23, 0.23, 0.31, 1.0));
  paint.setStrokeWidth(1);
  ckCanvas.drawLine(cx - 20, cy, cx + 20, cy, paint);
  ckCanvas.drawLine(cx, cy - 20, cx, cy + 20, paint);

  // Bounding box
  paint.setColor(CanvasKit.Color4f(0.29, 0.29, 0.42, 1.0));
  ckCanvas.drawRect(CanvasKit.LTRBRect(1, 1, w - 1, h - 1), paint);

  // --- Draw elements ---
  paint.setStyle(CanvasKit.PaintStyle.Fill);

  for (const el of parsed.elements) {
    const t = el.type;
    if (t === 'div' || t === 'tl') {
      // Draw a colored rectangle for each div
      const elPaint = new CanvasKit.Paint();
      const hue = (hashCode(el.id || '') % 360) / 360;
      elPaint.setColor(CanvasKit.Color4f(hue * 0.6 + 0.1, 0.4, 0.5, 0.08));

      const rect = parseRect(el.className || '', w, h);
      ckCanvas.drawRect(CanvasKit.LTRBRect(rect.l, rect.t, rect.r, rect.b), elPaint);
      elPaint.delete();
    } else if (t === 'text' && el.text) {
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

export function getCanvasKit() {
  return CanvasKit;
}

export function disposeSurface() {
  if (surface) {
    surface.delete();
    surface = null;
    ckCanvas = null;
  }
}

// --- Helpers ---

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
