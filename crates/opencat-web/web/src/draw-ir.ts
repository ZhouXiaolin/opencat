import { getBlobBytes, getSkottieBundleAssets } from './wasm';
import {
  getCachedVideoFrameRgba,
  getCachedVideoFrameSource,
} from './media/video-frame-injector';
import type {
  BlendMode,
  BlurStyle,
  Canvas,
  CanvasKit,
  ClipOp,
  ColorFilter,
  FillType,
  FilterMode,
  Image,
  ImageFilter,
  MaskFilter,
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
  Surface,
  TextureSource,
  TileMode,
} from 'canvaskit-wasm';

type BlendModeName =
  | 'Clear'
  | 'Src'
  | 'Dst'
  | 'SrcOver'
  | 'DstOver'
  | 'SrcIn'
  | 'DstIn'
  | 'SrcOut'
  | 'DstOut'
  | 'SrcATop'
  | 'DstATop'
  | 'Xor'
  | 'Plus'
  | 'Modulate'
  | 'Screen'
  | 'Overlay'
  | 'Darken'
  | 'Lighten'
  | 'ColorDodge'
  | 'ColorBurn'
  | 'HardLight'
  | 'SoftLight'
  | 'Difference'
  | 'Exclusion'
  | 'Multiply'
  | 'Hue'
  | 'Saturation'
  | 'Color'
  | 'Luminosity';

type CanvasKitEnum =
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

export type EncodedDrawFrame = Uint8Array;

const SECTION_OPS = 1;
const SECTION_F32_POOL = 2;
const SECTION_BYTES = 3;
const SECTION_BYTE_RANGES = 4;
const SECTION_STRINGS_UTF8 = 5;
const SECTION_STRING_RANGES = 6;
const SECTION_PAINTS = 7;
const SECTION_PATHS = 8;
const SECTION_CHILDREN = 9;
const SECTION_EFFECTS = 10;
const SECTION_SUBTREES = 11;

const OP_SAVE = 0;
const OP_SAVE_LAYER = 1;
const OP_RESTORE = 2;
const OP_RESTORE_TO_COUNT = 3;
const OP_TRANSLATE = 4;
const OP_SCALE = 5;
const OP_ROTATE = 6;
const OP_SKEW = 7;
const OP_CONCAT = 8;
const OP_SET_FILL_STYLE = 9;
const OP_SET_STROKE_STYLE = 10;
const OP_SET_LINE_WIDTH = 11;
const OP_SET_LINE_CAP = 12;
const OP_SET_LINE_JOIN = 13;
const OP_SET_LINE_DASH = 14;
const OP_CLEAR_LINE_DASH = 15;
const OP_SET_GLOBAL_ALPHA = 16;
const OP_SET_ANTI_ALIAS = 17;
const OP_BEGIN_PATH = 18;
const OP_PATH = 19;
const OP_FILL_PATH = 20;
const OP_STROKE_PATH = 21;
const OP_CLIP_PATH = 22;
const OP_CLEAR = 23;
const OP_PAINT = 24;
const OP_RECT = 25;
const OP_R_RECT = 26;
const OP_D_RRECT = 27;
const OP_OVAL = 28;
const OP_CIRCLE = 29;
const OP_ARC = 30;
const OP_LINE = 31;
const OP_POINTS = 32;
const OP_DRAW_PATH = 33;
const OP_IMAGE = 34;
const OP_IMAGE_RECT = 35;
const OP_RUNTIME_EFFECT = 36;
const OP_REPLAY_RANGE = 37;
const OP_DRAW_SUBTREE_PICTURE = 38;
const OP_LOTTIE_RECT = 39;

const NO_PAINT = 0xffff_ffff;

const lottieCache = new Map<string, ManagedSkottieAnimation>();

type Rect4 = { x: number; y: number; width: number; height: number };
type Range = { start: number; len: number };
type DecodedImageRef = { type: 'static'; assetId: string } | { type: 'video'; assetId: string; timeMicros: bigint };

type FillSpec =
  | { type: 'solid'; color: [number, number, number, number] }
  | { type: 'linearGradient'; tileMode: number; from: [number, number]; to: [number, number]; stops: number[]; colors: number[][]; localMatrix: number[] | null }
  | { type: 'radialGradient'; tileMode: number; center: [number, number]; radius: number; stops: number[]; colors: number[][]; localMatrix: number[] | null };
type GradientFillSpec = Exclude<FillSpec, { type: 'solid' }>;

type PaintSpec = {
  fill: FillSpec;
  style: number;
  antiAlias: boolean;
  blendMode: number;
  stroke?: { width: number; cap: number; join: number; miterLimit: number };
  imageFilter?: ImageFilterSpec;
  colorFilter?: ColorFilterSpec;
  maskFilter?: MaskFilterSpec;
  pathEffect?: PathEffectSpec;
};

type ImageFilterSpec =
  | { type: 'blur'; sigmaX: number; sigmaY: number }
  | { type: 'dropShadow'; dx: number; dy: number; sigmaX: number; sigmaY: number; color: number[] }
  | { type: 'colorFilter'; filter: ColorFilterSpec }
  | { type: 'compose'; outer: ImageFilterSpec; inner: ImageFilterSpec };

type ColorFilterSpec =
  | { type: 'matrix'; matrix: number[] }
  | { type: 'blendColor'; color: number[]; mode: number }
  | { type: 'linearToSrgbGamma' }
  | { type: 'srgbToLinearGamma' };

type MaskFilterSpec = { type: 'blur'; sigma: number; style: number; respectCtm: boolean };
type PathEffectSpec = { type: 'dash'; intervals: number[]; phase: number };
type PathSpec = { fillType: number; ops: PathCommand[] };
type PathCommand = { kind: number; values: number[] };

