/// DrawOp → CanvasKit 适配层：paint/shader/filter/path 的 wire 语义到
/// CK 调用的唯一映射处（map* 枚举表、build* 构造、runtime-effect child
/// shader 化）。解码面的类型从这里进来，不回头读字节。

import type {
  BlendMode,
  BlurStyle,
  Canvas,
  CanvasKit,
  ClipOp,
  ColorChannel,
  ColorFilter,
  ColorSpace,
  EmbindEnumEntity,
  FillType,
  FilterMode,
  Image,
  ImageFilter,
  MaskFilter,
  MipmapMode,
  Paint,
  PaintStyle,
  Path,
  PathBuilder,
  PathEffect,
  PointMode,
  Rect,
  ManagedSkottieAnimation,
  RuntimeEffect,
  Shader,
  StrokeCap,
  StrokeJoin,
  TextureSource,
  TileMode,
  Surface,
} from 'canvaskit-wasm';
import { getBlobBytes, getSkottieBundleAssets } from '../wasm';
import { BLEND_MODE, PATH_OP_SUB, PATH_OP_F32_WIDTHS } from '../generated/ocir-schema.generated';
import {
  type Rect4,
  type Range,
  type DecodedFrame,
  type DecodedImageRef,
  type FillSpec,
  type GradientFillSpec,
  type PaintSpec,
  type ImageFilterSpec,
  type FilterChainSpec,
  type FilterInputSpec,
  type ColorFilterSpec,
  type MaskFilterSpec,
  type PathEffectSpec,
  type PathSpec,
  type PathCommand,
  type ChildRef,
  type ShaderSpec,
  type RuntimeEffectSpec,
  type ExecuteRangeOnCanvas,
  type ExecuteSubtreeOnCanvas,
  type Payload,
  MediaValidationError,
  generatedImageRgba,
  generatedImageCanvas,
  NO_PAINT,
} from './decode';
import { getCachedVideoFrameRgba, getCachedVideoFrameSource } from '../media/video-frame-injector';

export type CanvasKitEnum =
  | BlendMode
  | BlurStyle
  | ClipOp
  | FillType
  | FilterMode
  | PaintStyle
  | PointMode
  | StrokeCap
  | StrokeJoin
  | TileMode;

// Derive CanvasKit PascalCase blend-mode names from the generated wire enum so
// mapBlendMode stays drift-proof against additions to the Rust encoder.
// Indexed via `as keyof typeof CK.BlendMode` — BlendModeEnumValues has no string index signature.
const BLEND_MODE_NAMES: readonly string[] = (() => {
  const pascal = (s: string): string =>
    s.split('_').map((p) => p.charAt(0).toUpperCase() + p.slice(1).toLowerCase()).join('');
  return Object.entries(BLEND_MODE)
    .sort(([, a], [, b]) => a - b)
    .map(([key]) => pascal(key));
})();

const lottieCache = new Map<string, ManagedSkottieAnimation>();

export type RenderState = {
  fillColor: [number, number, number, number];
  strokeColor: [number, number, number, number];
  fillGradient: StyleGradientSpec | null;
  strokeGradient: StyleGradientSpec | null;
  lineWidth: number;
  lineCap: number;
  lineJoin: number;
  lineDash: { intervals: number[] | Float32Array; phase: number } | null;
  globalAlpha: number;
  /// u8 wire blend enum（SET_BLEND_MODE sticky state，SrcOver=3）。
  blendMode: number;
  antiAlias: boolean;
};

/// Canvas 2D 方言的 fillStyle/strokeStyle 渐变（SET_STYLE_LINEAR/RADIAL_
/// GRADIENT）。语义对齐 replay.rs：solid style 赋值清掉它，shader 覆盖
/// paint 颜色、globalAlpha 走 paint alpha。
export type StyleGradientSpec =
  | { type: 'linear'; start: [number, number]; end: [number, number]; stops: number[]; colors: number[][] }
  | { type: 'radial'; start: [number, number, number]; end: [number, number, number]; stops: number[]; colors: number[][] };

const staticImageCache = new Map<string, Image>();
/// 已解码的 SVG 静态图（`<img>` 元素，供 MakeImageFromCanvas）。帧回放
/// 前由 `warmStaticSvgImage` 按帧 media plan 预热。
const svgImageCache = new Map<string, HTMLImageElement>();

/**
 * Pre-decode one SVG static image so the synchronous `resolveImage` can
 * hand CanvasKit a canvas-backed image (`MakeImageFromEncoded` has no SVG
 * codec). Idempotent; a failed decode stays a miss and surfaces as the
 * usual MediaValidationError at replay time. `decode()` completes before
 * the object URL is revoked, so the element keeps its decoded pixels.
 */
export async function warmStaticSvgImage(assetId: string): Promise<void> {
  if (svgImageCache.has(assetId) || staticImageCache.has(assetId)) return;
  const bytes = getBlobBytes(assetId);
  if (!bytes || bytes[0] !== 0x3c /* '<' */) return;
  // TS 5.7 的 ArrayBufferLike 严格化：getBlobBytes 的 Uint8Array 视图
  // buffer 可能是 SharedArrayBuffer，Blob 构造面只收定长 ArrayBuffer 视图
  // ——这里的字节来自资产缓存（普通拷贝，永不为 SAB），断言收窄。
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: 'image/svg+xml' }));
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    svgImageCache.set(assetId, img);
  } catch {
    // 保持 miss：回放时报缺，语义与位图解码失败一致。
  } finally {
    URL.revokeObjectURL(url);
  }
}
const effectCache = new Map<bigint, RuntimeEffect>();
export function initialRenderState(): RenderState {
  return {
    fillColor: [0, 0, 0, 1],
    strokeColor: [0, 0, 0, 1],
    fillGradient: null,
    strokeGradient: null,
    lineWidth: 1,
    lineCap: 0,
    lineJoin: 0,
    lineDash: null,
    globalAlpha: 1,
    blendMode: 3,
    antiAlias: true,
  };
}

