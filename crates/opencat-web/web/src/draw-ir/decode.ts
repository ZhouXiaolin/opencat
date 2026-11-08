/// OCIR DrawOp 帧解码面：唯一读字节的地方。wire 布局常量
/// （OPCODE_PAYLOAD_LEN）全部来自构建期生成物——core 侧 `OpField` 表是
/// 权威源，字段序漂移在解码期 fail-loud，解释层永远消费已解码的
/// `DecodedFrame`。

import { getBlobBytes } from '../wasm';
import { getCachedVideoFrameRgba, getCachedVideoFrameSource } from '../media/video-frame-injector';
import {
  SECTION,
  OP,
  IR_MAGIC,
  IR_VERSION,
  IR_COMPRESSED_MAGIC,
  IR_COMPRESSED_VERSION,
  CODEC_DEFLATE_RAW,
  CONTAINER_HEADER_LEN,
  OPCODE_PAYLOAD_LEN,
  PATH_OP_F32_WIDTHS,
} from '../generated/ocir-schema.generated';
import type { Canvas, Image } from 'canvaskit-wasm';

export type EncodedDrawFrame = Uint8Array;

// Wire-protocol section and opcode constants — imported from the canonical
// Rust-generated schema so drift between core and the TypeScript decoder is
// caught by the schema drift test.  Do not hardcode values here.
const SECTION_OPS = SECTION.OPS;
const SECTION_F32_POOL = SECTION.F32_POOL;
const SECTION_BYTES = SECTION.BYTES;
const SECTION_BYTE_RANGES = SECTION.BYTE_RANGES;
const SECTION_STRINGS_UTF8 = SECTION.STRINGS_UTF8;
const SECTION_STRING_RANGES = SECTION.STRING_RANGES;
const SECTION_PAINTS = SECTION.PAINTS;
const SECTION_PATHS = SECTION.PATHS;
const SECTION_CHILDREN = SECTION.CHILDREN;
const SECTION_EFFECTS = SECTION.EFFECTS;
const SECTION_SUBTREES = SECTION.SUBTREES;
const SECTION_GENERATED_IMAGES = SECTION.GENERATED_IMAGES;

export const OP_SAVE = OP.SAVE;
export const OP_SAVE_LAYER = OP.SAVE_LAYER;
export const OP_SAVE_BACKDROP_LAYER = OP.SAVE_BACKDROP_LAYER;
export const OP_RESTORE = OP.RESTORE;
export const OP_RESTORE_TO_COUNT = OP.RESTORE_TO_COUNT;
export const OP_TRANSLATE = OP.TRANSLATE;
export const OP_SCALE = OP.SCALE;
export const OP_ROTATE = OP.ROTATE;
export const OP_SKEW = OP.SKEW;
export const OP_CONCAT = OP.CONCAT;
export const OP_SET_FILL_STYLE = OP.SET_FILL_STYLE;
export const OP_SET_STROKE_STYLE = OP.SET_STROKE_STYLE;
export const OP_SET_LINE_WIDTH = OP.SET_LINE_WIDTH;
export const OP_SET_LINE_CAP = OP.SET_LINE_CAP;
export const OP_SET_LINE_JOIN = OP.SET_LINE_JOIN;
export const OP_SET_LINE_DASH = OP.SET_LINE_DASH;
export const OP_CLEAR_LINE_DASH = OP.CLEAR_LINE_DASH;
export const OP_SET_GLOBAL_ALPHA = OP.SET_GLOBAL_ALPHA;
export const OP_SET_ANTI_ALIAS = OP.SET_ANTI_ALIAS;
export const OP_SET_BLEND_MODE = OP.SET_BLEND_MODE;
export const OP_BEGIN_PATH = OP.BEGIN_PATH;
export const OP_FILL_PATH = OP.FILL_PATH;
export const OP_STROKE_PATH = OP.STROKE_PATH;
export const OP_CLIP_PATH = OP.CLIP_PATH;
export const OP_CLEAR = OP.CLEAR;
export const OP_PAINT = OP.PAINT;
export const OP_RECT = OP.RECT;
export const OP_R_RECT = OP.R_RECT;
export const OP_D_RRECT = OP.D_RRECT;
export const OP_OVAL = OP.OVAL;
export const OP_CIRCLE = OP.CIRCLE;
export const OP_ARC = OP.ARC;
export const OP_LINE = OP.LINE;
export const OP_POINTS = OP.POINTS;
export const OP_DRAW_PATH = OP.DRAW_PATH;
export const OP_IMAGE = OP.IMAGE;
export const OP_IMAGE_RECT = OP.IMAGE_RECT;
export const OP_RUNTIME_EFFECT = OP.RUNTIME_EFFECT;
export const OP_REPLAY_RANGE = OP.REPLAY_RANGE;
export const OP_DRAW_SUBTREE_PICTURE = OP.DRAW_SUBTREE_PICTURE;
export const OP_FILL_PATTERN = OP.FILL_PATTERN;
export const OP_STROKE_PATTERN = OP.STROKE_PATTERN;
export const OP_LOTTIE_RECT = OP.LOTTIE_RECT;
// v6: each PathOp command is promoted to its own top-level opcode (the old
// OP_PATH / PATH_OP sub-opcode form is gone). One op = one path command.
export const OP_PATH_MOVE_TO = OP.PATH_MOVE_TO;
export const OP_PATH_LINE_TO = OP.PATH_LINE_TO;
export const OP_PATH_QUAD_TO = OP.PATH_QUAD_TO;
export const OP_PATH_CUBIC_TO = OP.PATH_CUBIC_TO;
export const OP_PATH_CLOSE = OP.PATH_CLOSE;
export const OP_PATH_ADD_RECT = OP.PATH_ADD_RECT;
export const OP_PATH_ADD_RRECT = OP.PATH_ADD_RRECT;
export const OP_PATH_ADD_OVAL = OP.PATH_ADD_OVAL;
export const OP_PATH_ADD_ARC = OP.PATH_ADD_ARC;
/// Lowest v6 path opcode (records the contiguous path-command range).
export const OP_PATH_FIRST = OP_PATH_MOVE_TO;
/// Highest v6 path opcode.
export const OP_PATH_LAST = OP_PATH_ADD_ARC;