type ChildRef =
  | { type: 'image'; image: DecodedImageRef }
  | { type: 'picture'; range: Range }
  | { type: 'subtreePicture'; subtree: number }
  | { type: 'shader'; shader: ShaderSpec };

type ShaderSpec =
  | { type: 'linearGradient'; start: [number, number]; end: [number, number]; stops: number[]; colors: number[][] }
  | { type: 'radialGradient'; center: [number, number]; radius: number; stops: number[]; colors: number[][] };

type RuntimeEffectSpec = { hash: bigint; sksl: string };
type OpEntry = { opcode: number; payloadOffset: number; payloadLen: number };
type ExecuteRangeOnCanvas = (targetCanvas: Canvas, start: number, len: number) => void;
type ExecuteSubtreeOnCanvas = (targetCanvas: Canvas, subtree: number) => void;
type RenderEncodedDrawFrameOptions = {
  surface?: Surface;
};

type DecodedFrame = {
  bytes: Uint8Array;
  dataView: DataView;
  ops: Uint8Array;
  subtrees: Uint8Array[];
  f32Pool: number[];
  rawBytes: Uint8Array;
  byteRanges: Range[];
  strings: string[];
  paints: PaintSpec[];
  paths: PathSpec[];
  children: ChildRef[];
  effects: RuntimeEffectSpec[];
};

type RenderState = {
  fillColor: [number, number, number, number];
  strokeColor: [number, number, number, number];
  lineWidth: number;
  lineCap: number;
  lineJoin: number;
  lineDash: { intervals: number[]; phase: number } | null;
  globalAlpha: number;
  antiAlias: boolean;
};

class BinaryReader {
  private offset = 0;
  private view: DataView;