export type Dispose = <T extends { delete?: () => void } | null | undefined>(obj: T) => T;

export function buildPaintById(CK: CanvasKit, frame: DecodedFrame, paintId: number, dispose: Dispose): Paint {
  return buildPaint(CK, frame.paints[paintId], 1, dispose);
}

export function buildPaint(CK: CanvasKit, spec: PaintSpec | undefined, alpha: number, dispose: Dispose): Paint {
  const paint = new CK.Paint();
  dispose(paint);
  if (!spec) return paint;
  paint.setAntiAlias(spec.antiAlias);
  paint.setStyle(mapPaintStyle(CK, spec.style));
  paint.setBlendMode(mapBlendMode(CK, spec.blendMode));
  applyFill(CK, paint, spec.fill, alpha, dispose);
  if (spec.stroke) {
    paint.setStrokeWidth(spec.stroke.width);
    paint.setStrokeCap(mapStrokeCap(CK, spec.stroke.cap));
    paint.setStrokeJoin(mapStrokeJoin(CK, spec.stroke.join));
    paint.setStrokeMiter(spec.stroke.miterLimit);
  }
  if (spec.imageFilter) paint.setImageFilter(buildImageFilter(CK, spec.imageFilter, dispose));
  if (spec.colorFilter) paint.setColorFilter(buildColorFilter(CK, spec.colorFilter, dispose));
  if (spec.maskFilter) paint.setMaskFilter(buildMaskFilter(CK, spec.maskFilter, dispose));
  if (spec.pathEffect) paint.setPathEffect(buildPathEffect(CK, spec.pathEffect, dispose));
  return paint;
}

export function buildScriptPaint(CK: CanvasKit, state: RenderState, style: 'fill' | 'stroke', dispose: Dispose): Paint {
  const paint = new CK.Paint();
  dispose(paint);
  const gradient = style === 'fill' ? state.fillGradient : state.strokeGradient;
  if (gradient) {
    // Shader-painted paths take their coverage from the shader: globalAlpha
    // stays in the paint alpha, the solid color is untouched (replay.rs
    // apply_global_alpha's shader branch).
    paint.setShader(buildStyleGradientShader(CK, gradient, dispose));
    paint.setAlphaf(state.globalAlpha);
  } else {
    const color = [...(style === 'fill' ? state.fillColor : state.strokeColor)] as [number, number, number, number];
    color[3] *= state.globalAlpha;
    paint.setColor(CK.Color4f(color[0], color[1], color[2], color[3]));
  }
  paint.setAntiAlias(state.antiAlias);
  paint.setStyle(style === 'fill' ? CK.PaintStyle.Fill : CK.PaintStyle.Stroke);
  paint.setBlendMode(mapBlendMode(CK, state.blendMode));
  if (style === 'stroke') {
    paint.setStrokeWidth(state.lineWidth);
    paint.setStrokeCap(mapLineCap(CK, state.lineCap));
    paint.setStrokeJoin(mapLineJoin(CK, state.lineJoin));
    if (state.lineDash) {
      // MakeDash 形参 TS 面是 number[]，JS 运行时接受 Float32Array（F32_POOL
      // 零拷贝视图已全线跑通）——窄化断言保零拷贝。
      paint.setPathEffect(dispose(CK.PathEffect.MakeDash(state.lineDash.intervals as number[], state.lineDash.phase)));
    }
  }
  return paint;
}

/// Style-gradient stops from the f32Pool: `len / 5` entries of
/// [pos, r, g, b, a] with u8-domain colors (replay.rs rounds them back to
/// u8). Colors stay 0..1 unpremul; the shader flags premul interpolation.
export function parseStyleGradientStops(p: Payload, frame: DecodedFrame): { stops: number[]; colors: number[][] } {
  const poolStart = p.u32();
  const len = p.u32();
  const count = Math.floor(len / 5);
  if (poolStart + len > frame.f32Pool.length) {
    throw new Error(`SetStyleGradient f32_pool out of bounds (${poolStart}+${len} > ${frame.f32Pool.length})`);
  }
  const stops: number[] = [];
  const colors: number[][] = [];
  for (let i = 0; i < count; i++) {
    const o = poolStart + i * 5;
    stops.push(frame.f32Pool[o]);
    colors.push([
      frame.f32Pool[o + 1] / 255,
      frame.f32Pool[o + 2] / 255,
      frame.f32Pool[o + 3] / 255,
      frame.f32Pool[o + 4] / 255,
    ]);
  }
  return { stops, colors };
}

export function buildStyleGradientShader(CK: CanvasKit, grad: StyleGradientSpec, dispose: Dispose): Shader {
  const colors = grad.colors.map((c) => CK.Color4f(c[0], c[1], c[2], c[3]));
  // flags bit 1 = interpolate stops in premul space (replay.rs
  // INTERPOLATE_COLORS_IN_PREMUL); tile always Clamp.
  if (grad.type === 'linear') {
    return dispose(CK.Shader.MakeLinearGradient(grad.start, grad.end, colors, grad.stops, CK.TileMode.Clamp, undefined, 1));
  }
  // Two-point conical: radii travel as explicit args, the centers must be
  // plain [x,y] — CanvasKit marshals start/end through a 4-float wasm scratch
  // (`C.set(start); C.set(end, 2)`), so 3-element [x,y,r] circles overflow it
  // with `RangeError: offset is out of bounds`.
  return dispose(CK.Shader.MakeTwoPointConicalGradient(
    [grad.start[0], grad.start[1]], grad.start[2],
    [grad.end[0], grad.end[1]], grad.end[2],
    colors, grad.stops, CK.TileMode.Clamp, undefined, 1,
  ));
}

