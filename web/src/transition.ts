// ── Timeline transition rendering ──
// Mirrors crates/opencat-engine/src/backend/skia/transition.rs.
// Records the `from` and `to` subtrees into Skia Pictures via
// CanvasKit's PictureRecorder, then composites the two with the
// chosen transition effect.
//
// SKSL effects are compiled lazily on first use and reused.

import type { DisplayRect, DisplayNodeJson } from './types';

let CK_ref: any = null;
const skslCache = new Map<string, any /* RuntimeEffect */>();

const LIGHT_LEAK_MASK_SKSL = `
uniform float evolveProgress;
uniform float retractProgress;
uniform float seed;
uniform float retractSeed;
uniform float hueShift;
uniform float2 resolution;

const float PI = 3.14159265;

float3 computePattern(float2 uv, float s, float t) {
    float2 p = uv * 0.8;
    p += float2(sin(s * 1.61803) * 5.0, cos(s * 2.71828) * 5.0);

    for (int i = 1; i < 5; i++) {
        float fi = float(i);
        float phase = s * 0.7 * fi;
        float2 nextP = p;
        nextP.x += 0.6 / fi * cos(fi * p.y + t * 0.7 + 0.3 * fi + phase) + 20.0;
        nextP.y += 0.6 / fi * cos(fi * p.x + t * 0.7 + 0.3 * float(i + 10) + phase) - 5.0;
        p = nextP;
    }

    float v1 = 0.5 * sin(2.0 * p.x) + 0.5;
    float v2 = 0.5 * sin(2.0 * p.y) + 0.5;
    float blend = sin(p.x + p.y) * 0.5 + 0.5;
    float brightness = v1 * 0.5 + v2 * 0.5;
    float patternValue = brightness * 0.6 + blend * 0.4;

    return float3(brightness, blend, patternValue);
}

half4 main(float2 coord) {
    float refScale = 1.92;
    float2 uv = (coord / resolution) *
        float2(refScale, refScale * resolution.y / resolution.x);

    float3 patA = computePattern(uv, seed, evolveProgress * PI);
    float threshA = 1.0 - evolveProgress;
    float revealAlpha = smoothstep(threshA, threshA + 0.3, patA.z);

    float2 maxUv = float2(refScale, refScale * resolution.y / resolution.x);
    float2 retractUv = maxUv - uv;
    float3 patB = computePattern(retractUv, seed + 42.0, retractProgress * PI);
    float threshB = 1.0 - retractProgress;
    float eraseAlpha = smoothstep(threshB, threshB + 0.3, patB.z);

    float alpha = revealAlpha * (1.0 - eraseAlpha);

    float3 yellow = float3(1.0, 0.85, 0.2);
    float3 orange = float3(1.0, 0.5, 0.05);
    float3 col = mix(yellow, orange, patA.y);
    col *= 0.6 + 0.6 * patA.x;

    float angle = hueShift * PI / 180.0;
    float cosA = cos(angle);
    float sinA = sin(angle);
    mat3 hueRot = mat3(
        cosA + (1.0 - cosA) / 3.0,
        (1.0 - cosA) / 3.0 - sinA * 0.57735,
        (1.0 - cosA) / 3.0 + sinA * 0.57735,
        (1.0 - cosA) / 3.0 + sinA * 0.57735,
        cosA + (1.0 - cosA) / 3.0,
        (1.0 - cosA) / 3.0 - sinA * 0.57735,
        (1.0 - cosA) / 3.0 - sinA * 0.57735,
        (1.0 - cosA) / 3.0 + sinA * 0.57735,
        cosA + (1.0 - cosA) / 3.0
    );
    col = clamp(hueRot * col, 0.0, 1.0);

    return half4(col.x, col.y, col.z, alpha);
}
`;

const LIGHT_LEAK_COMPOSITE_SKSL = `
uniform shader fromScene;
uniform shader toScene;
uniform shader leakMask;
uniform float progress;

half4 main(float2 coord) {
    half4 mask = leakMask.eval(coord);
    half4 fromColor = fromScene.eval(coord);
    half4 toColor = toScene.eval(coord);
    half alpha = mask.a;
    half4 sceneColor = mix(fromColor, toColor, half(progress));
    half3 leakColor = mask.rgb;
    half3 finalColor = mix(sceneColor.rgb, leakColor, alpha);

    return half4(finalColor, 1.0);
}
`;