  constructor(private bytes: Uint8Array) {
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  u8(): number {
    return this.bytes[this.offset++] ?? 0;
  }

  u16(): number {
    const value = this.view.getUint16(this.offset, true);
    this.offset += 2;
    return value;
  }

  u32(): number {
    const value = this.view.getUint32(this.offset, true);
    this.offset += 4;
    return value;
  }

  u64(): bigint {
    const value = this.view.getBigUint64(this.offset, true);
    this.offset += 8;
    return value;
  }

  f32(): number {
    const value = this.view.getFloat32(this.offset, true);
    this.offset += 4;
    return value;
  }

  f32Array(count: number): number[] {
    const out: number[] = [];
    for (let i = 0; i < count; i++) out.push(this.f32());
    return out;
  }

  bytesWithLen(): Uint8Array {
    const len = this.u32();
    const start = this.offset;
    this.offset += len;
    return this.bytes.subarray(start, start + len);
  }
}

const staticImageCache = new Map<string, Image>();
const pathCache = new WeakMap<object, Path>();
const effectCache = new Map<bigint, RuntimeEffect>();

function initialRenderState(): RenderState {
  return {
    fillColor: [0, 0, 0, 1],
    strokeColor: [0, 0, 0, 1],
    lineWidth: 1,
    lineCap: 0,
    lineJoin: 0,
    lineDash: null,
    globalAlpha: 1,
    antiAlias: true,
  };
}

export function renderEncodedDrawFrame(
  encoded: EncodedDrawFrame,
  ckCanvas: Canvas,
  CK: CanvasKit,
  options: RenderEncodedDrawFrameOptions = {},
): void {
  const frame = decodeFrame(encoded);
  const entries = parseOps(frame.ops);
  const subtreeEntries = new Map<number, OpEntry[]>();
  const transientImageCache = new Map<string, Image>();

  const resolveFrameImage = (image: DecodedImageRef): Image | null => (
    resolveImage(CK, image, options.surface, transientImageCache)
  );

  const executeOpsOnCanvas = (targetCanvas: Canvas, opBytes: Uint8Array, opEntries: OpEntry[], start: number, len: number) => {
    const state = initialRenderState();
    let currentPathBuilder: PathBuilder | undefined;

    const ensurePathBuilder = () => {
      currentPathBuilder ??= new CK.PathBuilder();
      return currentPathBuilder;
    };

    const snapshotPath = () => {
      currentPathBuilder ??= new CK.PathBuilder();
      return currentPathBuilder.snapshot();
    };

    const executeRange = (rangeStart: number, rangeLen: number) => {
      const end = Math.min(opEntries.length, rangeStart + rangeLen);
      for (let i = rangeStart; i < end; i++) executeOp(opEntries[i]);
    };

    const executeOp = (entry: OpEntry) => {
      const p = new Payload(opBytes, entry.payloadOffset, entry.payloadLen);
      switch (entry.opcode) {
        case OP_SAVE:
          targetCanvas.save();
          break;
        case OP_SAVE_LAYER: {
          const flags = p.u8();
          const bounds = readRect4(p);
          const paintId = p.u32();
          const alpha = p.f32();
          let paint = (flags & 0b10) !== 0 ? buildPaint(CK, frame.paints[paintId], 1) : null;
          if ((flags & 0b10) === 0 && alpha < 1) paint = new CK.Paint();
          if (paint && alpha < 1) paint.setAlphaf(alpha);
          targetCanvas.saveLayer(paint ?? undefined, (flags & 0b01) !== 0 ? ckRect(CK, bounds) : null);
          break;
        }
        case OP_RESTORE:
          targetCanvas.restore();
          break;
        case OP_RESTORE_TO_COUNT:
          targetCanvas.restoreToCount?.(p.u32());
          break;
        case OP_TRANSLATE:
          targetCanvas.translate(p.f32(), p.f32());
          break;
        case OP_SCALE:
          targetCanvas.scale(p.f32(), p.f32());
          break;
        case OP_ROTATE:
          targetCanvas.rotate(p.f32(), p.f32(), p.f32());
          break;
        case OP_SKEW:
          targetCanvas.skew(p.f32(), p.f32());
          break;
        case OP_CONCAT:
          targetCanvas.concat(p.f32Array(9));
          break;
        case OP_SET_FILL_STYLE:
          state.fillColor = colorU32ToF32(p.u32());
          break;
        case OP_SET_STROKE_STYLE:
          state.strokeColor = colorU32ToF32(p.u32());
          break;
        case OP_SET_LINE_WIDTH:
          state.lineWidth = p.f32();
          break;
        case OP_SET_LINE_CAP:
          state.lineCap = p.u32();
          break;
        case OP_SET_LINE_JOIN:
          state.lineJoin = p.u32();
          break;
        case OP_SET_LINE_DASH: {
          const dashStart = p.u32();
          const dashLen = p.u32();
          state.lineDash = {
            intervals: frame.f32Pool.slice(dashStart, dashStart + dashLen),
            phase: p.f32(),
          };
          break;
        }
        case OP_CLEAR_LINE_DASH:
          state.lineDash = null;
          break;
        case OP_SET_GLOBAL_ALPHA:
          state.globalAlpha = p.f32();
          break;
        case OP_SET_ANTI_ALIAS:
          state.antiAlias = p.u8() !== 0;
          break;
        case OP_BEGIN_PATH:
          currentPathBuilder?.delete();
          currentPathBuilder = new CK.PathBuilder();
          break;
        case OP_PATH:
          applyPathPayload(CK, ensurePathBuilder(), p);
          break;
        case OP_FILL_PATH: {
          const path = snapshotPath();
          targetCanvas.drawPath(path, buildScriptPaint(CK, state, 'fill'));
          path.delete?.();
          break;
        }
        case OP_STROKE_PATH: {
          const path = snapshotPath();
          targetCanvas.drawPath(path, buildScriptPaint(CK, state, 'stroke'));
          path.delete?.();
          break;
        }
        case OP_CLIP_PATH: {
          const path = snapshotPath();
          targetCanvas.clipPath(path, CK.ClipOp.Intersect, p.u8() !== 0);
          path.delete?.();
          break;
        }
        case OP_CLEAR:
          targetCanvas.clear(CK.Color4f(p.f32(), p.f32(), p.f32(), p.f32()));
          break;
        case OP_PAINT:
          targetCanvas.drawPaint(buildPaintById(CK, frame, p.u32()));
          break;
        case OP_RECT:
          targetCanvas.drawRect(ckRect(CK, readRect4(p)), buildPaintById(CK, frame, p.u32()));
          break;
        case OP_R_RECT: {
          const rect = readRect4(p);
          const radii = p.f32Array(4);
          targetCanvas.drawRRect(ckRRect(rect, radii), buildPaintById(CK, frame, p.u32()));
          break;
        }
        case OP_D_RRECT: {
          const outer = readDRRect(p);
          const inner = readDRRect(p);
          targetCanvas.drawDRRect(
            ckRRect(outer.rect, outer.radii),
            ckRRect(inner.rect, inner.radii),
            buildPaintById(CK, frame, p.u32()),
          );
          break;
        }
        case OP_OVAL:
          targetCanvas.drawOval(ckRect(CK, readRect4(p)), buildPaintById(CK, frame, p.u32()));
          break;
        case OP_CIRCLE:
          targetCanvas.drawCircle(p.f32(), p.f32(), p.f32(), buildPaintById(CK, frame, p.u32()));
          break;
        case OP_ARC: {
          const rect = readRect4(p);
          const arcStart = p.f32();
          const sweep = p.f32();
          const useCenter = p.u8() !== 0;
          targetCanvas.drawArc(ckRect(CK, rect), arcStart, sweep, useCenter, buildPaintById(CK, frame, p.u32()));
          break;
        }
        case OP_LINE:
          targetCanvas.drawLine(p.f32(), p.f32(), p.f32(), p.f32(), buildPaintById(CK, frame, p.u32()));
          break;
        case OP_POINTS: {
          const mode = p.u32();
          const pointStart = p.u32();
          const pointsLen = p.u32();
          targetCanvas.drawPoints(mapPointMode(CK, mode), frame.f32Pool.slice(pointStart, pointStart + pointsLen), buildPaintById(CK, frame, p.u32()));
          break;
        }
        case OP_DRAW_PATH: {
          const path = buildPathById(CK, frame, p.u32());
          targetCanvas.drawPath(path, buildPaintById(CK, frame, p.u32()));
          break;
        }
        case OP_IMAGE: {
          const image = readImageRef(p, frame);
          const x = p.f32();
          const y = p.f32();
          const paintId = p.u32();
          const ckImage = resolveFrameImage(image);
          if (ckImage) targetCanvas.drawImage(ckImage, x, y, paintId === NO_PAINT ? null : buildPaintById(CK, frame, paintId));
          break;
        }
        case OP_IMAGE_RECT: {
          const image = readImageRef(p, frame);
          const hasSrc = p.u8() !== 0;
          const src = readRect4(p);
          const dst = readRect4(p);
          const paintId = p.u32();
          const ckImage = resolveFrameImage(image);
          if (!ckImage) break;
          const sourceRect = hasSrc ? ckRect(CK, src) : imageBounds(CK, ckImage);
          targetCanvas.drawImageRect(ckImage, sourceRect, ckRect(CK, dst), paintId === NO_PAINT ? new CK.Paint() : buildPaintById(CK, frame, paintId));
          break;
        }
        case OP_LOTTIE_RECT: {
          const bundleId = frame.strings[p.u32()] ?? '';
          const lottieFrame = p.f32();
          const dst = readRect4(p);
          const anim = resolveLottieAnimation(CK, bundleId);
          if (!anim) break;
          anim.seekFrame(lottieFrame, undefined);
          const dstRect = ckRect(CK, dst);
          targetCanvas.save();
          targetCanvas.clipRect(dstRect, CK.ClipOp.Intersect, false);
          anim.render(targetCanvas, dstRect);
          targetCanvas.restore();
          break;
        }
        case OP_RUNTIME_EFFECT:
          drawRuntimeEffect(CK, targetCanvas, frame, p, executeRangeOnCanvas, executeSubtreeOnCanvas, resolveFrameImage);
          break;
        case OP_REPLAY_RANGE:
          executeRange(p.u32(), p.u32());
          break;
        case OP_DRAW_SUBTREE_PICTURE:
          const subtree = p.u32();
          const x = p.f32();
          const y = p.f32();
          targetCanvas.save();
          targetCanvas.translate(x, y);
          executeSubtreeOnCanvas(targetCanvas, subtree);
          targetCanvas.restore();
          break;
        default:
          throw new Error(`Unsupported DrawOp opcode ${entry.opcode}`);
      }
    };

    executeRange(start, len);
    currentPathBuilder?.delete();
  };

  const executeRangeOnCanvas: ExecuteRangeOnCanvas = (targetCanvas, start, len) => {
    executeOpsOnCanvas(targetCanvas, frame.ops, entries, start, len);
  };

  const executeSubtreeOnCanvas: ExecuteSubtreeOnCanvas = (targetCanvas, subtree) => {
    const opBytes = frame.subtrees[subtree];
    if (!opBytes) return;
    let opEntries = subtreeEntries.get(subtree);
    if (!opEntries) {
      opEntries = parseOps(opBytes);
      subtreeEntries.set(subtree, opEntries);
    }
    executeOpsOnCanvas(targetCanvas, opBytes, opEntries, 0, opEntries.length);
  };

  try {
    executeRangeOnCanvas(ckCanvas, 0, entries.length);
  } finally {
    try { options.surface?.flush?.(); } catch { /* ignore CanvasKit cleanup flush failures */ }
    for (const image of transientImageCache.values()) {
      try { image.delete?.(); } catch { /* ignore CanvasKit cleanup failures */ }
    }
    transientImageCache.clear();
  }
}

class Payload {
  private view: DataView;
  private offset: number;
  private end: number;