export function applyFill(CK: CanvasKit, paint: Paint, fill: FillSpec, alpha: number, dispose: Dispose): void {
  if (fill.type === 'solid') {
    paint.setColor(CK.Color4f(fill.color[0], fill.color[1], fill.color[2], fill.color[3] * alpha));
    paint.setShader(null);
    return;
  }
  const shader = buildShader(CK, fill, dispose);
  if (!shader) {
    paint.setShader(null);
    return;
  }
  dispose(shader);
  paint.setShader(shader);
}

export function buildShader(CK: CanvasKit, fill: GradientFillSpec | ShaderSpec, dispose: Dispose): Shader | null {
  // 仅 GradientFillSpec（paint fill）携带 localMatrix；ShaderSpec（runtime effect child）不携带。
  const localMatrix = 'localMatrix' in fill && fill.localMatrix ? fill.localMatrix : undefined;
  if (fill.type === 'turbulence') {
    return buildTurbulenceShader(CK, fill, dispose);
  }
  // CSS Images 4 interpolates stops in premultiplied space
  // (flags bit 1); SVG/resvg semantics are unpremul — the encoded
  // interpolatePremul byte picks per-shader. ShaderSpec children without the
  // field default to premul.
  const gradFlags = 'interpolatePremul' in fill ? fill.interpolatePremul : 1;
  if (fill.type === 'sweepGradient') {
    // CSS conic-gradient → official Skia sweep. `startAngle`/`endAngle` arrive
    // already converted to Skia semantics (degrees, 0 = +x axis, clockwise).
    return CK.Shader.MakeSweepGradient(
      fill.center[0],
      fill.center[1],
      fill.colors.map((c) => CK.Color4f(c[0], c[1], c[2], c[3])),
      fill.stops,
      mapTileMode(CK, fill.tileMode),
      localMatrix as any,
      gradFlags,
      fill.startAngle,
      fill.endAngle,
    );
  }
  if (fill.type === 'linearGradient') {    return CK.Shader.MakeLinearGradient(
      'from' in fill ? fill.from : fill.start,
      'to' in fill ? fill.to : fill.end,
      fill.colors.map((c) => CK.Color4f(c[0], c[1], c[2], c[3])),
      fill.stops,
      mapTileMode(CK, 'tileMode' in fill ? fill.tileMode : 0),
      localMatrix as any,
      gradFlags,
    );
  }
  if ('focal' in fill && fill.focal) {
    // SVG focal radial: two-point conical (focus circle → main circle), same
    // semantics as tiny-skia RadialGradient (what resvg uses).
    const [fx, fy, fr] = fill.focal;
    return CK.Shader.MakeTwoPointConicalGradient(
      [fx, fy],
      fr,
      fill.center,
      fill.radius,
      fill.colors.map((c) => CK.Color4f(c[0], c[1], c[2], c[3])),
      fill.stops,
      mapTileMode(CK, 'tileMode' in fill ? fill.tileMode : 0),
      localMatrix as any,
      gradFlags,
    );
  }
  return CK.Shader.MakeRadialGradient(
    fill.center,
    fill.radius,
    fill.colors.map((c) => CK.Color4f(c[0], c[1], c[2], c[3])),
    fill.stops,
    mapTileMode(CK, 'tileMode' in fill ? fill.tileMode : 0),
    localMatrix as any,
    gradFlags,
  );
}

// Mirror of the engine's feTurbulence bake (executor/paint.rs): MakeTurbulence
// produces the same tileable Perlin field for a given tile size; bake one tile
// into a raster surface and repeat it, anchored at the node origin. The engine
// also runs a linear→sRGB table filter over the tile — CanvasKit has no table
// filter, but that shift is RGB-only (the alpha channel, which drives visibility
// on these ≈0.012-opacity layers, matches exactly) and sits far below the
// k3diff gates, so it is intentionally skipped.
export function buildTurbulenceShader(CK: CanvasKit, fill: Extract<FillSpec, { type: 'turbulence' }>, dispose: Dispose): Shader | null {
  if (!fill.tile) return null;
  const tw = Math.round(fill.tile[0]);
  const th = Math.round(fill.tile[1]);
  if (tw <= 0 || th <= 0) return null;
  const surface = CK.MakeSurface(tw, th);
  if (!surface) return null;
  let shader: Shader | null = null;
  try {
    const noise = dispose(CK.Shader.MakeTurbulence(
      fill.baseFrequency[0],
      fill.baseFrequency[1],
      fill.numOctaves,
      fill.seed,
      tw,
      th,
    ));
    const tilePaint = dispose(new CK.Paint());
    tilePaint.setShader(noise);
    surface.getCanvas().drawPaint(tilePaint);
    const tile = surface.makeImageSnapshot();
    if (tile) {
      dispose(tile);
      shader = tile.makeShaderOptions(
        CK.TileMode.Repeat,
        CK.TileMode.Repeat,
        CK.FilterMode.Nearest,
        CK.MipmapMode.None,
        CK.Matrix.translated(fill.origin[0], fill.origin[1]) as any,
      );
    }
  } finally {
    surface.delete();
  }
  return shader;
}