export function setCanvasKitForTransition(ck: any): void {
  CK_ref = ck;
}

function getEffect(key: string, sksl: string): any | null {
  if (!CK_ref) return null;
  const cached = skslCache.get(key);
  if (cached) return cached;
  const effect = CK_ref.RuntimeEffect.Make(sksl, (err: string) => {
    console.error(`[transition] SkSL compile failed for ${key}:`, err);
  });
  if (effect) skslCache.set(key, effect);
  return effect;
}

/// Record a sub-tree into a CanvasKit Picture using PictureRecorder.
/// `draw` runs inside a recording canvas; bounds are the timeline's rect.
export function recordPicture(
  bounds: DisplayRect,
  draw: (recordingCanvas: any) => void,
): any | null {
  if (!CK_ref) return null;
  const recorder = new CK_ref.PictureRecorder();
  // Bounds use absolute coordinates so the picture can be drawn back into the
  // parent canvas without an extra translate.
  const cullRect = CK_ref.LTRBRect(
    bounds.x,
    bounds.y,
    bounds.x + bounds.width,
    bounds.y + bounds.height,
  );
  const recCanvas = recorder.beginRecording(cullRect);
  try {
    draw(recCanvas);
  } finally {
    // finishRecordingAsPicture invalidates recCanvas
  }
  const picture = recorder.finishRecordingAsPicture();
  recorder.delete();
  return picture;
}

interface TransitionMeta {
  progress: number;
  kind: {
    type: string;
    direction?: string;
    seed?: number;
    hueShift?: number;
    maskScale?: number;
    name?: string;
  };
}

/// Composite from/to pictures with the requested transition.
/// `bounds` is the timeline bounds in absolute display coordinates.
export function drawTransition(
  canvas: any,
  fromPic: any,
  toPic: any,
  meta: TransitionMeta,
  bounds: DisplayRect,
): void {
  const progress = clamp01(meta.progress);
  const kind = meta.kind.type;

  switch (kind) {
    case 'slide':
      drawSlide(canvas, fromPic, toPic, progress, meta.kind.direction || 'fromLeft', bounds);
      break;
    case 'fade':
      drawFade(canvas, fromPic, toPic, progress);
      break;
    case 'wipe':
      drawWipe(canvas, fromPic, toPic, progress, meta.kind.direction || 'fromLeft', bounds);
      break;
    case 'clockWipe':
      drawClockWipe(canvas, fromPic, toPic, progress, bounds);
      break;
    case 'iris':
      drawIris(canvas, fromPic, toPic, progress, bounds);
      break;
    case 'lightLeak':
      drawLightLeak(canvas, fromPic, toPic, progress, meta.kind, bounds);
      break;
    case 'gl':
      // SKSL-from-GLSL conversion is not yet ported to web — fall back to fade
      // so the timeline keeps animating instead of freezing on the from frame.
      console.warn('[transition] GL transition not yet supported on web; falling back to fade');
      drawFade(canvas, fromPic, toPic, progress);
      break;
    default:
      console.warn('[transition] unknown kind:', kind);
      canvas.drawPicture(fromPic);
      break;
  }
}

// ── Slide ──

function drawSlide(
  canvas: any,
  fromPic: any,
  toPic: any,
  progress: number,
  direction: string,
  bounds: DisplayRect,
): void {
  const w = bounds.width;
  const h = bounds.height;
  let toOffset: [number, number];
  let fromOffset: [number, number];
  switch (direction) {
    case 'fromLeft':
      toOffset = [w * (progress - 1), 0];
      fromOffset = [w * progress, 0];
      break;
    case 'fromRight':
      toOffset = [w * (1 - progress), 0];
      fromOffset = [-w * progress, 0];
      break;
    case 'fromTop':
      toOffset = [0, h * (progress - 1)];
      fromOffset = [0, h * progress];
      break;
    case 'fromBottom':
    default:
      toOffset = [0, h * (1 - progress)];
      fromOffset = [0, -h * progress];
      break;
  }

  canvas.save();
  canvas.translate(toOffset[0], toOffset[1]);
  canvas.drawPicture(toPic);
  canvas.restore();

  canvas.save();
  canvas.translate(fromOffset[0], fromOffset[1]);
  canvas.drawPicture(fromPic);
  canvas.restore();
}