export const NO_PAINT = 0xffff_ffff;

export type Rect4 = { x: number; y: number; width: number; height: number };
export type Range = { start: number; len: number };
export type DecodedImageRef =
  | { type: 'static'; assetId: string }
  | { type: 'video'; assetId: string; timeMicros: bigint }
  | { type: 'generated'; id: bigint }
  // svg <image>: encoded bytes live in the frame byte table; the host
  // decodes (core stays a pure protocol layer).
  | { type: 'encoded'; byteId: number };

export type FillSpec =
  | { type: 'solid'; color: [number, number, number, number] }
  | { type: 'linearGradient'; tileMode: number; interpolatePremul: number; from: [number, number]; to: [number, number]; stops: number[]; colors: number[][]; localMatrix: number[] | null }
  | { type: 'radialGradient'; tileMode: number; interpolatePremul: number; center: [number, number]; radius: number; stops: number[]; colors: number[][]; localMatrix: number[] | null; focal: [number, number, number] | null }
  | { type: 'sweepGradient'; tileMode: number; interpolatePremul: number; center: [number, number]; startAngle: number; endAngle: number; stops: number[]; colors: number[][] }
  // SVG feTurbulence fractal-noise (bg-[fe-turbulence(...)]). `tile` mirrors the
  // engine: Some([w, h]) bakes one tileable tile repeated from `origin` (CSS
  // background semantics); null renders nothing (the engine skips it too).
  | { type: 'turbulence'; baseFrequency: [number, number]; numOctaves: number; seed: number; tile: [number, number] | null; origin: [number, number] };
export type GradientFillSpec = Exclude<FillSpec, { type: 'solid' }>;