export function buildImageFilter(CK: CanvasKit, spec: ImageFilterSpec, dispose: Dispose): ImageFilter | null {
  if (spec.type === 'blur') {
    return dispose(CK.ImageFilter.MakeBlur(
      spec.sigmaX,
      spec.sigmaY,
      spec.decal ? CK.TileMode.Decal : CK.TileMode.Clamp,
      null,
    ));
  }
  if (spec.type === 'dropShadow') {
    const make = spec.keepContent
      ? CK.ImageFilter.MakeDropShadow
      : CK.ImageFilter.MakeDropShadowOnly;
    return dispose(make(
      spec.dx,
      spec.dy,
      spec.sigmaX,
      spec.sigmaY,
      CK.Color4f(spec.color[0], spec.color[1], spec.color[2], spec.color[3]),
      null,
    ));
  }
  if (spec.type === 'colorFilter') return dispose(CK.ImageFilter.MakeColorFilter(buildColorFilter(CK, spec.filter, dispose), null));
  if (spec.type === 'chain') return buildFilterChains(CK, spec.chains, dispose);
  return dispose(CK.ImageFilter.MakeCompose(buildImageFilter(CK, spec.outer, dispose), buildImageFilter(CK, spec.inner, dispose)));
}

// ── SVG filter chain（FilterChainSpec → CanvasKit filter 树）────────
// 语义镜像 engine `executor/paint.rs` 的 build_filter_chain 族：从 output
// primitive 递归展开，Prim 引用按消费点重复展开（DAG 共享子链独立求值）；
// 段式色彩空间（per-primitive color-interpolation-filters）用 gamma color
// filter 在域切换处插桩，链输出恒 sRGB。CanvasKit 这一版的工厂没有
// cropRect 参数，prim subregion 与 filter region 恒等（usvg 默认）时忽略，
// 显式 subregion fail-loud。

/// SourceAlpha：输出 (0,0,0,a)（黑体 + alpha）。
const SOURCE_ALPHA_MATRIX = [
  0.0, 0.0, 0.0, 0.0, 0.0,
  0.0, 0.0, 0.0, 0.0, 0.0,
  0.0, 0.0, 0.0, 0.0, 0.0,
  0.0, 0.0, 0.0, 1.0, 0.0,
];

function buildFilterChains(CK: CanvasKit, chains: FilterChainSpec[], dispose: Dispose): ImageFilter {
  // `url(#a) url(#b)` 从左到右求值：compose(outer, inner) = outer(inner)，
  // 故从右往左包 compose。input=null 即层快照（backdrop 语义一致）。
  let acc = buildFilterChain(CK, chains[chains.length - 1], dispose);
  for (let i = chains.length - 2; i >= 0; i--) {
    acc = dispose(CK.ImageFilter.MakeCompose(acc, buildFilterChain(CK, chains[i], dispose)));
  }
  return acc;
}

function buildFilterChain(CK: CanvasKit, chain: FilterChainSpec, dispose: Dispose): ImageFilter {
  if (chain.primitives.length === 0) throw new Error('svg filter chain has no primitives');
  return buildFilterPrim(CK, chain, chain.output, true, dispose);
}

function buildFilterInput(
  CK: CanvasKit,
  chain: FilterChainSpec,
  input: FilterInputSpec,
  wantSrgb: boolean,
  dispose: Dispose,
): ImageFilter | null {
  if (input.type === 'sourceGraphic') {
    // SourceGraphic = 无 input（层快照）；linear 域消费时插 sRGB→linear。
    if (wantSrgb) return null;
    return dispose(CK.ImageFilter.MakeColorFilter(dispose(CK.ColorFilter.MakeSRGBToLinearGamma()), null));
  }
  if (input.type === 'sourceAlpha') {
    // SourceAlpha 只保留 α，色彩空间无关，按 sRGB 计。
    let f: ImageFilter = dispose(
      CK.ImageFilter.MakeColorFilter(
        dispose(CK.ColorFilter.MakeMatrix(SOURCE_ALPHA_MATRIX)),
        null,
      ),
    );
    if (!wantSrgb) {
      f = dispose(CK.ImageFilter.MakeColorFilter(dispose(CK.ColorFilter.MakeSRGBToLinearGamma()), f));
    }
    return f;
  }
  return buildFilterPrim(CK, chain, input.index, wantSrgb, dispose);
}