// ── Fade ──

function drawFade(canvas: any, fromPic: any, toPic: any, progress: number): void {
  const CK = CK_ref;
  // Cross-fade via opacity layers. drawPicture has no alpha param, so wrap each
  // in a saveLayer with the proper alpha.
  const fromAlpha = (1 - progress);
  const toAlpha = progress;
  if (fromAlpha > 0) {
    const paint = new CK.Paint();
    paint.setAlphaf(fromAlpha);
    canvas.saveLayer(paint);
    paint.delete();
    canvas.drawPicture(fromPic);
    canvas.restore();
  }
  if (toAlpha > 0) {
    const paint = new CK.Paint();
    paint.setAlphaf(toAlpha);
    canvas.saveLayer(paint);
    paint.delete();
    canvas.drawPicture(toPic);
    canvas.restore();
  }
}

// ── Wipe ──

function drawWipe(
  canvas: any,
  fromPic: any,
  toPic: any,
  progress: number,
  direction: string,
  bounds: DisplayRect,
): void {
  const CK = CK_ref;
  const { x, y, width: w, height: h } = bounds;

  canvas.drawPicture(fromPic);

  let clip: number[];
  switch (direction) {
    case 'fromLeft':
      clip = [x, y, x + w * progress, y + h];
      break;
    case 'fromRight':
      clip = [x + w * (1 - progress), y, x + w, y + h];
      break;
    case 'fromTop':
      clip = [x, y, x + w, y + h * progress];
      break;
    case 'fromBottom':
      clip = [x, y + h * (1 - progress), x + w, y + h];
      break;
    case 'fromTopLeft':
      clip = [x, y, x + w * progress, y + h * progress];
      break;
    case 'fromTopRight':
      clip = [x + w * (1 - progress), y, x + w, y + h * progress];
      break;
    case 'fromBottomLeft':
      clip = [x, y + h * (1 - progress), x + w * progress, y + h];
      break;
    case 'fromBottomRight':
    default:
      clip = [x + w * (1 - progress), y + h * (1 - progress), x + w, y + h];
      break;
  }

  canvas.save();
  canvas.clipRect(
    CK.LTRBRect(clip[0], clip[1], clip[2], clip[3]),
    CK.ClipOp.Intersect,
    true,
  );
  canvas.drawPicture(toPic);
  canvas.restore();
}

// ── Clock Wipe ──

function drawClockWipe(
  canvas: any,
  fromPic: any,
  toPic: any,
  progress: number,
  bounds: DisplayRect,
): void {
  const CK = CK_ref;
  canvas.drawPicture(fromPic);
  if (progress <= 0) return;

  const cx = bounds.x + bounds.width / 2;
  const cy = bounds.y + bounds.height / 2;
  const radius = Math.hypot(bounds.width / 2, bounds.height / 2);

  const startAngleDeg = -90;
  const sweepAngleDeg = progress * 360;
  const startRad = (startAngleDeg * Math.PI) / 180;

  // Pie wedge from center.
  const path = new CK.Path();
  path.moveTo(cx, cy);
  path.lineTo(cx + radius * Math.cos(startRad), cy + radius * Math.sin(startRad));
  const arcRect = CK.LTRBRect(cx - radius, cy - radius, cx + radius, cy + radius);
  path.arcToOval(arcRect, startAngleDeg, sweepAngleDeg, false);
  path.close();

  canvas.save();
  canvas.clipPath(path, CK.ClipOp.Intersect, true);
  canvas.drawPicture(toPic);
  canvas.restore();
  path.delete();
}

// ── Iris ──

function drawIris(
  canvas: any,
  fromPic: any,
  toPic: any,
  progress: number,
  bounds: DisplayRect,
): void {
  const CK = CK_ref;
  canvas.drawPicture(fromPic);
  if (progress <= 0) return;

  const cx = bounds.x + bounds.width / 2;
  const cy = bounds.y + bounds.height / 2;
  const maxRadius = Math.hypot(bounds.width / 2, bounds.height / 2);
  const radius = progress * maxRadius;

  const rrect = CK.RRectXY(
    CK.LTRBRect(cx - radius, cy - radius, cx + radius, cy + radius),
    radius,
    radius,
  );
  canvas.save();
  canvas.clipRRect(rrect, CK.ClipOp.Intersect, true);
  canvas.drawPicture(toPic);
  canvas.restore();
}