  constructor(private bytes: Uint8Array, offset: number, len: number) {
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    this.offset = offset;
    this.end = offset + len;
  }

  u8(): number {
    if (this.offset >= this.end) return 0;
    return this.bytes[this.offset++] ?? 0;
  }

  u16(): number {
    const value = this.view.getUint16(this.offset, true);
    this.offset += 2;
    return value;
  }

  u32(): number {
    const value = this.view.getUint32(this.offset, true);
    this.offset += 4;
    return value;
  }

  u64(): bigint {
    const value = this.view.getBigUint64(this.offset, true);
    this.offset += 8;
    return value;
  }

  f32(): number {
    const value = this.view.getFloat32(this.offset, true);
    this.offset += 4;
    return value;
  }

  f32Array(count: number): number[] {
    const out: number[] = [];
    for (let i = 0; i < count; i++) out.push(this.f32());
    return out;
  }
}

function decodeFrame(bytes: Uint8Array): DecodedFrame {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (
    bytes[0] !== 0x4f ||
    bytes[1] !== 0x43 ||
    bytes[2] !== 0x49 ||
    bytes[3] !== 0x52
  ) {
    throw new Error('Invalid OpenCat IR magic');
  }
  const version = view.getUint32(4, true);
  if (version !== 3) throw new Error(`Unsupported OpenCat IR version ${version}`);

  const sectionCount = view.getUint32(8, true);
  const sections = new Map<number, Uint8Array>();
  for (let i = 0; i < sectionCount; i++) {
    const base = 12 + i * 12;
    const id = view.getUint32(base, true);
    const offset = view.getUint32(base + 4, true);
    const len = view.getUint32(base + 8, true);
    sections.set(id, bytes.subarray(offset, offset + len));
  }

  const stringsUtf8 = requireSection(sections, SECTION_STRINGS_UTF8);
  const stringRanges = parseRanges(requireSection(sections, SECTION_STRING_RANGES));
  const decoder = new TextDecoder();
  const strings = stringRanges.map((range) => decoder.decode(stringsUtf8.subarray(range.start, range.start + range.len)));

  return {
    bytes,
    dataView: view,
    ops: requireSection(sections, SECTION_OPS),
    subtrees: parseSubtrees(requireSection(sections, SECTION_SUBTREES)),
    f32Pool: parseF32Pool(requireSection(sections, SECTION_F32_POOL)),
    rawBytes: requireSection(sections, SECTION_BYTES),
    byteRanges: parseRanges(requireSection(sections, SECTION_BYTE_RANGES)),
    strings,
    paints: parsePaints(requireSection(sections, SECTION_PAINTS)),
    paths: parsePaths(requireSection(sections, SECTION_PATHS)),
    children: parseChildren(requireSection(sections, SECTION_CHILDREN), strings),
    effects: parseEffects(requireSection(sections, SECTION_EFFECTS)),
  };
}

function requireSection(sections: Map<number, Uint8Array>, id: number): Uint8Array {
  const section = sections.get(id);
  if (!section) throw new Error(`Missing OpenCat IR section ${id}`);
  return section;
}

function parseOps(ops: Uint8Array): OpEntry[] {
  const view = new DataView(ops.buffer, ops.byteOffset, ops.byteLength);
  const entries: OpEntry[] = [];
  let offset = 0;
  while (offset < ops.byteLength) {
    const opcode = view.getUint16(offset, true);
    const payloadLen = view.getUint32(offset + 4, true);
    const payloadOffset = offset + 8;
    entries.push({ opcode, payloadOffset, payloadLen });
    offset = align4(payloadOffset + payloadLen);
  }
  return entries;
}

function parseF32Pool(bytes: Uint8Array): number[] {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out: number[] = [];
  for (let offset = 0; offset + 4 <= bytes.byteLength; offset += 4) {
    out.push(view.getFloat32(offset, true));
  }
  return out;
}

function parseRanges(bytes: Uint8Array): Range[] {
  const reader = new BinaryReader(bytes);
  const ranges: Range[] = [];
  for (let offset = 0; offset + 8 <= bytes.byteLength; offset += 8) {
    ranges.push({ start: reader.u32(), len: reader.u32() });
  }
  return ranges;
}

function parseSubtrees(bytes: Uint8Array): Uint8Array[] {
  const reader = new BinaryReader(bytes);
  const count = reader.u32();
  const out: Uint8Array[] = [];
  for (let i = 0; i < count; i++) out.push(reader.bytesWithLen());
  return out;
}

function parsePaints(bytes: Uint8Array): PaintSpec[] {
  const reader = new BinaryReader(bytes);
  const count = reader.u32();
  const out: PaintSpec[] = [];
  for (let i = 0; i < count; i++) {
    out.push(parsePaint(new BinaryReader(reader.bytesWithLen())));
  }
  return out;
}

function parsePaint(reader: BinaryReader): PaintSpec {
  const fill = parseFill(reader);
  const style = reader.u8();
  const antiAlias = reader.u8() !== 0;
  const blendMode = reader.u8();
  const hasStroke = reader.u8() !== 0;
  const stroke = hasStroke
    ? {
        width: reader.f32(),
        cap: reader.u8(),
        join: reader.u8(),
        miterLimit: reader.f32(),
      }
    : undefined;
  return {
    fill,
    style,
    antiAlias,
    blendMode,
    stroke,
    imageFilter: readOptional(reader, parseImageFilter),
    colorFilter: readOptional(reader, parseColorFilter),
    maskFilter: readOptional(reader, parseMaskFilter),
    pathEffect: readOptional(reader, parsePathEffect),
  };
}

function parseFill(reader: BinaryReader): FillSpec {
  const kind = reader.u8();
  if (kind === 0) return { type: 'solid', color: reader.f32Array(4) as [number, number, number, number] };
  const shaderKind = reader.u8();
  const tileMode = reader.u8();
  if (shaderKind === 0) {
    return {
      type: 'linearGradient',
      tileMode,
      from: reader.f32Array(2) as [number, number],
      to: reader.f32Array(2) as [number, number],
      stops: readF32Vec(reader),
      colors: readColorVec(reader),
      localMatrix: readOptionalMatrix(reader),
    };
  }
  return {
    type: 'radialGradient',
    tileMode,
    center: reader.f32Array(2) as [number, number],
    radius: reader.f32(),
    stops: readF32Vec(reader),
    colors: readColorVec(reader),
    localMatrix: readOptionalMatrix(reader),
  };
}

/// Read an optional 3×3 row-major matrix: presence byte (1 = Some) + 9×f32.
function readOptionalMatrix(reader: BinaryReader): number[] | null {
  const present = reader.u8();
  if (present === 0) return null;
  return reader.f32Array(9) as number[];
}

function parseImageFilter(reader: BinaryReader): ImageFilterSpec {
  const kind = reader.u8();
  if (kind === 0) {
    const sigmaX = reader.f32();
    const sigmaY = reader.f32();
    const hasCrop = reader.u8() !== 0;
    if (hasCrop) reader.f32Array(4);
    return { type: 'blur', sigmaX, sigmaY };
  }
  if (kind === 1) {
    return {
      type: 'dropShadow',
      dx: reader.f32(),
      dy: reader.f32(),
      sigmaX: reader.f32(),
      sigmaY: reader.f32(),
      color: reader.f32Array(4),
    };
  }
  if (kind === 2) return { type: 'colorFilter', filter: parseColorFilter(reader) };
  return { type: 'compose', outer: parseImageFilter(reader), inner: parseImageFilter(reader) };
}

function parseColorFilter(reader: BinaryReader): ColorFilterSpec {
  const kind = reader.u8();
  if (kind === 0) return { type: 'matrix', matrix: reader.f32Array(20) };
  if (kind === 1) return { type: 'blendColor', color: reader.f32Array(4), mode: reader.u8() };
  if (kind === 2) return { type: 'linearToSrgbGamma' };
  return { type: 'srgbToLinearGamma' };
}

function parseMaskFilter(reader: BinaryReader): MaskFilterSpec {
  const kind = reader.u8();
  if (kind !== 0) throw new Error(`Unsupported mask filter ${kind}`);
  return { type: 'blur', sigma: reader.f32(), style: reader.u8(), respectCtm: reader.u8() !== 0 };
}

function parsePathEffect(reader: BinaryReader): PathEffectSpec {
  const kind = reader.u8();
  if (kind !== 0) throw new Error(`Unsupported path effect ${kind}`);
  return { type: 'dash', intervals: readF32Vec(reader), phase: reader.f32() };
}

function parsePaths(bytes: Uint8Array): PathSpec[] {
  const reader = new BinaryReader(bytes);
  const count = reader.u32();
  const out: PathSpec[] = [];
  for (let i = 0; i < count; i++) {
    const r = new BinaryReader(reader.bytesWithLen());
    const fillType = r.u8();
    const opCount = r.u32();
    const ops: PathCommand[] = [];
    for (let j = 0; j < opCount; j++) {
      const kind = r.u16();
      const widths = [2, 2, 4, 6, 0, 4, 5, 4, 6];
      ops.push({ kind, values: r.f32Array(widths[kind] ?? 0) });
    }
    out.push({ fillType, ops });
  }
  return out;
}

function parseChildren(bytes: Uint8Array, strings: string[]): ChildRef[] {
  const reader = new BinaryReader(bytes);
  const count = reader.u32();
  const out: ChildRef[] = [];
  for (let i = 0; i < count; i++) {
    const r = new BinaryReader(reader.bytesWithLen());
    const kind = r.u8();
    if (kind === 0) out.push({ type: 'image', image: readImageRefFromReader(r, strings) });
    else if (kind === 1) out.push({ type: 'picture', range: { start: r.u32(), len: r.u32() } });
    else if (kind === 3) out.push({ type: 'subtreePicture', subtree: r.u32() });
    else out.push({ type: 'shader', shader: parseIrShader(r) });
  }
  return out;
}

function parseIrShader(reader: BinaryReader): ShaderSpec {
  const kind = reader.u8();
  if (kind === 0) {
    return {
      type: 'linearGradient',
      start: reader.f32Array(2) as [number, number],
      end: reader.f32Array(2) as [number, number],
      ...readIrGradientStops(reader),
    };
  }
  return {
    type: 'radialGradient',
    center: reader.f32Array(2) as [number, number],
    radius: reader.f32(),
    ...readIrGradientStops(reader),
  };
}

function parseEffects(bytes: Uint8Array): RuntimeEffectSpec[] {
  const reader = new BinaryReader(bytes);
  const count = reader.u32();
  const out: RuntimeEffectSpec[] = [];
  const decoder = new TextDecoder();
  for (let i = 0; i < count; i++) {
    out.push({ hash: reader.u64(), sksl: decoder.decode(reader.bytesWithLen()) });
  }
  return out;
}

function readOptional<T>(reader: BinaryReader, decode: (reader: BinaryReader) => T): T | undefined {
  return reader.u8() !== 0 ? decode(reader) : undefined;
}

function readF32Vec(reader: BinaryReader): number[] {
  return reader.f32Array(reader.u32());
}

function readColorVec(reader: BinaryReader): number[][] {
  const count = reader.u32();
  const colors: number[][] = [];
  for (let i = 0; i < count; i++) colors.push(reader.f32Array(4));
  return colors;
}

function readIrGradientStops(reader: BinaryReader): { stops: number[]; colors: number[][] } {
  const count = reader.u32();
  const stops: number[] = [];
  const colors: number[][] = [];
  for (let i = 0; i < count; i++) {
    stops.push(reader.f32());
    colors.push(reader.f32Array(4));
  }
  return { stops, colors };
}

function readImageRef(payload: Payload, frame: DecodedFrame): DecodedImageRef {
  const tag = payload.u8();
  const assetId = frame.strings[payload.u32()] ?? '';
  const timeMicros = payload.u64();
  return tag === 0 ? { type: 'static', assetId } : { type: 'video', assetId, timeMicros };
}

function readImageRefFromReader(reader: BinaryReader, strings: string[]): DecodedImageRef {
  const tag = reader.u8();
  const assetId = strings[reader.u32()] ?? '';
  const timeMicros = reader.u64();
  return tag === 0 ? { type: 'static', assetId } : { type: 'video', assetId, timeMicros };
}

function buildPaintById(CK: CanvasKit, frame: DecodedFrame, paintId: number): Paint {
  return buildPaint(CK, frame.paints[paintId], 1);
}

function buildPaint(CK: CanvasKit, spec: PaintSpec | undefined, alpha: number): Paint {
  const paint = new CK.Paint();
  if (!spec) return paint;
  paint.setAntiAlias(spec.antiAlias);
  paint.setStyle(mapPaintStyle(CK, spec.style));
  paint.setBlendMode(mapBlendMode(CK, spec.blendMode));
  applyFill(CK, paint, spec.fill, alpha);
  if (spec.stroke) {
    paint.setStrokeWidth(spec.stroke.width);
    paint.setStrokeCap(mapStrokeCap(CK, spec.stroke.cap));
    paint.setStrokeJoin(mapStrokeJoin(CK, spec.stroke.join));
    paint.setStrokeMiter(spec.stroke.miterLimit);
  }
  if (spec.imageFilter) paint.setImageFilter(buildImageFilter(CK, spec.imageFilter));
  if (spec.colorFilter) paint.setColorFilter(buildColorFilter(CK, spec.colorFilter));
  if (spec.maskFilter) paint.setMaskFilter(buildMaskFilter(CK, spec.maskFilter));
  if (spec.pathEffect) paint.setPathEffect(buildPathEffect(CK, spec.pathEffect));
  return paint;
}

function buildScriptPaint(CK: CanvasKit, state: RenderState, style: 'fill' | 'stroke'): Paint {
  const paint = new CK.Paint();
  const color = [...(style === 'fill' ? state.fillColor : state.strokeColor)] as [number, number, number, number];
  color[3] *= state.globalAlpha;
  paint.setColor(CK.Color4f(color[0], color[1], color[2], color[3]));
  paint.setAntiAlias(state.antiAlias);
  paint.setStyle(style === 'fill' ? CK.PaintStyle.Fill : CK.PaintStyle.Stroke);
  if (style === 'stroke') {
    paint.setStrokeWidth(state.lineWidth);
    paint.setStrokeCap(mapLineCap(CK, state.lineCap));
    paint.setStrokeJoin(mapLineJoin(CK, state.lineJoin));
    if (state.lineDash) {
      paint.setPathEffect(CK.PathEffect.MakeDash(state.lineDash.intervals, state.lineDash.phase));
    }
  }
  return paint;
}

function applyFill(CK: CanvasKit, paint: Paint, fill: FillSpec, alpha: number): void {
  if (fill.type === 'solid') {
    paint.setColor(CK.Color4f(fill.color[0], fill.color[1], fill.color[2], fill.color[3] * alpha));
    paint.setShader(null);
    return;
  }
  const shader = buildShader(CK, fill);
  paint.setShader(shader);
}

function buildShader(CK: CanvasKit, fill: GradientFillSpec | ShaderSpec): Shader {
  // 仅 GradientFillSpec（paint fill）携带 localMatrix；ShaderSpec（runtime effect child）不携带。
  const localMatrix = 'localMatrix' in fill && fill.localMatrix ? fill.localMatrix : undefined;
  if (fill.type === 'linearGradient') {
    return CK.Shader.MakeLinearGradient(
      'from' in fill ? fill.from : fill.start,
      'to' in fill ? fill.to : fill.end,
      fill.colors.map((c) => CK.Color4f(c[0], c[1], c[2], c[3])),
      fill.stops,
      mapTileMode(CK, 'tileMode' in fill ? fill.tileMode : 0),
      localMatrix as any,
    );
  }
  return CK.Shader.MakeRadialGradient(
    fill.center,
    fill.radius,
    fill.colors.map((c) => CK.Color4f(c[0], c[1], c[2], c[3])),
    fill.stops,
    mapTileMode(CK, 'tileMode' in fill ? fill.tileMode : 0),
    localMatrix as any,
  );
}

function buildImageFilter(CK: CanvasKit, spec: ImageFilterSpec): ImageFilter | null {
  if (spec.type === 'blur') return CK.ImageFilter.MakeBlur(spec.sigmaX, spec.sigmaY, CK.TileMode.Decal ?? CK.TileMode.Clamp, null);
  if (spec.type === 'dropShadow') {
    return CK.ImageFilter.MakeDropShadow(
      spec.dx,
      spec.dy,
      spec.sigmaX,
      spec.sigmaY,
      CK.Color4f(spec.color[0], spec.color[1], spec.color[2], spec.color[3]),
      null,
    );
  }
  if (spec.type === 'colorFilter') return CK.ImageFilter.MakeColorFilter(buildColorFilter(CK, spec.filter), null);
  return CK.ImageFilter.MakeCompose(buildImageFilter(CK, spec.outer), buildImageFilter(CK, spec.inner));
}

function buildColorFilter(CK: CanvasKit, spec: ColorFilterSpec): ColorFilter {
  if (spec.type === 'matrix') return CK.ColorFilter.MakeMatrix(spec.matrix);
  if (spec.type === 'blendColor') return CK.ColorFilter.MakeBlend(CK.Color4f(spec.color[0], spec.color[1], spec.color[2], spec.color[3]), mapBlendMode(CK, spec.mode));
  if (spec.type === 'linearToSrgbGamma') return CK.ColorFilter.MakeLinearToSRGBGamma();
  return CK.ColorFilter.MakeSRGBToLinearGamma();
}

function buildMaskFilter(CK: CanvasKit, spec: MaskFilterSpec): MaskFilter {
  return CK.MaskFilter.MakeBlur(mapBlurStyle(CK, spec.style), spec.sigma, spec.respectCtm);
}

function buildPathEffect(CK: CanvasKit, spec: PathEffectSpec): PathEffect {
  return CK.PathEffect.MakeDash(spec.intervals, spec.phase);
}

function buildPathById(CK: CanvasKit, frame: DecodedFrame, id: number): Path {
  const spec = frame.paths[id];
  if (!spec) throw new Error(`Missing path ${id}`);
  const cached = pathCache.get(spec);
  if (cached) return cached;
  const builder = new CK.PathBuilder();
  for (const op of spec.ops) applyPathCommand(CK, builder, op);
  const path = builder.snapshot();
  path.setFillType?.(mapFillType(CK, spec.fillType));
  builder.delete?.();
  pathCache.set(spec, path);
  return path;
}

function applyPathPayload(CK: CanvasKit, builder: PathBuilder, payload: Payload): void {
  const kind = payload.u16();
  const width = [2, 2, 4, 6, 0, 4, 5, 4, 6][kind] ?? 0;
  applyPathCommand(CK, builder, { kind, values: payload.f32Array(width) });
}

function applyPathCommand(CK: CanvasKit, builder: PathBuilder, op: PathCommand): void {
  const v = op.values;
  switch (op.kind) {
    case 0:
      builder.moveTo(v[0], v[1]);
      break;
    case 1:
      builder.lineTo(v[0], v[1]);
      break;
    case 2:
      builder.quadTo(v[0], v[1], v[2], v[3]);
      break;
    case 3:
      builder.cubicTo(v[0], v[1], v[2], v[3], v[4], v[5]);
      break;
    case 4:
      builder.close();
      break;
    case 5:
      builder.addRect(CK.XYWHRect(v[0], v[1], v[2], v[3]));
      break;
    case 6:
      builder.addRRect(CK.RRectXY(CK.XYWHRect(v[0], v[1], v[2], v[3]), v[4], v[4]));
      break;
    case 7:
      builder.addOval(CK.XYWHRect(v[0], v[1], v[2], v[3]));
      break;
    case 8:
      builder.addArc(CK.XYWHRect(v[0], v[1], v[2], v[3]), v[4], v[5]);
      break;
    default:
      throw new Error(`Unsupported PathOp ${op.kind}`);
  }
}

function resolveLottieAnimation(CK: CanvasKit, bundleId: string): ManagedSkottieAnimation | null {
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

function resolveImage(
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
    const ckImage = CK.MakeImageFromEncoded(bytes);
    if (ckImage) staticImageCache.set(image.assetId, ckImage);
    return ckImage;
  }

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

function drawRuntimeEffect(
  CK: CanvasKit,
  canvas: Canvas,
  frame: DecodedFrame,
  payload: Payload,
  executeRangeOnCanvas: ExecuteRangeOnCanvas,
  executeSubtreeOnCanvas: ExecuteSubtreeOnCanvas,
  resolveFrameImage: (image: DecodedImageRef) => Image | null,
): void {
  const effectId = payload.u32();
  const uniformRangeId = payload.u32();
  const childStart = payload.u32();
  const childLen = payload.u32();
  const dst = readRect4(payload);
  const spec = frame.effects[effectId];
  if (!spec) return;
  let effect: RuntimeEffect | undefined = effectCache.get(spec.hash);
  if (!effect) {
    const compiled = CK.RuntimeEffect.Make(spec.sksl);
    if (!compiled) return;
    effect = compiled;
    effectCache.set(spec.hash, effect);
  }
  const range = frame.byteRanges[uniformRangeId];
  const bytes = range ? frame.rawBytes.subarray(range.start, range.start + range.len) : new Uint8Array();
  const uniforms = new Float32Array(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
  const children = frame.children.slice(childStart, childStart + childLen)
    .map((child) => buildRuntimeChildShader(CK, frame, child, dst, executeRangeOnCanvas, executeSubtreeOnCanvas, resolveFrameImage))
    .filter((child): child is Shader => child !== null);
  const shader = children.length > 0 ? effect.makeShaderWithChildren(uniforms, children) : effect.makeShader(uniforms);
  const paint = new CK.Paint();
  paint.setShader(shader);
  canvas.drawRect(ckRect(CK, dst), paint);
}

function buildRuntimeChildShader(
  CK: CanvasKit,
  frame: DecodedFrame,
  child: ChildRef,
  dst: Rect4,
  executeRangeOnCanvas: ExecuteRangeOnCanvas,
  executeSubtreeOnCanvas: ExecuteSubtreeOnCanvas,
  resolveFrameImage: (image: DecodedImageRef) => Image | null,
): Shader | null {
  if (child.type === 'shader') return buildShader(CK, child.shader);
  if (child.type === 'image') {
    const img = resolveFrameImage(child.image);
    if (!img?.makeShaderOptions) return null;
    return img.makeShaderOptions(CK.TileMode.Clamp, CK.TileMode.Clamp, CK.FilterMode.Linear, CK.MipmapMode.None);
  }
  const width = Math.max(1, Math.ceil(dst.x + dst.width));
  const height = Math.max(1, Math.ceil(dst.y + dst.height));
  const recorder = new CK.PictureRecorder();
  const canvas = recorder.beginRecording(CK.XYWHRect(0, 0, width, height));
  if (child.type === 'subtreePicture') executeSubtreeOnCanvas(canvas, child.subtree);
  else executeRangeOnCanvas(canvas, child.range.start, child.range.len);
  const picture = recorder.finishRecordingAsPicture();
  recorder.delete();
  return picture?.makeShader?.(CK.TileMode.Clamp, CK.TileMode.Clamp, CK.FilterMode?.Linear) ?? null;
}

function readRect4(payload: Payload): Rect4 {
  return { x: payload.f32(), y: payload.f32(), width: payload.f32(), height: payload.f32() };
}

function readDRRect(payload: Payload): { rect: Rect4; radii: number[] } {
  return { rect: readRect4(payload), radii: payload.f32Array(4) };
}

function ckRect(CK: CanvasKit, rect: Rect4): Rect {
  return CK.XYWHRect ? CK.XYWHRect(rect.x, rect.y, rect.width, rect.height) : Float32Array.of(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height);
}

function ckRRect(rect: Rect4, radii: number[]): Float32Array {
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

function imageBounds(CK: CanvasKit, image: Image): Rect {
  const width = typeof image.width === 'function' ? image.width() : 0;
  const height = typeof image.height === 'function' ? image.height() : 0;
  return CK.XYWHRect(0, 0, width, height);
}

function colorU32ToF32(color: number): [number, number, number, number] {
  return [
    (color & 0xff) / 255,
    ((color >>> 8) & 0xff) / 255,
    ((color >>> 16) & 0xff) / 255,
    ((color >>> 24) & 0xff) / 255,
  ];
}

function mapPaintStyle(CK: CanvasKit, value: number): CanvasKitEnum {
  return value === 1 ? CK.PaintStyle.Stroke : CK.PaintStyle.Fill;
}

function mapBlendMode(CK: CanvasKit, value: number): CanvasKitEnum {
  const names: BlendModeName[] = ['Clear', 'Src', 'Dst', 'SrcOver', 'DstOver', 'SrcIn', 'DstIn', 'SrcOut', 'DstOut', 'SrcATop', 'DstATop', 'Xor', 'Plus', 'Modulate', 'Screen', 'Overlay', 'Darken', 'Lighten', 'ColorDodge', 'ColorBurn', 'HardLight', 'SoftLight', 'Difference', 'Exclusion', 'Multiply', 'Hue', 'Saturation', 'Color', 'Luminosity'];
  return CK.BlendMode[names[value] ?? 'SrcOver'];
}

function mapStrokeCap(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.StrokeCap.Butt, CK.StrokeCap.Round, CK.StrokeCap.Square][value] ?? CK.StrokeCap.Butt;
}

function mapStrokeJoin(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.StrokeJoin.Miter, CK.StrokeJoin.Round, CK.StrokeJoin.Bevel][value] ?? CK.StrokeJoin.Miter;
}

function mapLineCap(CK: CanvasKit, value: number): CanvasKitEnum {
  return mapStrokeCap(CK, value);
}

function mapLineJoin(CK: CanvasKit, value: number): CanvasKitEnum {
  return mapStrokeJoin(CK, value);
}

function mapTileMode(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.TileMode.Clamp, CK.TileMode.Repeat, CK.TileMode.Mirror, CK.TileMode.Decal][value] ?? CK.TileMode.Clamp;
}

function mapBlurStyle(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.BlurStyle.Normal, CK.BlurStyle.Inner, CK.BlurStyle.Solid, CK.BlurStyle.Outer][value] ?? CK.BlurStyle.Normal;
}

function mapFillType(CK: CanvasKit, value: number): CanvasKitEnum {
  return value === 1 ? CK.FillType.EvenOdd : CK.FillType.Winding;
}

function mapPointMode(CK: CanvasKit, value: number): CanvasKitEnum {
  return [CK.PointMode.Points, CK.PointMode.Lines, CK.PointMode.Polygon][value] ?? CK.PointMode.Points;
}

function align4(value: number): number {
  return (value + 3) & ~3;
}