function buildFilterPrim(
  CK: CanvasKit,
  chain: FilterChainSpec,
  idx: number,
  outSrgb: boolean,
  dispose: Dispose,
): ImageFilter {
  const p = chain.primitives[idx];
  if (!p) throw new Error(`svg filter prim ${idx} out of range`);
  const region = chain.sourceCrop;
  if (p.crop && region && !rectClose(p.crop, region)) {
    // CanvasKit 工厂无 cropRect；显式 prim subregion 不可表达，fail-loud。
    throw new Error(`svg filter prim subregion not supported on web (${p.crop.join(',')})`);
  }
  const inputOf = (i: FilterInputSpec) => buildFilterInput(CK, chain, i, p.srgb, dispose);
  // 各臂输出的原生域：消费 source 的 prim = primitive 的 cs；Flood 的
  // shader 输出恒 sRGB 数值（同 engine 的 native_srgb 选择理由）。
  let nativeSrgb = p.srgb;
  let inner: ImageFilter | null;
  switch (p.kind.type) {
    case 'blend':
      // engine: blend(mode, dst=in2, src=in1) ↔ MakeBlend(blend, bg, fg)。
      inner = dispose(CK.ImageFilter.MakeBlend(mapBlendMode(CK, p.kind.mode), inputOf(p.kind.in2), inputOf(p.kind.in1)));
      break;
    case 'colorMatrix':
      inner = dispose(CK.ImageFilter.MakeColorFilter(dispose(CK.ColorFilter.MakeMatrix(p.kind.rowMajor)), inputOf(p.kind.input)));
      break;
    case 'componentTransfer':
      // CanvasKit 无 table color filter（engine 用 table_argb 256 查表）。
      throw new Error('svg feComponentTransfer is not replayed on web yet');
    case 'composite': {
      const op = p.kind.op;
      if (op > 5) throw new Error(`svg feComposite arithmetic (op ${op}) is not replayed on web yet`);
      const modes = ['SrcOver', 'SrcIn', 'SrcOut', 'SrcATop', 'Xor', 'Plus'] as const;
      inner = dispose(
        CK.ImageFilter.MakeBlend(
          // 单值 enum 成员索引访问在类型面混入梯度 enum 的 number[] 臂；
          // BlendMode 这六个成员都是单值实体。
          CK.BlendMode[modes[op] as keyof typeof CK.BlendMode] as EmbindEnumEntity,
          inputOf(p.kind.in2),
          inputOf(p.kind.in1),
        ),
      );
      break;
    }
    case 'convolveMatrix':
      throw new Error('svg feConvolveMatrix is not replayed on web yet');
    case 'diffuseLighting':
    case 'specularLighting':
      throw new Error(`svg ${p.kind.type} is not replayed on web yet`);
    case 'displacementMap':
      inner = dispose(
        CK.ImageFilter.MakeDisplacementMap(
          filterChannel(CK, p.kind.xChannel),
          filterChannel(CK, p.kind.yChannel),
          p.kind.scale,
          inputOf(p.kind.in2),
          inputOf(p.kind.input),
        ),
      );
      break;
    case 'dropShadow':
      inner = dispose(
        CK.ImageFilter.MakeDropShadow(
          p.kind.dx,
          p.kind.dy,
          p.kind.sigmaX,
          p.kind.sigmaY,
          CK.Color4f(p.kind.color[0], p.kind.color[1], p.kind.color[2], p.kind.color[3]),
          inputOf(p.kind.input),
        ),
      );
      break;
    case 'flood':
      nativeSrgb = true;
      // space=null = canvaskit 缺省 sRGB（运行时接受，类型面未标可空）。
      inner = dispose(CK.ImageFilter.MakeShader(dispose(CK.Shader.MakeColor(CK.Color4f(p.kind.color[0], p.kind.color[1], p.kind.color[2], p.kind.color[3]), null as unknown as ColorSpace))));
      break;
    case 'gaussianBlur':
      inner = dispose(CK.ImageFilter.MakeBlur(p.kind.sigmaX, p.kind.sigmaY, CK.TileMode.Decal, inputOf(p.kind.input)));
      break;
    case 'merge': {
      // merge = 输入按序 SrcOver 叠放（SkImageFilters::Merge 语义），
      // CanvasKit 无 MakeMerge，用嵌套 MakeBlend 折叠。
      if (p.kind.inputs.length === 0) throw new Error('svg feMerge has no inputs');
      let acc: ImageFilter | null = inputOf(p.kind.inputs[0]);
      for (let i = 1; i < p.kind.inputs.length; i++) {
        acc = dispose(CK.ImageFilter.MakeBlend(CK.BlendMode.SrcOver, acc, inputOf(p.kind.inputs[i])));
      }
      inner = acc;
      break;
    }
    case 'morphology':
      inner = p.kind.erode
        ? dispose(CK.ImageFilter.MakeErode(p.kind.radiusX, p.kind.radiusY, inputOf(p.kind.input)))
        : dispose(CK.ImageFilter.MakeDilate(p.kind.radiusX, p.kind.radiusY, inputOf(p.kind.input)));
      break;
    case 'offset':
      inner = dispose(CK.ImageFilter.MakeOffset(p.kind.dx, p.kind.dy, inputOf(p.kind.input)));
      break;
    case 'tile':
    case 'image':
      throw new Error(`svg fe${p.kind.type} is not replayed on web yet`);
    case 'turbulence':
      throw new Error('svg feTurbulence inside a filter chain is not replayed on web yet');
  }
  inner = inner as ImageFilter;
  // 输出域到消费方要求域的转换（同域零开销）。
  if (nativeSrgb === outSrgb) return inner;
  const gamma = nativeSrgb
    ? dispose(CK.ColorFilter.MakeSRGBToLinearGamma())
    : dispose(CK.ColorFilter.MakeLinearToSRGBGamma());
  return dispose(CK.ImageFilter.MakeColorFilter(gamma, inner));
}

function filterChannel(CK: CanvasKit, c: number): ColorChannel {
  const names = ['Red', 'Green', 'Blue', 'Alpha'] as const;
  const name = names[c] ?? 'Alpha';
  // 单值 enum 成员的索引访问在类型面混入梯度 enum 的 number[] 臂；
  // Red/Green/Blue/Alpha 都是单值实体。
  return CK.ColorChannel[name as keyof typeof CK.ColorChannel] as ColorChannel;
}

/// prim subregion 与 filter region 恒等判定（emit 侧同源数值，容差只防
/// f32 往返噪声）。
function rectClose(a: [number, number, number, number], b: [number, number, number, number]): boolean {
  const eps = 1e-3;
  return Math.abs(a[0] - b[0]) < eps && Math.abs(a[1] - b[1]) < eps
    && Math.abs(a[2] - b[2]) < eps && Math.abs(a[3] - b[3]) < eps;
}

export function buildColorFilter(CK: CanvasKit, spec: ColorFilterSpec, dispose: Dispose): ColorFilter {
  if (spec.type === 'matrix') return dispose(CK.ColorFilter.MakeMatrix(spec.matrix));
  if (spec.type === 'blendColor') return dispose(CK.ColorFilter.MakeBlend(CK.Color4f(spec.color[0], spec.color[1], spec.color[2], spec.color[3]), mapBlendMode(CK, spec.mode)));
  if (spec.type === 'linearToSrgbGamma') return dispose(CK.ColorFilter.MakeLinearToSRGBGamma());
  return dispose(CK.ColorFilter.MakeSRGBToLinearGamma());
}