// ── Light Leak (SKSL) ──

function drawLightLeak(
  canvas: any,
  fromPic: any,
  toPic: any,
  progress: number,
  params: { seed?: number; hueShift?: number; maskScale?: number },
  bounds: DisplayRect,
): void {
  const CK = CK_ref;
  const seed = params.seed ?? 0;
  const hueShift = params.hueShift ?? 0;
  const rawMaskScale = params.maskScale ?? 1;
  const maskScale = Math.min(1, Math.max(0.03125, rawMaskScale));

  // Render the leak mask to an offscreen surface at reduced resolution
  // to keep the heavy procedural pattern cheap.
  const maskW = Math.max(1, Math.round(bounds.width * maskScale));
  const maskH = Math.max(1, Math.round(bounds.height * maskScale));

  const maskEffect = getEffect('lightLeakMask', LIGHT_LEAK_MASK_SKSL);
  const compositeEffect = getEffect('lightLeakComposite', LIGHT_LEAK_COMPOSITE_SKSL);
  if (!maskEffect || !compositeEffect) {
    console.warn('[transition] light leak SkSL unavailable; falling back to fade');
    drawFade(canvas, fromPic, toPic, progress);
    return;
  }

  const normalized = clamp01(progress);
  const evolveProgress = Math.min(1, normalized * 2);
  const retractProgress = Math.max(0, normalized * 2 - 1);

  const maskUniforms = new Float32Array([
    evolveProgress,
    retractProgress,
    seed,
    seed + 42,
    hueShift,
    maskW,
    maskH,
  ]);

  // Bake mask into an Image via an offscreen surface.
  const maskSurface = CK.MakeSurface(maskW, maskH);
  if (!maskSurface) {
    console.warn('[transition] light leak mask surface failed; falling back to fade');
    drawFade(canvas, fromPic, toPic, progress);
    return;
  }
  const maskCanvas = maskSurface.getCanvas();
  const maskShader = maskEffect.makeShader(maskUniforms);
  const maskPaint = new CK.Paint();
  maskPaint.setShader(maskShader);
  maskCanvas.drawPaint(maskPaint);
  maskPaint.delete();
  const maskImage = maskSurface.makeImageSnapshot();

  // Build shaders from from/to pictures (in absolute coordinates already)
  // and from the mask image. The mask is scaled back up to bounds size.
  const noMatrix = undefined;
  const fromShader = fromPic.makeShader(
    CK.TileMode.Clamp,
    CK.TileMode.Clamp,
    CK.FilterMode.Linear,
    noMatrix,
    undefined,
  );
  const toShader = toPic.makeShader(
    CK.TileMode.Clamp,
    CK.TileMode.Clamp,
    CK.FilterMode.Linear,
    noMatrix,
    undefined,
  );
  // Mask shader covers bounds, so we translate to bounds.x/y and scale 1/maskScale
  // to map mask pixels back to bounds pixels.
  const scaleMatrix = [
    1 / maskScale, 0, bounds.x,
    0, 1 / maskScale, bounds.y,
    0, 0, 1,
  ];
  const maskUpscaledShader = maskImage.makeShaderOptions(
    CK.TileMode.Clamp,
    CK.TileMode.Clamp,
    CK.FilterMode.Linear,
    CK.MipmapMode.None,
    scaleMatrix,
  );

  const compositeUniforms = new Float32Array([normalized]);
  const compositeShader = compositeEffect.makeShaderWithChildren(
    compositeUniforms,
    [fromShader, toShader, maskUpscaledShader],
  );

  const paint = new CK.Paint();
  paint.setShader(compositeShader);
  canvas.save();
  canvas.clipRect(
    CK.LTRBRect(bounds.x, bounds.y, bounds.x + bounds.width, bounds.y + bounds.height),
    CK.ClipOp.Intersect,
    true,
  );
  canvas.drawPaint(paint);
  canvas.restore();
  paint.delete();
  maskImage.delete();
  maskSurface.delete();
}

function clamp01(v: number): number {
  if (v < 0) return 0;
  if (v > 1) return 1;
  return v;
}

// Helper type re-exported for renderer.ts integration.
export type { DisplayNodeJson };