export type PaintSpec = {
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

export type ImageFilterSpec =
  | { type: 'blur'; sigmaX: number; sigmaY: number; decal: boolean }
  | { type: 'dropShadow'; dx: number; dy: number; sigmaX: number; sigmaY: number; color: number[]; keepContent: boolean }
  | { type: 'colorFilter'; filter: ColorFilterSpec }
  | { type: 'compose'; outer: ImageFilterSpec; inner: ImageFilterSpec }
  // SVG `<filter>` primitive 链（P4）。线格式是 chain 列表（chainCount 前
  // 导）；web 侧透传字节流（字段不解构成 CanvasKit filter），保证 reader
  // 对齐即可。
  | { type: 'chain'; chains: { output: number; primitives: unknown[] }[] };

export type ColorFilterSpec =
  | { type: 'matrix'; matrix: number[] }
  | { type: 'blendColor'; color: number[]; mode: number }
  | { type: 'linearToSrgbGamma' }
  | { type: 'srgbToLinearGamma' };

export type MaskFilterSpec = { type: 'blur'; sigma: number; style: number; respectCtm: boolean };
export type PathEffectSpec = { type: 'dash'; intervals: number[]; phase: number };
export type PathSpec = { fillType: number; ops: PathCommand[] };
export type PathCommand = { kind: number; values: number[] };

export type ChildRef =
  | { type: 'image'; image: DecodedImageRef }
  | { type: 'picture'; range: Range }
  | { type: 'subtreePicture'; subtree: number }
  | { type: 'shader'; shader: ShaderSpec };

export type ShaderSpec =
  | { type: 'linearGradient'; start: [number, number]; end: [number, number]; stops: number[]; colors: number[][] }
  | { type: 'radialGradient'; center: [number, number]; radius: number; stops: number[]; colors: number[][] };

export type RuntimeEffectSpec = { hash: bigint; sksl: string };
export type OpEntry = { opcode: number; payloadOffset: number; payloadLen: number };
export type ExecuteRangeOnCanvas = (targetCanvas: Canvas, start: number, len: number) => void;
export type ExecuteSubtreeOnCanvas = (targetCanvas: Canvas, subtree: number) => void;
export type DecodedFrame = {
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

export class BinaryReader {
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

// Core-generated color-emoji glyphs (issue #10). In v5+ (issue #45) the OCIR is
// self-contained — no epoch/delta — so the cache is keyed purely by the glyph id.
// Full RGBA arrives every frame in section 12. The CanvasKit Image is built
// lazily from the RGBA on first use and reused across frames.
export const generatedImageCache = new Map<string, Image>();
// (The old backdrop two-pass pre-rasterized each layer's op prefix into a
// scratch surface. That scratch could never draw the frame's video images:
// `makeImageFromTextureSource` binds them to the main surface's GL context —
// a CPU raster scratch (or a makeSurface peer, which gets its own context)
// silently samples recycled texture content instead, observed as a previous
// video frame bleeding into every backdrop after a video cut. The snapshot is
// now taken from the main surface itself at the backdrop op — the video image
// is on its home surface, so the content is always the frame's own.)
// Churning ids (animated Scene3 placeholders mint a new content-addressed id
// every frame) must not retain one image per rendered frame. FIFO cap: far
// above any single frame's distinct generated-image count, so a live image is
// never evicted mid-frame, while stable ids (glyphs, static scenes) stay hot.
export const GENERATED_IMAGE_CACHE_CAP = 32;

export const generatedImageRgba = new Map<string, { width: number; height: number; rgba: Uint8Array }>();

/**
 * Register RGBA for a generated-image id from outside the OCIR envelope —
 * the GPU-stream executor renders the frame's streams and lands its pixels
 * here. resolveImage('generated') builds the CanvasKit Image lazily from
 * this slot at the placeholder's z-position (core never ships stream
 * placeholders' pixels in section 12).
 */

// Media validation — typed errors for atomic fail-fast before CanvasKit replay
// ---------------------------------------------------------------------------

/**
 * Error thrown when validation detects a missing or failed media entry before
 * replay starts. Each instance carries the asset type and identifier so the
 * host can distinguish failure modes without string matching.
 */
export class MediaValidationError extends Error {
  constructor(
    public readonly mediaType: 'image' | 'video' | 'lottie' | 'generatedImage' | 'runtimeEffect',
    public readonly assetId: string,
    message: string,
  ) {
    super(message);
    this.name = 'MediaValidationError';
  }
}

/** Walk a decoded frame and check every referenced media entry is available. */
export function validateFrameMedia(frame: DecodedFrame): void {
  // Check every op for media references.
  const entries = parseOps(frame.ops);

  const checkImage = (image: DecodedImageRef) => {
    if (image.type === 'static') {
      const bytes = getBlobBytes(image.assetId);
      if (!bytes || bytes.length === 0) {
        throw new MediaValidationError(
          'image',
          image.assetId,
          `Missing image data for assetId="${image.assetId}"`,
        );
      }
    } else if (image.type === 'video') {
      const cached = getCachedVideoFrameRgba(image.assetId, image.timeMicros);
      const source = getCachedVideoFrameSource(image.assetId, image.timeMicros);
      if (!cached && !source) {
        throw new MediaValidationError(
          'video',
          `${image.assetId}@${image.timeMicros}`,
          `Missing video frame for assetId="${image.assetId}" at time ${image.timeMicros}`,
        );
      }
    }
    // Generated images are validated by section 12 parsing.
  };

  const checkLottie = (bundleId: string) => {
    const bytes = getBlobBytes(bundleId);
    if (!bytes || bytes.length === 0) {
      throw new MediaValidationError(
        'lottie',
        bundleId,
        `Missing Lottie bundle data for bundleId="${bundleId}"`,
      );
    }
  };

  const checkRuntimeEffect = (hash: bigint, sksl: string) => {
    if (!sksl || sksl.length === 0) {
      throw new MediaValidationError(
        'runtimeEffect',
        `0x${hash.toString(16)}`,
        `Empty sksl for runtime effect ${hash}`,
      );
    }
  };

  const payload = new Payload(frame.ops, 0, frame.ops.byteLength);
  for (const entry of entries) {
    switch (entry.opcode) {
      case OP_IMAGE: {
        const p = new Payload(frame.ops, entry.payloadOffset, entry.payloadLen);
        const image = readImageRef(p, frame);
        checkImage(image);
        break;
      }
      case OP_IMAGE_RECT: {
        const p = new Payload(frame.ops, entry.payloadOffset, entry.payloadLen);
        const image = readImageRef(p, frame);
        checkImage(image);
        break;
      }
      case OP_LOTTIE_RECT: {
        const p = new Payload(frame.ops, entry.payloadOffset, entry.payloadLen);
        const bundleId = frame.strings[p.u32()] ?? '';
        checkLottie(bundleId);
        break;
      }
      case OP_RUNTIME_EFFECT: {
        const p = new Payload(frame.ops, entry.payloadOffset, entry.payloadLen);
        const effectId = p.u32();
        const spec = frame.effects[effectId];
        if (spec) checkRuntimeEffect(spec.hash, spec.sksl);
        break;
      }
    }
  }

  // Validate Lottie bundles and images referenced in subtrees.
  for (let si = 0; si < frame.subtrees.length; si++) {
    const subOps = parseOps(frame.subtrees[si]);
    for (const entry of subOps) {
      const payload = new Payload(frame.subtrees[si], entry.payloadOffset, entry.payloadLen);
      switch (entry.opcode) {
        case OP_IMAGE:
        case OP_IMAGE_RECT: {
          const image = readImageRef(payload, frame);
          checkImage(image);
          break;
        }
        case OP_LOTTIE_RECT: {
          const bundleId = frame.strings[payload.u32()] ?? '';
          checkLottie(bundleId);
          break;
        }
      }
    }
  }

  // Validate runtime effect children (ImageRefs referenced in children table).
  for (let ci = 0; ci < frame.children.length; ci++) {
    const child = frame.children[ci];
    if (child.type === 'image') checkImage(child.image);
  }
}

export class Payload {
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

export function decodeFrame(bytes: Uint8Array): DecodedFrame {
  if (bytes.byteLength < 12) {
    throw new Error(`Truncated OpenCat IR envelope: header needs 12 bytes, got ${bytes.byteLength}`);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (
    bytes[0] !== IR_MAGIC[0] ||
    bytes[1] !== IR_MAGIC[1] ||
    bytes[2] !== IR_MAGIC[2] ||
    bytes[3] !== IR_MAGIC[3]
  ) {
    throw new Error('Invalid OpenCat IR magic');
  }
  const version = view.getUint32(4, true);
  if (version !== IR_VERSION) throw new Error(`Unsupported OpenCat IR version ${version}`);

  // v5+ (issue #45): header is 12 bytes (magic + version + section_count).
  // No pipeline_epoch — OCIR is self-contained. A fresh decoder can
  // independently decode any single frame without epoch/delta/history state.
  const sectionCount = view.getUint32(8, true);
  const dirEnd = 12 + sectionCount * 12;
  if (bytes.byteLength < dirEnd) {
    throw new Error(
      `Truncated OpenCat IR envelope: section directory needs ${dirEnd} bytes, got ${bytes.byteLength}`,
    );
  }
  const sections = new Map<number, Uint8Array>();
  for (let i = 0; i < sectionCount; i++) {
    const base = 12 + i * 12;
    const id = view.getUint32(base, true);
    const offset = view.getUint32(base + 4, true);
    const len = view.getUint32(base + 8, true);
    if (offset > bytes.byteLength || len > bytes.byteLength - offset) {
      throw new Error(
        `Illegal OpenCat IR section ${id} range: offset=${offset} len=${len} envelope=${bytes.byteLength}`,
      );
    }
    sections.set(id, bytes.subarray(offset, offset + len));
  }

  const stringsUtf8 = requireSection(sections, SECTION_STRINGS_UTF8);
  const stringRanges = parseRanges(requireSection(sections, SECTION_STRING_RANGES));
  for (const range of stringRanges) {
    if (range.start > stringsUtf8.byteLength || range.len > stringsUtf8.byteLength - range.start) {
      throw new Error(
        `Illegal OpenCat IR string range: start=${range.start} len=${range.len} utf8=${stringsUtf8.byteLength}`,
      );
    }
  }
  const decoder = new TextDecoder();
  const strings = stringRanges.map((range) => decoder.decode(stringsUtf8.subarray(range.start, range.start + range.len)));

  // Section 12 carries the full generated-image RGBA every frame (issue #45).
  // No epoch/delta/history dependency — self-contained. The CanvasKit Image for
  // each glyph is built lazily inside resolveImage and cached by id.
  parseGeneratedImages(requireSection(sections, SECTION_GENERATED_IMAGES));

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

/// Decode section 12 and register each glyph's RGBA keyed by id. The CanvasKit
/// Image is built on first use by resolveImage. Since v5+ is self-contained
/// (no epoch), a glyph whose id already has an entry is replaced transparently
/// — the same id always carries the same RGBA from the same RenderFrame.
export function parseGeneratedImages(section: Uint8Array): void {
  const reader = new BinaryReader(section);
  const count = reader.u32();
  for (let i = 0; i < count; i++) {
    const id = reader.u64();
    const width = reader.u32();
    const height = reader.u32();
    const rgba = reader.bytesWithLen();
    const key = `${id}`;
    if (!generatedImageCache.has(key)) {
      generatedImageRgba.set(key, { width, height, rgba });
    }
  }
}

export function requireSection(sections: Map<number, Uint8Array>, id: number): Uint8Array {
  const section = sections.get(id);
  if (!section) throw new Error(`Missing OpenCat IR section ${id}`);
  return section;
}

export function parseOps(ops: Uint8Array): OpEntry[] {
  const entries: OpEntry[] = [];
  let offset = 0;
  while (offset < ops.byteLength) {
    // v6 dense stream: [opcode: u8][fixed-length payload], no length field and
    // no padding. Advance by 1 + OPCODE_PAYLOAD_LEN[opcode].
    const opcode = ops[offset]!;
    const payloadLen = OPCODE_PAYLOAD_LEN[opcode];
    if (payloadLen === undefined) {
      throw new Error(`Unknown OpenCat IR opcode ${opcode} at offset ${offset}`);
    }
    const payloadOffset = offset + 1;
    entries.push({ opcode, payloadOffset, payloadLen });
    offset = payloadOffset + payloadLen;
  }
  return entries;
}

export function parseF32Pool(bytes: Uint8Array): number[] {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out: number[] = [];
  for (let offset = 0; offset + 4 <= bytes.byteLength; offset += 4) {
    out.push(view.getFloat32(offset, true));
  }
  return out;
}

export function parseRanges(bytes: Uint8Array): Range[] {
  const reader = new BinaryReader(bytes);
  const ranges: Range[] = [];
  for (let offset = 0; offset + 8 <= bytes.byteLength; offset += 8) {
    ranges.push({ start: reader.u32(), len: reader.u32() });
  }
  return ranges;
}

export function parseSubtrees(bytes: Uint8Array): Uint8Array[] {
  const reader = new BinaryReader(bytes);
  const count = reader.u32();
  const out: Uint8Array[] = [];
  for (let i = 0; i < count; i++) out.push(reader.bytesWithLen());
  return out;
}

export function parsePaints(bytes: Uint8Array): PaintSpec[] {
  const reader = new BinaryReader(bytes);
  const count = reader.u32();
  const out: PaintSpec[] = [];
  for (let i = 0; i < count; i++) {
    out.push(parsePaint(new BinaryReader(reader.bytesWithLen())));
  }
  return out;
}

export function parsePaint(reader: BinaryReader): PaintSpec {
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

export function parseFill(reader: BinaryReader): FillSpec {
  const kind = reader.u8();
  if (kind === 0) return { type: 'solid', color: reader.f32Array(4) as [number, number, number, number] };
  const shaderKind = reader.u8();
  if (shaderKind === 0) {
    // linearGradient: [tileMode u8][interpolatePremul u8][from f32x2][to f32x2][stops][colors][matrix?]
    const tileMode = reader.u8();
    const interpolatePremul = reader.u8();
    return {
      type: 'linearGradient',
      tileMode,
      interpolatePremul,
      from: reader.f32Array(2) as [number, number],
      to: reader.f32Array(2) as [number, number],
      stops: readF32Vec(reader),
      colors: readColorVec(reader),
      localMatrix: readOptionalMatrix(reader),
    };
  }
  if (shaderKind === 2) {
    // turbulence (draw_encoding.rs): [baseFreq f32x2][octaves u32][seed u32]
    // [tile: u8 + f32x2 | u8=0][origin f32x2] — no tileMode byte.
    return {
      type: 'turbulence',
      baseFrequency: reader.f32Array(2) as [number, number],
      numOctaves: reader.u32(),
      seed: reader.u32(),
      tile: reader.u8() === 1 ? (reader.f32Array(2) as [number, number]) : null,
      origin: reader.f32Array(2) as [number, number],
    };
  }
  if (shaderKind === 3) {
    // sweepGradient (CSS conic-gradient): [tileMode u8][interpolatePremul u8]
    // [center f32x2][startAngle f32][endAngle f32][stops][colors] — angles are
    // already Skia-semantics degrees (0 = +x axis, clockwise); the CSS `from θ`
    // conversion happened at spec-build time.
    const tileMode = reader.u8();
    const interpolatePremul = reader.u8();
    return {
      type: 'sweepGradient',
      tileMode,
      interpolatePremul,
      center: reader.f32Array(2) as [number, number],
      startAngle: reader.f32(),
      endAngle: reader.f32(),
      stops: readF32Vec(reader),
      colors: readColorVec(reader),
    };
  }
  // radialGradient: [tileMode u8][interpolatePremul u8][center f32x2][radius f32][stops][colors][matrix?][focal: u8 + f32x3 | u8=0]
  const tileMode = reader.u8();
  const interpolatePremul = reader.u8();
  return {
    type: 'radialGradient',
    tileMode,
    interpolatePremul,
    center: reader.f32Array(2) as [number, number],
    radius: reader.f32(),
    stops: readF32Vec(reader),
    colors: readColorVec(reader),
    localMatrix: readOptionalMatrix(reader),
    focal: reader.u8() === 1 ? (reader.f32Array(3) as [number, number, number]) : null,
  };
}

/// Read an optional 3x3 row-major matrix: presence byte (1 = Some) + 9xf32.
export function readOptionalMatrix(reader: BinaryReader): number[] | null {
  const present = reader.u8();
  if (present === 0) return null;
  return reader.f32Array(9) as number[];
}

export function parseImageFilter(reader: BinaryReader): ImageFilterSpec {
  const kind = reader.u8();
  if (kind === 0) {
    // Blur: sigma_x, sigma_y, decal flag, optional crop rect — mirrors
    // `encode_image_filter`. `decal` selects CSS filter semantics (transparent
    // past the bounds) vs Clamp, so it must be read even though CanvasKit's
    // MakeBlur has no crop-rect parameter.
    const sigmaX = reader.f32();
    const sigmaY = reader.f32();
    const decal = reader.u8() !== 0;
    const hasCrop = reader.u8() !== 0;
    if (hasCrop) reader.f32Array(4);
    return { type: 'blur', sigmaX, sigmaY, decal };
  }
  if (kind === 1) {
    // DropShadow: dx, dy, sigma_x, sigma_y, color, keep_content — `keep_content`
    // picks MakeDropShadow (shadow + content) vs MakeDropShadowOnly.
    return {
      type: 'dropShadow',
      dx: reader.f32(),
      dy: reader.f32(),
      sigmaX: reader.f32(),
      sigmaY: reader.f32(),
      color: reader.f32Array(4),
      keepContent: reader.u8() !== 0,
    };
  }
  if (kind === 2) return { type: 'colorFilter', filter: parseColorFilter(reader) };
  if (kind === 3) return { type: 'compose', outer: parseImageFilter(reader), inner: parseImageFilter(reader) };
  // kind === 4：chain 列表（`url(#a) url(#b)` 级联）。字段序与 Rust
  // `encode_image_filter` / `encode_filter_chain` 严格对称，逐 primitive
  // 读完（web 渲染暂不消费这些字段）。
  const chainCount = reader.u32();
  const chains: { output: number; primitives: unknown[] }[] = [];
  for (let c = 0; c < chainCount; c++) {
    const hasSourceCrop = reader.u8() !== 0;
    if (hasSourceCrop) reader.f32Array(4);
    const output = reader.u16();
    const count = reader.u32();
    const primitives: unknown[] = [];
    for (let i = 0; i < count; i++) {
      const hasCrop = reader.u8() !== 0;
      const crop = hasCrop ? reader.f32Array(4) : null;
      const srgb = reader.u8() !== 0;
      primitives.push({ crop, srgb, kind: parseFilterPrimKind(reader) });
    }
    chains.push({ output, primitives });
  }
  return { type: 'chain', chains };
}

export function parseFilterInput(reader: BinaryReader): unknown {
  const tag = reader.u8();
  if (tag === 2) return { type: 'prim', index: reader.u16() };
  return { type: tag === 1 ? 'sourceAlpha' : 'sourceGraphic' };
}

export function parseTransferFn(reader: BinaryReader): unknown {
  const tag = reader.u8();
  if (tag === 0) return { type: 'identity' };
  if (tag === 1 || tag === 2) return { type: tag === 1 ? 'table' : 'discrete', values: readF32Vec(reader) };
  if (tag === 3) return { type: 'linear', slope: reader.f32(), intercept: reader.f32() };
  return { type: 'gamma', amplitude: reader.f32(), exponent: reader.f32(), offset: reader.f32() };
}

export function parseLightSpec(reader: BinaryReader): unknown {
  const tag = reader.u8();
  if (tag === 0) return { type: 'distant', azimuth: reader.f32(), elevation: reader.f32() };
  if (tag === 1) return { type: 'point', x: reader.f32(), y: reader.f32(), z: reader.f32() };
  return {
    type: 'spot',
    x: reader.f32(), y: reader.f32(), z: reader.f32(),
    pointsAtX: reader.f32(), pointsAtY: reader.f32(), pointsAtZ: reader.f32(),
    specularExponent: reader.f32(), limitingConeAngle: reader.f32(),
  };
}

export function parseFilterPrimKind(reader: BinaryReader): unknown {
  const tag = reader.u8();
  switch (tag) {
    case 0: return { type: 'blend', mode: reader.u8(), in1: parseFilterInput(reader), in2: parseFilterInput(reader) };
    case 1: return { type: 'colorMatrix', rowMajor: reader.f32Array(20), input: parseFilterInput(reader) };
    case 2: return {
      type: 'componentTransfer',
      fns: [parseTransferFn(reader), parseTransferFn(reader), parseTransferFn(reader), parseTransferFn(reader)],
      input: parseFilterInput(reader),
    };
    case 3: return { type: 'composite', op: reader.u8(), k: reader.f32Array(4), in1: parseFilterInput(reader), in2: parseFilterInput(reader) };
    case 4: {
      const order = [reader.u32(), reader.u32()];
      const kernel = readF32Vec(reader);
      const divisor = reader.f32();
      const bias = reader.f32();
      const target = [reader.u32() | 0, reader.u32() | 0];
      const kernelUnitLength = [reader.f32(), reader.f32()];
      const edge = reader.u8();
      const preserveAlpha = reader.u8() !== 0;
      return { type: 'convolveMatrix', order, kernel, divisor, bias, target, kernelUnitLength, edge, preserveAlpha, input: parseFilterInput(reader) };
    }
    case 5: {
      const surfaceScale = reader.f32();
      const diffuseConstant = reader.f32();
      const lightingColor = reader.f32Array(4);
      const light = parseLightSpec(reader);
      return { type: 'diffuseLighting', surfaceScale, diffuseConstant, lightingColor, light, input: parseFilterInput(reader) };
    }
    case 6: return { type: 'displacementMap', xChannel: reader.u8(), yChannel: reader.u8(), scale: reader.f32(), input: parseFilterInput(reader), in2: parseFilterInput(reader) };
    case 7: {
      const dx = reader.f32(); const dy = reader.f32(); const sigmaX = reader.f32(); const sigmaY = reader.f32();
      const color = reader.f32Array(4);
      return { type: 'dropShadow', dx, dy, sigmaX, sigmaY, color, input: parseFilterInput(reader) };
    }
    case 8: return { type: 'flood', color: reader.f32Array(4) };
    case 9: return { type: 'gaussianBlur', sigmaX: reader.f32(), sigmaY: reader.f32(), input: parseFilterInput(reader) };
    case 10: {
      const inputs = Array.from({ length: reader.u32() }, () => parseFilterInput(reader));
      return { type: 'merge', inputs };
    }
    case 11: return { type: 'morphology', erode: reader.u8() !== 0, radiusX: reader.f32(), radiusY: reader.f32(), input: parseFilterInput(reader) };
    case 12: return { type: 'offset', dx: reader.f32(), dy: reader.f32(), input: parseFilterInput(reader) };
    case 13: {
      const surfaceScale = reader.f32();
      const specularConstant = reader.f32();
      const specularExponent = reader.f32();
      const lightingColor = reader.f32Array(4);
      const light = parseLightSpec(reader);
      return { type: 'specularLighting', surfaceScale, specularConstant, specularExponent, lightingColor, light, input: parseFilterInput(reader) };
    }
    case 14: return { type: 'tile', input: parseFilterInput(reader) };
    case 16: return { type: 'image', subtree: reader.u32() };
    default: return {
      type: 'turbulence',
      fractal: reader.u8() !== 0,
      baseFrequency: reader.f32Array(2),
      numOctaves: reader.u32(),
      seed: reader.f32(),
      stitch: reader.u8() !== 0,
    };
  }
}

export function parseColorFilter(reader: BinaryReader): ColorFilterSpec {
  const kind = reader.u8();
  if (kind === 0) return { type: 'matrix', matrix: reader.f32Array(20) };
  if (kind === 1) return { type: 'blendColor', color: reader.f32Array(4), mode: reader.u8() };
  if (kind === 2) return { type: 'linearToSrgbGamma' };
  return { type: 'srgbToLinearGamma' };
}

export function parseMaskFilter(reader: BinaryReader): MaskFilterSpec {
  const kind = reader.u8();
  if (kind !== 0) throw new Error(`Unsupported mask filter ${kind}`);
  return { type: 'blur', sigma: reader.f32(), style: reader.u8(), respectCtm: reader.u8() !== 0 };
}

export function parsePathEffect(reader: BinaryReader): PathEffectSpec {
  const kind = reader.u8();
  if (kind !== 0) throw new Error(`Unsupported path effect ${kind}`);
  return { type: 'dash', intervals: readF32Vec(reader), phase: reader.f32() };
}

export function parsePaths(bytes: Uint8Array): PathSpec[] {
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
      ops.push({ kind, values: r.f32Array(PATH_OP_F32_WIDTHS[kind] ?? 0) });
    }
    out.push({ fillType, ops });
  }
  return out;
}

export function parseChildren(bytes: Uint8Array, strings: string[]): ChildRef[] {
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

export function parseIrShader(reader: BinaryReader): ShaderSpec {
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

export function parseEffects(bytes: Uint8Array): RuntimeEffectSpec[] {
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

export function readF32Vec(reader: BinaryReader): number[] {
  return reader.f32Array(reader.u32());
}

export function readColorVec(reader: BinaryReader): number[][] {
  const count = reader.u32();
  const colors: number[][] = [];
  for (let i = 0; i < count; i++) colors.push(reader.f32Array(4));
  return colors;
}

export function readIrGradientStops(reader: BinaryReader): { stops: number[]; colors: number[][] } {
  const count = reader.u32();
  const stops: number[] = [];
  const colors: number[][] = [];
  for (let i = 0; i < count; i++) {
    stops.push(reader.f32());
    colors.push(reader.f32Array(4));
  }
  return { stops, colors };
}

export function readImageRef(payload: Payload, frame: DecodedFrame): DecodedImageRef {
  const tag = payload.u8();
  // Generated images carry a numeric id (not an interned asset string) and a
  // reserved u32; the RGBA arrives via section 12. Layout mirrors the
  // Rust encoder: tag(1) + id_u64(8) + reserved(4).
  if (tag === 2) {
    const id = payload.u64();
    payload.u32(); // reserved
    return { type: 'generated', id };
  }
  // Encoded bytes: tag(1) + byte_id_u32(4) + pad to the 12-byte id-slot.
  if (tag === 3) {
    const byteId = payload.u32();
    payload.u64(); // reserved
    return { type: 'encoded', byteId };
  }
  const assetId = frame.strings[payload.u32()] ?? '';
  const timeMicros = payload.u64();
  return tag === 0 ? { type: 'static', assetId } : { type: 'video', assetId, timeMicros };
}

export function readImageRefFromReader(reader: BinaryReader, strings: string[]): DecodedImageRef {
  const tag = reader.u8();
  if (tag === 2) {
    const id = reader.u64();
    reader.u32(); // reserved
    return { type: 'generated', id };
  }
  if (tag === 3) {
    const byteId = reader.u32();
    reader.u64(); // reserved
    return { type: 'encoded', byteId };
  }
  const assetId = strings[reader.u32()] ?? '';
  const timeMicros = reader.u64();
  return tag === 0 ? { type: 'static', assetId } : { type: 'video', assetId, timeMicros };
}

// Every CanvasKit object whose lifetime ends with the frame is registered with
// the current scope's disposer and freed at the end of the op-stream walk. This
// is deterministic cleanup, not GC-dependent: without it, a long render loop
// (playback, or a 495-frame oracle sweep) grows the WASM heap until the tab
// OOM-crashes (~100 frames). Caches that outlive a frame (paths, images,
// effects) are intentionally NOT tracked here.
export function align4(value: number): number {
  return (value + 3) & ~3;
}

/// Debug (oracle eval_js channel): decode an encoded frame and summarize its
/// op stream — opcode names plus the geometry-bearing params. This is how a
/// "layer vanished on web" question gets answered at the IR level: if the ops
/// are present with sane coords, the bug is replay; if absent, the bug is
/// upstream in core's display build.
/// True if `bytes` is a compressed OCIR transport container ("OCZ1"), as opposed
/// to a raw "OCIR" envelope.
export function isEncodedDrawEnvelope(body: ArrayBuffer | Uint8Array): boolean {
  const bytes = body instanceof Uint8Array ? body : new Uint8Array(body);
  return (
    bytes.byteLength >= 4 &&
    bytes[0] === IR_COMPRESSED_MAGIC[0] &&
    bytes[1] === IR_COMPRESSED_MAGIC[1] &&
    bytes[2] === IR_COMPRESSED_MAGIC[2] &&
    bytes[3] === IR_COMPRESSED_MAGIC[3]
  );
}

/// Unwrap a compressed OCIR container to the raw "OCIR" envelope, using the
/// platform raw-deflate decoder (`DecompressionStream('deflate-raw')`). Passing
/// a raw envelope through unchanged keeps callers agnostic to the wire variant.
///
/// The compressed form is the optional bandwidth-saving hop produced by core's
/// `transport::compress_ir_envelope`; the raw form is what the wasm bridge emits
/// in-process. Throws if the container is malformed or the codec is unsupported.
export async function decodeDrawEnvelope(
  body: ArrayBuffer | Uint8Array,
): Promise<Uint8Array> {
  const bytes = body instanceof Uint8Array ? body : new Uint8Array(body);
  if (!isEncodedDrawEnvelope(bytes)) return bytes;

  if (bytes.byteLength < CONTAINER_HEADER_LEN) {
    throw new Error(`Truncated OpenCat IR container: needs ${CONTAINER_HEADER_LEN} bytes, got ${bytes.byteLength}`);
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const version = view.getUint32(4, true);
  if (version !== IR_COMPRESSED_VERSION) {
    throw new Error(`Unsupported OpenCat IR container version ${version}`);
  }
  const codec = bytes[8];
  if (codec !== CODEC_DEFLATE_RAW) {
    throw new Error(`Unsupported OpenCat IR codec ${codec}`);
  }
  const expectedLen = view.getUint32(12, true);
  const payload = bytes.subarray(CONTAINER_HEADER_LEN);

  if (typeof DecompressionStream === 'undefined') {
    throw new Error("DecompressionStream unavailable; cannot decode a compressed OCIR container here");
  }
  const stream = new Blob([payload as BlobPart]).stream().pipeThrough(
    new DecompressionStream('deflate-raw'),
  );
  const raw = new Uint8Array(await new Response(stream).arrayBuffer());
  if (raw.byteLength !== expectedLen) {
    throw new Error(`OpenCat IR container length mismatch: header ${expectedLen}, inflated ${raw.byteLength}`);
  }
  if (raw.byteLength < 4 || !(raw[0] === IR_MAGIC[0] && raw[1] === IR_MAGIC[1] && raw[2] === IR_MAGIC[2] && raw[3] === IR_MAGIC[3])) {
    throw new Error('Inflated OpenCat IR container is not an OCIR envelope');
  }
  return raw;
}