export function buildMaskFilter(CK: CanvasKit, spec: MaskFilterSpec, dispose: Dispose): MaskFilter {
  return dispose(CK.MaskFilter.MakeBlur(mapBlurStyle(CK, spec.style), spec.sigma, spec.respectCtm));
}

export function buildPathEffect(CK: CanvasKit, spec: PathEffectSpec, dispose: Dispose): PathEffect {
  return dispose(CK.PathEffect.MakeDash(spec.intervals as number[], spec.phase));
}

export function buildPathById(CK: CanvasKit, frame: DecodedFrame, id: number, pathCache: Map<number, Path>, dispose: Dispose): Path {
  // Cache is frame-scoped: `frame.paths[id]` spec objects are fresh per decoded
  // frame, so a module-level WebMap keyed on them would keep every frame's
  // paths alive until GC — the same unbounded growth the disposer exists to
  // stop. Keyed by id it is freed with the frame.
  const cached = pathCache.get(id);
  if (cached) return cached;
  const spec = frame.paths[id];
  if (!spec) throw new Error(`Missing path ${id}`);
  const builder = dispose(new CK.PathBuilder());
  for (const op of spec.ops) applyPathCommand(CK, builder, op);
  const path = dispose(builder.snapshot());
  path.setFillType?.(mapFillType(CK, spec.fillType));
  pathCache.set(id, path);
  return path;
}

export function applyPathPayload(CK: CanvasKit, builder: PathBuilder, payload: Payload, kind: number): void {
  // v6: `kind` is the PathOp sub-opcode (0..=8) carried by the top-level
  // opcode; the payload is the raw f32 command args (no embedded sub-opcode).
  const width = PATH_OP_F32_WIDTHS[kind] ?? 0;
  applyPathCommand(CK, builder, { kind, values: payload.f32Array(width) });
}

export function applyPathCommand(CK: CanvasKit, builder: PathBuilder, op: PathCommand): void {
  const v = op.values;
  switch (op.kind) {
    case PATH_OP_SUB.MOVE_TO:
      builder.moveTo(v[0], v[1]);
      break;
    case PATH_OP_SUB.LINE_TO:
      builder.lineTo(v[0], v[1]);
      break;
    case PATH_OP_SUB.QUAD_TO:
      builder.quadTo(v[0], v[1], v[2], v[3]);
      break;
    case PATH_OP_SUB.CUBIC_TO:
      builder.cubicTo(v[0], v[1], v[2], v[3], v[4], v[5]);
      break;
    case PATH_OP_SUB.CLOSE:
      builder.close();
      break;
    case PATH_OP_SUB.ADD_RECT:
      builder.addRect(CK.XYWHRect(v[0], v[1], v[2], v[3]));
      break;
    case PATH_OP_SUB.ADD_RRECT:
      builder.addRRect(CK.RRectXY(CK.XYWHRect(v[0], v[1], v[2], v[3]), v[4], v[4]));
      break;
    case PATH_OP_SUB.ADD_OVAL:
      builder.addOval(CK.XYWHRect(v[0], v[1], v[2], v[3]));
      break;
    case PATH_OP_SUB.ADD_ARC: {
      // Stream arc angles are radians (canvas semantics); CanvasKit takes
      // degrees. The third argument is a SIGNED SWEEP despite the type name
      // `endAngle` (probe: arcToOval(rect, 90, 0, false) is an empty single
      // point, arcToOval(rect, 0, -90, false) is the upper-right quarter).
      // forceMoveTo=false: arcs continue the contour (a wedge's inner arc
      // must not split it); addArc hardcodes forceMoveTo=true.
      const deg = 180 / Math.PI;
      builder.arcToOval(CK.XYWHRect(v[0], v[1], v[2], v[3]), v[4] * deg, v[5] * deg, false);
      break;
    }
    default:
      throw new Error(`Unsupported PathOp ${op.kind}`);
  }
}

export function resolveLottieAnimation(CK: CanvasKit, bundleId: string): ManagedSkottieAnimation | null {
  const cached = lottieCache.get(bundleId);
  if (cached) return cached;
  const bytes = getBlobBytes(bundleId);
  if (!bytes) return null;
  const json = new TextDecoder().decode(bytes);
  const rawAssets = getSkottieBundleAssets(bundleId);
  const assets: Record<string, ArrayBuffer> = {};
  for (const [key, val] of Object.entries(rawAssets)) {
    const copy = new ArrayBuffer(val.byteLength);
    new Uint8Array(copy).set(val);
    assets[key] = copy;
  }
  const anim = CK.MakeManagedAnimation(json, assets);
  if (!anim) return null;
  lottieCache.set(bundleId, anim);
  return anim;
}

export function resolveImage(
  CK: CanvasKit,
  image: DecodedImageRef,
  surface: Surface | undefined,
  transientImageCache: Map<string, Image>,
): Image | null {
  if (image.type === 'static') {
    const existing = staticImageCache.get(image.assetId);
    if (existing) return existing;
    const bytes = getBlobBytes(image.assetId);
    if (!bytes) return null;
    // SVG 位图资产：CanvasKit 的编码解码器不认 SVG（Skia 无 svg codec），
    // 走浏览器 img 解码（与 swgl 参考同一渲染器），字节以 '<' 开头嗅探。
    // 解码由 warmStaticSvgImage 在帧回放前完成；未预热的资源落 null，
    // 由调用方走既有 MediaValidationError 报缺。
    if (bytes[0] === 0x3c /* '<' */) {
      const img = svgImageCache.get(image.assetId);
      if (!img) return null;
      const ckImage = CK.MakeImageFromCanvasImageSource(img as unknown as HTMLCanvasElement);
      if (ckImage) staticImageCache.set(image.assetId, ckImage);
      return ckImage;
    }
    const ckImage = CK.MakeImageFromEncoded(bytes);
    if (ckImage) staticImageCache.set(image.assetId, ckImage);
    return ckImage;
  }

  if (image.type === 'generated') {
    // Streaming: generated pixels are re-produced every frame (emoji glyphs
    // re-rasterized by core, GPU stream targets re-executed by the
    // browser-native executors), so the image is built and consumed exactly
    // once per frame — no cache. The draw sites delete it right after
    // drawing (`image.type === 'generated'` there), and the slot is dropped
    // as soon as it has been wrapped/copied.
    const key = `${image.id}`;
    // WebGL2 fast path (batch B): the executor hands over the stream's
    // OffscreenCanvas itself — presentation order (top-down), no row flip,
    // no full-frame CPU readback. Invariants (see decode.ts
    // `generatedImageCanvas`): one-frame-one-consumption (delete on take;
    // render-session prunes unconsumed ids from the previous frame before
    // the executors run) and same-task validity (the executor registered
    // the canvas in this same JS task — the back buffer is intact here).
    // MakeImageFromCanvasImageSource snapshots the canvas as an image
    // source (drawImage → getImageData) and declares Unpremul/RGBA_8888/
    // SRGB. Alpha: the source context kept its recorded premultipliedAlpha
    // attr — if k3diff rules alpha drift vs the old readPixels path, the
    // fix point is a premul adaptation around this call (or an explicit
    // ImageInfo via MakeImage), NOT the context creation attrs.
    const gpuCanvas = generatedImageCanvas.get(key);
    if (gpuCanvas) {
      const ckImage = CK.MakeImageFromCanvasImageSource(gpuCanvas.canvas);
      generatedImageCanvas.delete(key);
      return ckImage;
    }
    // Fallback (core-rasterized emoji section 12, wgpu CPU readbacks):
    // raw RGBA bytes → MakeImage with an explicit Unpremul info.
    const rgba = generatedImageRgba.get(key);
    if (!rgba) return null;
    const ckImage = CK.MakeImage(
      {
        width: rgba.width,
        height: rgba.height,
        colorType: CK.ColorType.RGBA_8888,
        alphaType: CK.AlphaType.Unpremul,
        colorSpace: CK.ColorSpace.SRGB,
      },
      rgba.rgba,
      rgba.width * 4,
    );
    generatedImageRgba.delete(key);
    return ckImage;
  }

  // encoded refs（字节在 IR bytes section，web 尚不消费）走到这里返回
  // null，由调用方抛 MediaValidationError（带 `encoded:<byteId>` 描述）。
  if (image.type !== 'video') return null;
  const transientKey = `${image.assetId}\0${image.timeMicros}`;
  const existing = transientImageCache.get(transientKey);
  if (existing) return existing;

  const source = getCachedVideoFrameSource(image.assetId, image.timeMicros);
  if (source) {
    const info = {
      width: source.width,
      height: source.height,
      colorType: CK.ColorType.RGBA_8888,
      alphaType: CK.AlphaType.Unpremul,
      colorSpace: CK.ColorSpace.SRGB,
    };
    const textureSource = source.source as unknown as TextureSource;
    const ckImage = typeof surface?.makeImageFromTextureSource === 'function'
      ? surface.makeImageFromTextureSource(textureSource, info)
      : CK.MakeLazyImageFromTextureSource?.(textureSource, info);
    if (ckImage) {
      transientImageCache.set(transientKey, ckImage);
      return ckImage;
    }
  }

  const cached = getCachedVideoFrameRgba(image.assetId, image.timeMicros);
  if (!cached) return null;
  const ckImage = CK.MakeImage(
    {
      width: cached.width,
      height: cached.height,
      colorType: CK.ColorType.RGBA_8888,
      alphaType: CK.AlphaType.Unpremul,
      colorSpace: CK.ColorSpace.SRGB,
    },
    cached.rgba,
    cached.width * 4,
  );
  if (ckImage) transientImageCache.set(transientKey, ckImage);
  return ckImage;
}

export function drawRuntimeEffect(
  CK: CanvasKit,
  canvas: Canvas,
  frame: DecodedFrame,
  payload: Payload,
  executeRangeOnCanvas: ExecuteRangeOnCanvas,
  executeSubtreeOnCanvas: ExecuteSubtreeOnCanvas,
  resolveFrameImage: (image: DecodedImageRef) => Image | null,
  dispose: Dispose,
): void {
  const effectId = payload.u32();
  const uniformRangeId = payload.u32();
  const childStart = payload.u32();
  const childLen = payload.u32();
  const dst = readRect4(payload);
  const spec = frame.effects[effectId];
  if (!spec) {
    throw new MediaValidationError(
      'runtimeEffect',
      `effectId:${effectId}`,
      `RuntimeEffect spec missing at index ${effectId}`,
    );
  }
  let effect: RuntimeEffect | undefined = effectCache.get(spec.hash);
  if (!effect) {
    const compiled = CK.RuntimeEffect.Make(spec.sksl);
    if (!compiled) {
      throw new MediaValidationError(
        'runtimeEffect',
        `0x${spec.hash.toString(16)}`,
        `RuntimeEffect compilation failed for hash ${spec.hash}`,
      );
    }
    effect = compiled;
    effectCache.set(spec.hash, effect);
  }
  const range = frame.byteRanges[uniformRangeId];
  const bytes = range ? frame.rawBytes.subarray(range.start, range.start + range.len) : new Uint8Array();
  const uniforms = new Float32Array(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
  const children = frame.children.slice(childStart, childStart + childLen)
    .map((child) => buildRuntimeChildShader(CK, frame, child, dst, executeRangeOnCanvas, executeSubtreeOnCanvas, resolveFrameImage, dispose))
    .filter((child): child is Shader => child !== null);
  const shader = dispose(children.length > 0 ? effect.makeShaderWithChildren(uniforms, children) : effect.makeShader(uniforms));
  const paint = dispose(new CK.Paint());
  paint.setShader(shader);
  canvas.drawRect(ckRect(CK, dst), paint);
}

export function buildRuntimeChildShader(
  CK: CanvasKit,
  frame: DecodedFrame,
  child: ChildRef,
  dst: Rect4,
  executeRangeOnCanvas: ExecuteRangeOnCanvas,
  executeSubtreeOnCanvas: ExecuteSubtreeOnCanvas,
  resolveFrameImage: (image: DecodedImageRef) => Image | null,
  dispose: Dispose,
): Shader | null {
  if (child.type === 'shader') return dispose(buildShader(CK, child.shader, dispose));
  if (child.type === 'image') {
    const img = resolveFrameImage(child.image);
    if (!img?.makeShaderOptions) return null;
    const shader = img.makeShaderOptions(CK.TileMode.Clamp, CK.TileMode.Clamp, CK.FilterMode.Linear, CK.MipmapMode.None);
    // Streaming: generated images are one-shot per frame; the shader holds
    // its own pixel reference, so the image wrapper is dropped right away.
    if (child.image.type === 'generated') img.delete();
    return dispose(shader);
  }
  const width = Math.max(1, Math.ceil(dst.x + dst.width));
  const height = Math.max(1, Math.ceil(dst.y + dst.height));
  const recorder = new CK.PictureRecorder();
  const canvas = recorder.beginRecording(CK.XYWHRect(0, 0, width, height));
  if (child.type === 'subtreePicture') executeSubtreeOnCanvas(canvas, child.subtree);
  else executeRangeOnCanvas(canvas, child.range.start, child.range.len);
  const picture = recorder.finishRecordingAsPicture();
  recorder.delete();
  dispose(picture);
  return dispose(picture?.makeShader?.(CK.TileMode.Clamp, CK.TileMode.Clamp, CK.FilterMode?.Linear) ?? null);
}

export function readRect4(payload: Payload): Rect4 {
  return { x: payload.f32(), y: payload.f32(), width: payload.f32(), height: payload.f32() };
}

export function readDRRect(payload: Payload): { rect: Rect4; radii: number[] } {
  return { rect: readRect4(payload), radii: payload.f32Array(4) };
}

export function ckRect(CK: CanvasKit, rect: Rect4): Rect {
  return CK.XYWHRect ? CK.XYWHRect(rect.x, rect.y, rect.width, rect.height) : Float32Array.of(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height);
}

export function ckRRect(rect: Rect4, radii: number[]): Float32Array {
  return Float32Array.of(
    rect.x,
    rect.y,
    rect.x + rect.width,
    rect.y + rect.height,
    radii[0] ?? 0,
    radii[0] ?? 0,
    radii[1] ?? 0,
    radii[1] ?? 0,
    radii[2] ?? 0,
    radii[2] ?? 0,
    radii[3] ?? 0,
    radii[3] ?? 0,
  );
}

export function imageBounds(CK: CanvasKit, image: Image): Rect {
  const width = typeof image.width === 'function' ? image.width() : 0;
  const height = typeof image.height === 'function' ? image.height() : 0;
  return CK.XYWHRect(0, 0, width, height);
}

export function colorU32ToF32(color: number): [number, number, number, number] {
  return [
    (color & 0xff) / 255,
    ((color >>> 8) & 0xff) / 255,
    ((color >>> 16) & 0xff) / 255,
    ((color >>> 24) & 0xff) / 255,
  ];
}

export function mapPaintStyle(CK: CanvasKit, value: number): CanvasKitEnum {
  return value === 1 ? CK.PaintStyle.Stroke : CK.PaintStyle.Fill;
}

export function mapBlendMode(CK: CanvasKit, value: number): CanvasKitEnum {
  const name = (BLEND_MODE_NAMES[value] ?? 'SrcOver') as keyof typeof CK.BlendMode;
  return CK.BlendMode[name] as CanvasKitEnum;
}

export function mapStrokeCap(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.StrokeCap.Butt, CK.StrokeCap.Round, CK.StrokeCap.Square][value] ?? CK.StrokeCap.Butt;
}

export function mapStrokeJoin(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.StrokeJoin.Miter, CK.StrokeJoin.Round, CK.StrokeJoin.Bevel][value] ?? CK.StrokeJoin.Miter;
}

export function mapLineCap(CK: CanvasKit, value: number): CanvasKitEnum {
  return mapStrokeCap(CK, value);
}

export function mapLineJoin(CK: CanvasKit, value: number): CanvasKitEnum {
  return mapStrokeJoin(CK, value);
}

export function mapTileMode(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.TileMode.Clamp, CK.TileMode.Repeat, CK.TileMode.Mirror, CK.TileMode.Decal][value] ?? CK.TileMode.Clamp;
}

export function mapBlurStyle(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.BlurStyle.Normal, CK.BlurStyle.Inner, CK.BlurStyle.Solid, CK.BlurStyle.Outer][value] ?? CK.BlurStyle.Normal;
}

export function mapFillType(CK: CanvasKit, value: number): CanvasKitEnum {
  return value === 1 ? CK.FillType.EvenOdd : CK.FillType.Winding;
}

export function mapPointMode(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.PointMode.Points, CK.PointMode.Lines, CK.PointMode.Polygon][value] ?? CK.PointMode.Points;
}

