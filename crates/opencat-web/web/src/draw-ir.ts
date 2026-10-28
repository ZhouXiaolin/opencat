import { getBlobBytes, getSkottieBundleAssets } from './wasm';
import {
  getCachedVideoFrameRgba,
  getCachedVideoFrameSource,
} from './media/video-frame-injector';
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
  PATH_OP_SUB,
  PATH_OP_F32_WIDTHS,
  BLEND_MODE,
} from './generated/ocir-schema.generated';
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

const OP_SAVE = OP.SAVE;
const OP_SAVE_LAYER = OP.SAVE_LAYER;
const OP_SAVE_BACKDROP_LAYER = OP.SAVE_BACKDROP_LAYER;
const OP_RESTORE = OP.RESTORE;
const OP_RESTORE_TO_COUNT = OP.RESTORE_TO_COUNT;
const OP_TRANSLATE = OP.TRANSLATE;
const OP_SCALE = OP.SCALE;
const OP_ROTATE = OP.ROTATE;
const OP_SKEW = OP.SKEW;
const OP_CONCAT = OP.CONCAT;
const OP_SET_FILL_STYLE = OP.SET_FILL_STYLE;
const OP_SET_STROKE_STYLE = OP.SET_STROKE_STYLE;
const OP_SET_LINE_WIDTH = OP.SET_LINE_WIDTH;
const OP_SET_LINE_CAP = OP.SET_LINE_CAP;
const OP_SET_LINE_JOIN = OP.SET_LINE_JOIN;
const OP_SET_LINE_DASH = OP.SET_LINE_DASH;
const OP_CLEAR_LINE_DASH = OP.CLEAR_LINE_DASH;
const OP_SET_GLOBAL_ALPHA = OP.SET_GLOBAL_ALPHA;
const OP_SET_ANTI_ALIAS = OP.SET_ANTI_ALIAS;
const OP_BEGIN_PATH = OP.BEGIN_PATH;
const OP_FILL_PATH = OP.FILL_PATH;
const OP_STROKE_PATH = OP.STROKE_PATH;
const OP_CLIP_PATH = OP.CLIP_PATH;
const OP_CLEAR = OP.CLEAR;
const OP_PAINT = OP.PAINT;
const OP_RECT = OP.RECT;
const OP_R_RECT = OP.R_RECT;
const OP_D_RRECT = OP.D_RRECT;
const OP_OVAL = OP.OVAL;
const OP_CIRCLE = OP.CIRCLE;
const OP_ARC = OP.ARC;
const OP_LINE = OP.LINE;
const OP_POINTS = OP.POINTS;
const OP_DRAW_PATH = OP.DRAW_PATH;
const OP_IMAGE = OP.IMAGE;
const OP_IMAGE_RECT = OP.IMAGE_RECT;
const OP_RUNTIME_EFFECT = OP.RUNTIME_EFFECT;
const OP_REPLAY_RANGE = OP.REPLAY_RANGE;
const OP_DRAW_SUBTREE_PICTURE = OP.DRAW_SUBTREE_PICTURE;
const OP_FILL_PATTERN = OP.FILL_PATTERN;
const OP_STROKE_PATTERN = OP.STROKE_PATTERN;
const OP_LOTTIE_RECT = OP.LOTTIE_RECT;
// v6: each PathOp command is promoted to its own top-level opcode (the old
// OP_PATH / PATH_OP sub-opcode form is gone). One op = one path command.
const OP_PATH_MOVE_TO = OP.PATH_MOVE_TO;
const OP_PATH_LINE_TO = OP.PATH_LINE_TO;
const OP_PATH_QUAD_TO = OP.PATH_QUAD_TO;
const OP_PATH_CUBIC_TO = OP.PATH_CUBIC_TO;
const OP_PATH_CLOSE = OP.PATH_CLOSE;
const OP_PATH_ADD_RECT = OP.PATH_ADD_RECT;
const OP_PATH_ADD_RRECT = OP.PATH_ADD_RRECT;
const OP_PATH_ADD_OVAL = OP.PATH_ADD_OVAL;
const OP_PATH_ADD_ARC = OP.PATH_ADD_ARC;
/// Lowest v6 path opcode (records the contiguous path-command range).
const OP_PATH_FIRST = OP_PATH_MOVE_TO;
/// Highest v6 path opcode.
const OP_PATH_LAST = OP_PATH_ADD_ARC;

const NO_PAINT = 0xffff_ffff;

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

type Rect4 = { x: number; y: number; width: number; height: number };
type Range = { start: number; len: number };
type DecodedImageRef =
  | { type: 'static'; assetId: string }
  | { type: 'video'; assetId: string; timeMicros: bigint }
  | { type: 'generated'; id: bigint }
  // svg <image>: encoded bytes live in the frame byte table; the host
  // decodes (core stays a pure protocol layer).
  | { type: 'encoded'; byteId: number };

type FillSpec =
  | { type: 'solid'; color: [number, number, number, number] }
  | { type: 'linearGradient'; tileMode: number; interpolatePremul: number; from: [number, number]; to: [number, number]; stops: number[]; colors: number[][]; localMatrix: number[] | null }
  | { type: 'radialGradient'; tileMode: number; interpolatePremul: number; center: [number, number]; radius: number; stops: number[]; colors: number[][]; localMatrix: number[] | null; focal: [number, number, number] | null }
  | { type: 'sweepGradient'; tileMode: number; interpolatePremul: number; center: [number, number]; startAngle: number; endAngle: number; stops: number[]; colors: number[][] }
  // SVG feTurbulence fractal-noise (bg-[fe-turbulence(...)]). `tile` mirrors the
  // engine: Some([w, h]) bakes one tileable tile repeated from `origin` (CSS
  // background semantics); null renders nothing (the engine skips it too).
  | { type: 'turbulence'; baseFrequency: [number, number]; numOctaves: number; seed: number; tile: [number, number] | null; origin: [number, number] };
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
  | { type: 'blur'; sigmaX: number; sigmaY: number; decal: boolean }
  | { type: 'dropShadow'; dx: number; dy: number; sigmaX: number; sigmaY: number; color: number[]; keepContent: boolean }
  | { type: 'colorFilter'; filter: ColorFilterSpec }
  | { type: 'compose'; outer: ImageFilterSpec; inner: ImageFilterSpec }
  // SVG `<filter>` primitive 链（P4）。线格式是 chain 列表（chainCount 前
  // 导）；web 侧透传字节流（字段不解构成 CanvasKit filter），保证 reader
  // 对齐即可。
  | { type: 'chain'; chains: { output: number; primitives: unknown[] }[] };

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
const effectCache = new Map<bigint, RuntimeEffect>();
// Core-generated color-emoji glyphs (issue #10). In v5+ (issue #45) the OCIR is
// self-contained — no epoch/delta — so the cache is keyed purely by the glyph id.
// Full RGBA arrives every frame in section 12. The CanvasKit Image is built
// lazily from the RGBA on first use and reused across frames.
const generatedImageCache = new Map<string, Image>();
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
const GENERATED_IMAGE_CACHE_CAP = 32;

function cacheGeneratedImage(key: string, image: Image): void {
  generatedImageCache.set(key, image);
  if (generatedImageCache.size > GENERATED_IMAGE_CACHE_CAP) {
    const oldest = generatedImageCache.keys().next().value;
    if (oldest !== undefined) {
      generatedImageCache.get(oldest)?.delete();
      generatedImageCache.delete(oldest);
    }
  }
}
const generatedImageRgba = new Map<string, { width: number; height: number; rgba: Uint8Array }>();

/**
 * Register RGBA for a generated-image id from outside the OCIR envelope —
 * the GPU-stream executor renders the frame's streams and lands its pixels
 * here. resolveImage('generated') builds the CanvasKit Image lazily from
 * this slot at the placeholder's z-position (core never ships stream
 * placeholders' pixels in section 12).
 */
export function provideGeneratedImageRgba(id: bigint | string, width: number, height: number, rgba: Uint8Array): void {
  generatedImageRgba.set(`${id}`, { width, height, rgba });
}

// ---------------------------------------------------------------------------
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
function validateFrameMedia(frame: DecodedFrame): void {
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
  // Atomic fail-fast: validate all current-frame media exists before any
  // CanvasKit operation. If any image, video frame, Lottie bundle, or runtime
  // effect is missing or failed, a typed MediaValidationError is thrown and no
  // partial drawing is observable.
  validateFrameMedia(frame);
  const entries = parseOps(frame.ops);
  const subtreeEntries = new Map<number, OpEntry[]>();
  const transientImageCache = new Map<string, Image>();

  // Deterministic per-frame disposer. Everything this frame allocates on the
  // CanvasKit/WASM heap lands here and is freed after the surface flush — never
  // left to the GC, which cannot keep up with a per-frame render loop (the
  // cause of the ~100-frame tab crash). Freeing earlier than the flush crashes
  // with `Paint instance already deleted` / `PathBuilder instance already
  // deleted`: CanvasKit resolves the deferred display list at flush time.
  const frameDisposables: Array<{ delete: () => void }> = [];
  // Per-frame path dedupe (id → Path), so repeated DrawPath ops in one frame
  // build each path once without keeping it alive across frames.
  const framePathCache = new Map<number, Path>();
  const dispose: Dispose = (obj) => {
    if (obj && typeof obj.delete === 'function') frameDisposables.push(obj as { delete: () => void });
    return obj;
  };

  const resolveFrameImage = (image: DecodedImageRef): Image | null => (
    resolveImage(CK, image, options.surface, transientImageCache)
  );

  const executeOpsOnCanvas = (targetCanvas: Canvas, opBytes: Uint8Array, opEntries: OpEntry[], start: number, len: number) => {
    const state = initialRenderState();
    let currentPathBuilder: PathBuilder | undefined;

    const ensurePathBuilder = () => {
      currentPathBuilder ??= dispose(new CK.PathBuilder());
      return currentPathBuilder;
    };

    const snapshotPath = () => {
      // The snapshot is handed to the canvas; the builder is freed at walk end
      // (the snapshot owns its own memory).
      currentPathBuilder ??= dispose(new CK.PathBuilder());
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
          let paint = (flags & 0b10) !== 0 ? buildPaint(CK, frame.paints[paintId], 1, dispose) : null;
          if ((flags & 0b10) === 0 && alpha < 1) paint = dispose(new CK.Paint());
          if (paint && alpha < 1) paint.setAlphaf(alpha);
          targetCanvas.saveLayer(paint ?? undefined, (flags & 0b01) !== 0 ? ckRect(CK, bounds) : null);
          break;
        }
        case OP_SAVE_BACKDROP_LAYER: {
          // CSS `backdrop-filter`: same payload as SAVE_LAYER (alpha always
          // 1.0); the paint's image filter applies to what is already drawn
          // beneath the layer bounds, and the layer content stays unfiltered.
          const flags = p.u8();
          const bounds = readRect4(p);
          const paintId = p.u32();
          p.f32(); // alpha slot, always 1.0 — kept for payload-layout parity
          // 4×f32 border-box corner radii (tl, tr, br, bl; 0 = square)
          const radii = [p.f32(), p.f32(), p.f32(), p.f32()];
          const paint = (flags & 0b10) !== 0 ? buildPaint(CK, frame.paints[paintId], 1, dispose) : null;
          let backdrop: ImageFilter | null = null;
          if (paint) {
            const spec = frame.paints[paintId];
            // canvaskit 0.41 has no Paint.getImageFilter getter at runtime
            // (its type defs omit it too) — rebuild the filter from the spec
            // instead of reading it back off the paint.
            if (spec?.imageFilter) {
              backdrop = buildImageFilter(CK, spec.imageFilter, dispose);
              paint.setImageFilter(null);
            }
          }
          if (!backdrop || (flags & 0b01) === 0 || !options.surface) {
            // No filter, no surface to snapshot, or a nesting level where the
            // backdrop never rendered anyway (replay.rs's fallback path).
            targetCanvas.saveLayer(
              paint ?? undefined,
              (flags & 0b01) !== 0 ? ckRect(CK, bounds) : null,
              backdrop ?? undefined,
            );
            break;
          }
          // Two-pass: snapshot what the frame has already drawn (the CSS
          // backdrop — taken on the main surface, where the frame's video
          // images are texture-backed and only drawable), open the layer
          // unbounded (box-shadows draw past it), clip the blurred snapshot to
          // the element rect, then stamp it back through the backdrop filter.
          targetCanvas.saveLayer(paint ?? undefined, null);
          targetCanvas.save();
          if ((flags & 0b01) !== 0) {
            const r = ckRect(CK, bounds);
            if (radii.some((rad) => rad > 0)) {
              // CanvasKit RRect: [l,t,r,b, tlX,tlY, trX,trY, brX,brY, blX,blY]
              targetCanvas.clipRRect(
                [r[0], r[1], r[2], r[3], radii[0], radii[0], radii[1], radii[1], radii[2], radii[2], radii[3], radii[3]],
                CK.ClipOp.Intersect,
                true,
              );
            } else {
              targetCanvas.clipRect(r, CK.ClipOp.Intersect, false);
            }
          }
          // No Canvas.setMatrix in 0.41 — concat the inverse to get back to
          // the identity basis the snapshot was rasterized at.
          const inv = CK.M44.invert(targetCanvas.getTotalMatrix());
          if (inv) targetCanvas.concat(inv);
          const stamp = dispose(new CK.Paint());
          stamp.setImageFilter(backdrop);
          const snap = dispose(options.surface.makeImageSnapshot());
          targetCanvas.drawImage(snap, 0, 0, stamp);
          targetCanvas.restore();
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
        case OP_CONCAT: {
          // Wire layout is column-major (`[h00,h10,h20, h01,h11,h21, h02,
          // h12,h22]`, Mat4::to_homography3); CanvasKit's 9-array concat reads
          // SkMatrix row-major [sx,kx,tx, ky,sy,ty, p0,p1,p2]. Same shuffle
          // replay.rs does with Matrix::new_all — without it the translation
          // lands in the perspective row and W=tx·x collapses the draw.
          const m = p.f32Array(9);
          targetCanvas.concat([m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]]);
          break;
        }
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
          // The previous builder (if any) is already registered with the walk
          // disposer; just start a fresh one.
          currentPathBuilder = dispose(new CK.PathBuilder());
          break;
        case OP_PATH_MOVE_TO:
        case OP_PATH_LINE_TO:
        case OP_PATH_QUAD_TO:
        case OP_PATH_CUBIC_TO:
        case OP_PATH_CLOSE:
        case OP_PATH_ADD_RECT:
        case OP_PATH_ADD_RRECT:
        case OP_PATH_ADD_OVAL:
        case OP_PATH_ADD_ARC:
          applyPathPayload(CK, ensurePathBuilder(), p, entry.opcode - OP_PATH_FIRST);
          break;
        case OP_FILL_PATH: {
          const path = snapshotPath();
          targetCanvas.drawPath(path, dispose(buildScriptPaint(CK, state, 'fill', dispose)));
          dispose(path);
          break;
        }
        case OP_STROKE_PATH: {
          const path = snapshotPath();
          targetCanvas.drawPath(path, dispose(buildScriptPaint(CK, state, 'stroke', dispose)));
          dispose(path);
          break;
        }
        case OP_CLIP_PATH: {
          // wire: [u8 anti_alias][u8 fill_type]（与 Rust 侧 draw_encoding 同序）。
          const antiAlias = p.u8() !== 0;
          const fillType = p.u8();
          const path = snapshotPath();
          path.setFillType?.(mapFillType(CK, fillType));
          targetCanvas.clipPath(path, CK.ClipOp.Intersect, antiAlias);
          dispose(path);
          break;
        }
        case OP_CLEAR:
          targetCanvas.clear(CK.Color4f(p.f32(), p.f32(), p.f32(), p.f32()));
          break;
        case OP_PAINT:
          targetCanvas.drawPaint(buildPaintById(CK, frame, p.u32(), dispose));
          break;
        case OP_RECT:
          targetCanvas.drawRect(ckRect(CK, readRect4(p)), buildPaintById(CK, frame, p.u32(), dispose));
          break;
        case OP_R_RECT: {
          const rect = readRect4(p);
          const radii = p.f32Array(4);
          targetCanvas.drawRRect(ckRRect(rect, radii), buildPaintById(CK, frame, p.u32(), dispose));
          break;
        }
        case OP_D_RRECT: {
          const outer = readDRRect(p);
          const inner = readDRRect(p);
          targetCanvas.drawDRRect(
            ckRRect(outer.rect, outer.radii),
            ckRRect(inner.rect, inner.radii),
            buildPaintById(CK, frame, p.u32(), dispose),
          );
          break;
        }
        case OP_OVAL:
          targetCanvas.drawOval(ckRect(CK, readRect4(p)), buildPaintById(CK, frame, p.u32(), dispose));
          break;
        case OP_CIRCLE:
          targetCanvas.drawCircle(p.f32(), p.f32(), p.f32(), buildPaintById(CK, frame, p.u32(), dispose));
          break;
        case OP_ARC: {
          const rect = readRect4(p);
          const arcStart = p.f32();
          const sweep = p.f32();
          const useCenter = p.u8() !== 0;
          targetCanvas.drawArc(ckRect(CK, rect), arcStart, sweep, useCenter, buildPaintById(CK, frame, p.u32(), dispose));
          break;
        }
        case OP_LINE:
          targetCanvas.drawLine(p.f32(), p.f32(), p.f32(), p.f32(), buildPaintById(CK, frame, p.u32(), dispose));
          break;
        case OP_POINTS: {
          const mode = p.u32();
          const pointStart = p.u32();
          const pointsLen = p.u32();
          targetCanvas.drawPoints(mapPointMode(CK, mode), frame.f32Pool.slice(pointStart, pointStart + pointsLen), buildPaintById(CK, frame, p.u32(), dispose));
          break;
        }
        case OP_DRAW_PATH: {
          // framePathCache dedupes within the frame; the path (and the builder
          // that snapshotted it) are freed after the surface flush.
          const path = buildPathById(CK, frame, p.u32(), framePathCache, dispose);
          targetCanvas.drawPath(path, buildPaintById(CK, frame, p.u32(), dispose));
          break;
        }
        case OP_FILL_PATTERN: {
          // svg `<pattern>`: tile picture recording repeated via a picture
          // shader. The tile pixel scale mirrors the engine: (ctm x pattern
          // transform)'s scale part drives the recording resolution; the
          // shader local matrix maps user space onto the tile.
          const pathId = p.u32();
          const subtree = p.u32();
          const tile = readRect4(p);
          const pt = [p.f32(), p.f32(), p.f32(), p.f32(), p.f32(), p.f32()];
          const opacity = p.f32();
          const ctm = targetCanvas.getTotalMatrix();
          const ptM = Float32Array.of(pt[0], pt[1], pt[2], pt[3], pt[4], pt[5], 0, 0, 1);
          const m = CK.Matrix.multiply(ctm as unknown as number[], ptM);
          const sx = Math.sqrt(m[0] * m[0] + m[3] * m[3]);
          const sy = Math.sqrt(m[1] * m[1] + m[4] * m[4]);
          if (!(sx > 0) || !(sy > 0) || tile.width <= 0 || tile.height <= 0 || opacity <= 0) break;
          const tileW = Math.max(1, Math.round(tile.width * sx));
          const tileH = Math.max(1, Math.round(tile.height * sy));
          const recorder = new CK.PictureRecorder();
          const tileCanvas = recorder.beginRecording(CK.XYWHRect(0, 0, tileW, tileH));
          tileCanvas.scale(sx, sy);
          executeSubtreeOnCanvas(tileCanvas, subtree);
          const tilePicture = recorder.finishRecordingAsPicture();
          recorder.delete();
          const localMatrix = CK.Matrix.multiply(
            CK.Matrix.multiply(ptM, CK.Matrix.translated(tile.x, tile.y)),
            CK.Matrix.scaled(1 / sx, 1 / sy),
          );
          const shader = tilePicture.makeShader?.(
            CK.TileMode.Repeat,
            CK.TileMode.Repeat,
            CK.FilterMode.Linear,
            /* tileRect */ undefined,
            /* localMatrix */ localMatrix,
          );
          if (shader) {
            const paint = dispose(new CK.Paint());
            paint.setAlphaf(opacity);
            paint.setShader(dispose(shader));
            targetCanvas.drawPath(buildPathById(CK, frame, pathId, framePathCache, dispose), paint);
          }
          tilePicture.delete();
          break;
        }
        case OP_STROKE_PATTERN: {
          // svg `<pattern>` stroke: same tile layout + inline stroke params
          // ([f32 width][u8 cap][u8 join][f32 miter]); the repeated tile is
          // stroked along the path instead of filling it.
          const pathId = p.u32();
          const subtree = p.u32();
          const tile = readRect4(p);
          const pt = [p.f32(), p.f32(), p.f32(), p.f32(), p.f32(), p.f32()];
          const opacity = p.f32();
          const strokeW = p.f32();
          const strokeCap = p.u8();
          const strokeJoin = p.u8();
          const strokeMiter = p.f32();
          const ctm = targetCanvas.getTotalMatrix();
          const ptM = Float32Array.of(pt[0], pt[1], pt[2], pt[3], pt[4], pt[5], 0, 0, 1);
          const m = CK.Matrix.multiply(ctm as unknown as number[], ptM);
          const sx = Math.sqrt(m[0] * m[0] + m[3] * m[3]);
          const sy = Math.sqrt(m[1] * m[1] + m[4] * m[4]);
          if (!(sx > 0) || !(sy > 0) || tile.width <= 0 || tile.height <= 0 || opacity <= 0) break;
          const tileW = Math.max(1, Math.round(tile.width * sx));
          const tileH = Math.max(1, Math.round(tile.height * sy));
          const recorder = new CK.PictureRecorder();
          const tileCanvas = recorder.beginRecording(CK.XYWHRect(0, 0, tileW, tileH));
          tileCanvas.scale(sx, sy);
          executeSubtreeOnCanvas(tileCanvas, subtree);
          const tilePicture = recorder.finishRecordingAsPicture();
          recorder.delete();
          const localMatrix = CK.Matrix.multiply(
            CK.Matrix.multiply(ptM, CK.Matrix.translated(tile.x, tile.y)),
            CK.Matrix.scaled(1 / sx, 1 / sy),
          );
          const shader = tilePicture.makeShader?.(
            CK.TileMode.Repeat,
            CK.TileMode.Repeat,
            CK.FilterMode.Linear,
            /* tileRect */ undefined,
            /* localMatrix */ localMatrix,
          );
          if (shader) {
            const paint = dispose(new CK.Paint());
            paint.setAlphaf(opacity);
            paint.setShader(dispose(shader));
            paint.setStyle(CK.PaintStyle.Stroke);
            paint.setStrokeWidth(strokeW);
            paint.setStrokeCap([CK.StrokeCap.Butt, CK.StrokeCap.Round, CK.StrokeCap.Square][strokeCap] ?? CK.StrokeCap.Butt);
            paint.setStrokeJoin([CK.StrokeJoin.Miter, CK.StrokeJoin.Round, CK.StrokeJoin.Bevel][strokeJoin] ?? CK.StrokeJoin.Miter);
            paint.setStrokeMiter(strokeMiter);
            targetCanvas.drawPath(buildPathById(CK, frame, pathId, framePathCache, dispose), paint);
          }
          tilePicture.delete();
          break;
        }
        case OP_IMAGE: {
          const image = readImageRef(p, frame);
          const x = p.f32();
          const y = p.f32();
          const paintId = p.u32();
          const ckImage = resolveFrameImage(image);
          if (!ckImage) {
            const assetDesc = image.type === 'static' ? image.assetId
              : image.type === 'video' ? `${image.assetId}@${image.timeMicros}`
              : image.type === 'generated' ? `generated:${image.id}`
              : `encoded:${image.byteId}`;
            throw new MediaValidationError(
              'image',
              assetDesc,
              `resolveFrameImage returned null for ${assetDesc}`,
            );
          }
          targetCanvas.drawImage(ckImage, x, y, paintId === NO_PAINT ? null : buildPaintById(CK, frame, paintId, dispose));
          break;
        }
        case OP_IMAGE_RECT: {
          const image = readImageRef(p, frame);
          const hasSrc = p.u8() !== 0;
          const src = readRect4(p);
          const dst = readRect4(p);
          const paintId = p.u32();
          const ckImage = resolveFrameImage(image);
          if (!ckImage) {
            const assetDesc = image.type === 'static' ? image.assetId
              : image.type === 'video' ? `${image.assetId}@${image.timeMicros}`
              : image.type === 'generated' ? `generated:${image.id}`
              : `encoded:${image.byteId}`;
            throw new MediaValidationError(
              'image',
              assetDesc,
              `resolveFrameImage returned null for ${assetDesc}`,
            );
          }
          const sourceRect = hasSrc ? ckRect(CK, src) : imageBounds(CK, ckImage);
          targetCanvas.drawImageRect(ckImage, sourceRect, ckRect(CK, dst), paintId === NO_PAINT ? dispose(new CK.Paint()) : buildPaintById(CK, frame, paintId, dispose));
          break;
        }
        case OP_LOTTIE_RECT: {
          const bundleId = frame.strings[p.u32()] ?? '';
          const lottieFrame = p.f32();
          const dst = readRect4(p);
          const anim = resolveLottieAnimation(CK, bundleId);
          if (!anim) {
            throw new MediaValidationError(
              'lottie',
              bundleId,
              `resolveLottieAnimation returned null for bundleId="${bundleId}"`,
            );
          }
          anim.seekFrame(lottieFrame, undefined);
          const dstRect = ckRect(CK, dst);
          targetCanvas.save();
          targetCanvas.clipRect(dstRect, CK.ClipOp.Intersect, false);
          anim.render(targetCanvas, dstRect);
          targetCanvas.restore();
          break;
        }
        case OP_RUNTIME_EFFECT:
          drawRuntimeEffect(CK, targetCanvas, frame, p, executeRangeOnCanvas, executeSubtreeOnCanvas, resolveFrameImage, dispose);
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
    // Free this frame's CanvasKit objects only after the flush above has
    // consumed the display list (reverse order: dependents first).
    for (let i = frameDisposables.length - 1; i >= 0; i--) {
      try { frameDisposables[i].delete(); } catch { /* already freed by CanvasKit */ }
    }
    frameDisposables.length = 0;
    framePathCache.clear();
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
function parseGeneratedImages(section: Uint8Array): void {
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

function requireSection(sections: Map<number, Uint8Array>, id: number): Uint8Array {
  const section = sections.get(id);
  if (!section) throw new Error(`Missing OpenCat IR section ${id}`);
  return section;
}

function parseOps(ops: Uint8Array): OpEntry[] {
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
function readOptionalMatrix(reader: BinaryReader): number[] | null {
  const present = reader.u8();
  if (present === 0) return null;
  return reader.f32Array(9) as number[];
}

function parseImageFilter(reader: BinaryReader): ImageFilterSpec {
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

function parseFilterInput(reader: BinaryReader): unknown {
  const tag = reader.u8();
  if (tag === 2) return { type: 'prim', index: reader.u16() };
  return { type: tag === 1 ? 'sourceAlpha' : 'sourceGraphic' };
}

function parseTransferFn(reader: BinaryReader): unknown {
  const tag = reader.u8();
  if (tag === 0) return { type: 'identity' };
  if (tag === 1 || tag === 2) return { type: tag === 1 ? 'table' : 'discrete', values: readF32Vec(reader) };
  if (tag === 3) return { type: 'linear', slope: reader.f32(), intercept: reader.f32() };
  return { type: 'gamma', amplitude: reader.f32(), exponent: reader.f32(), offset: reader.f32() };
}

function parseLightSpec(reader: BinaryReader): unknown {
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

function parseFilterPrimKind(reader: BinaryReader): unknown {
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
      ops.push({ kind, values: r.f32Array(PATH_OP_F32_WIDTHS[kind] ?? 0) });
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

function readImageRefFromReader(reader: BinaryReader, strings: string[]): DecodedImageRef {
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
type Dispose = <T extends { delete?: () => void } | null | undefined>(obj: T) => T;

function buildPaintById(CK: CanvasKit, frame: DecodedFrame, paintId: number, dispose: Dispose): Paint {
  return buildPaint(CK, frame.paints[paintId], 1, dispose);
}

function buildPaint(CK: CanvasKit, spec: PaintSpec | undefined, alpha: number, dispose: Dispose): Paint {
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

function buildScriptPaint(CK: CanvasKit, state: RenderState, style: 'fill' | 'stroke', dispose: Dispose): Paint {
  const paint = new CK.Paint();
  dispose(paint);
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
      paint.setPathEffect(dispose(CK.PathEffect.MakeDash(state.lineDash.intervals, state.lineDash.phase)));
    }
  }
  return paint;
}

function applyFill(CK: CanvasKit, paint: Paint, fill: FillSpec, alpha: number, dispose: Dispose): void {
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

function buildShader(CK: CanvasKit, fill: GradientFillSpec | ShaderSpec, dispose: Dispose): Shader | null {
  // 仅 GradientFillSpec（paint fill）携带 localMatrix；ShaderSpec（runtime effect child）不携带。
  const localMatrix = 'localMatrix' in fill && fill.localMatrix ? fill.localMatrix : undefined;
  if (fill.type === 'turbulence') {
    return buildTurbulenceShader(CK, fill, dispose);
  }
  // CSS Images 4 (JSX gradients) interpolates stops in premultiplied space
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
function buildTurbulenceShader(CK: CanvasKit, fill: Extract<FillSpec, { type: 'turbulence' }>, dispose: Dispose): Shader | null {
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

function buildImageFilter(CK: CanvasKit, spec: ImageFilterSpec, dispose: Dispose): ImageFilter | null {
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
  if (spec.type === 'chain') {
    // 解码侧只保证 reader 对齐（见 ImageFilterSpec chain 注释）；web 重放
    // 尚不消费链式滤镜，fail-loud 指路。
    throw new Error('chain image filters are not replayed on web yet');
  }
  return dispose(CK.ImageFilter.MakeCompose(buildImageFilter(CK, spec.outer, dispose), buildImageFilter(CK, spec.inner, dispose)));
}

function buildColorFilter(CK: CanvasKit, spec: ColorFilterSpec, dispose: Dispose): ColorFilter {
  if (spec.type === 'matrix') return dispose(CK.ColorFilter.MakeMatrix(spec.matrix));
  if (spec.type === 'blendColor') return dispose(CK.ColorFilter.MakeBlend(CK.Color4f(spec.color[0], spec.color[1], spec.color[2], spec.color[3]), mapBlendMode(CK, spec.mode)));
  if (spec.type === 'linearToSrgbGamma') return dispose(CK.ColorFilter.MakeLinearToSRGBGamma());
  return dispose(CK.ColorFilter.MakeSRGBToLinearGamma());
}

function buildMaskFilter(CK: CanvasKit, spec: MaskFilterSpec, dispose: Dispose): MaskFilter {
  return dispose(CK.MaskFilter.MakeBlur(mapBlurStyle(CK, spec.style), spec.sigma, spec.respectCtm));
}

function buildPathEffect(CK: CanvasKit, spec: PathEffectSpec, dispose: Dispose): PathEffect {
  return dispose(CK.PathEffect.MakeDash(spec.intervals, spec.phase));
}

function buildPathById(CK: CanvasKit, frame: DecodedFrame, id: number, pathCache: Map<number, Path>, dispose: Dispose): Path {
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

function applyPathPayload(CK: CanvasKit, builder: PathBuilder, payload: Payload, kind: number): void {
  // v6: `kind` is the PathOp sub-opcode (0..=8) carried by the top-level
  // opcode; the payload is the raw f32 command args (no embedded sub-opcode).
  const width = PATH_OP_F32_WIDTHS[kind] ?? 0;
  applyPathCommand(CK, builder, { kind, values: payload.f32Array(width) });
}

function applyPathCommand(CK: CanvasKit, builder: PathBuilder, op: PathCommand): void {
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
    case PATH_OP_SUB.ADD_ARC:
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

  if (image.type === 'generated') {
    // Core-rasterized color-emoji glyph. The RGBA arrived in section 12;
    // cache the CanvasKit image by id so each glyph is built exactly once.
    // Since v5+ is self-contained (no epoch), the id alone is sufficient.
    const key = `${image.id}`;
    const cached = generatedImageCache.get(key);
    if (cached) return cached;
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
    if (ckImage) {
      cacheGeneratedImage(key, ckImage);
      generatedImageRgba.delete(key);
    }
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

function drawRuntimeEffect(
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

function buildRuntimeChildShader(
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
    return dispose(img.makeShaderOptions(CK.TileMode.Clamp, CK.TileMode.Clamp, CK.FilterMode.Linear, CK.MipmapMode.None));
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
  const name = (BLEND_MODE_NAMES[value] ?? 'SrcOver') as keyof typeof CK.BlendMode;
  return CK.BlendMode[name] as CanvasKitEnum;
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

/// Debug (oracle eval_js channel): decode an encoded frame and summarize its
/// op stream — opcode names plus the geometry-bearing params. This is how a
/// "layer vanished on web" question gets answered at the IR level: if the ops
/// are present with sane coords, the bug is replay; if absent, the bug is
/// upstream in core's display build.
export function dumpEncodedFrameOps(encoded: EncodedDrawFrame, limit = 400): string {
  const frame = decodeFrame(encoded);
  const entries = parseOps(frame.ops);
  const names: Record<number, string> = {};
  for (const [name, value] of Object.entries(OP)) names[value as number] = name;

  const f32 = (p: { f32: () => number }, n: number): number[] =>
    Array.from({ length: n }, () => p.f32());
  const fx = (v: number[]): string => v.map((x) => (Math.round(x * 1000) / 1000).toString()).join(',');

  const lines: string[] = [];
  const push = (s: string) => lines.push(s);
  push(`ops=${entries.length} subtrees=${frame.subtrees.length} paths=${frame.paths.length} strings=${frame.strings.length}`);
  const n = Math.min(entries.length, limit);
  for (let i = 0; i < n; i++) {
    const e = entries[i];
    const p = new Payload(frame.ops, e.payloadOffset, e.payloadLen);
    let detail = '';
    switch (e.opcode) {
      case OP.SAVE: break;
      case OP.SAVE_LAYER: {
        const flags = p.u8();
        const v = f32(p, 4);
        detail = `flags=${flags.toString(2).padStart(3, '0')} rect=[${fx(v)}] paint=${p.u32()} alpha=${p.f32().toFixed(2)}`;
        break;
      }
      case OP.SAVE_BACKDROP_LAYER: {
        const flags = p.u8();
        const v = f32(p, 4);
        const paint = p.u32();
        const alpha = p.f32();
        const radii = f32(p, 4);
        detail = `flags=${flags.toString(2).padStart(3, '0')} rect=[${fx(v)}] paint=${paint} alpha=${alpha.toFixed(2)} r=[${fx(radii)}]`;
        break;
      }
      case OP.RESTORE: break;
      case OP.RESTORE_TO_COUNT: detail = `count=${p.u32()}`; break;
      case OP.TRANSLATE: detail = `t=[${fx(f32(p, 2))}]`; break;
      case OP.SCALE: detail = `s=[${fx(f32(p, 2))}]`; break;
      case OP.ROTATE: detail = `r=[${fx(f32(p, 3))}]`; break;
      case OP.SKEW: detail = `k=[${fx(f32(p, 2))}]`; break;
      case OP.CONCAT: detail = `m=[${fx(f32(p, 9))}]`; break;
      case OP.SET_FILL_STYLE: detail = `fill=#${p.u32().toString(16)}`; break;
      case OP.SET_STROKE_STYLE: detail = `stroke=#${p.u32().toString(16)}`; break;
      case OP.SET_LINE_WIDTH: p.f32(); break;
      case OP.SET_GLOBAL_ALPHA: detail = `a=${p.f32().toFixed(3)}`; break;
      case OP.PAINT: detail = `paint=${p.u32()}`; break;
      case OP.RECT: {
        const v = f32(p, 4);
        detail = `rect=[${fx(v)}] paint=${p.u32()}`;
        break;
      }
      case OP.R_RECT: {
        const v = f32(p, 8);
        detail = `rr=[${fx(v.slice(0, 4))}] r=[${fx(v.slice(4))}] paint=${p.u32()}`;
        break;
      }
      case OP.D_RRECT: {
        const outer = f32(p, 8);
        const inner = f32(p, 8);
        detail = `outer=[${fx(outer)}] inner=[${fx(inner)}] paint=${p.u32()}`;
        break;
      }
      case OP.OVAL: {
        const v = f32(p, 4);
        detail = `oval=[${fx(v)}] paint=${p.u32()}`;
        break;
      }
      case OP.CIRCLE: detail = `c=[${fx(f32(p, 3))}] paint=${p.u32()}`; break;
      case OP.DRAW_PATH: detail = `path=${p.u32()} paint=${p.u32()}`; break;
      case OP.IMAGE_RECT: {
        // tag u8 + id-slot 12B (u32 string + u64 time) + src-present u8 +
        // src 4xf32 + dst 4xf32 + paint u32 (0xFFFFFFFF = no paint).
        const tag = p.u8();
        const asset = frame.strings[p.u32()] ?? '?';
        const time = p.u64();
        const hasSrc = p.u8() === 1;
        const src = f32(p, 4);
        const dst = f32(p, 4);
        const paintId = p.u32();
        const paint = paintId === 0xffff_ffff ? '' : ` paint=${paintId}`;
        detail = `tag=${tag} ${asset}@${Number(time) / 1e6}s src=${hasSrc ? `[${fx(src)}]` : 'none'} dst=[${fx(dst)}]${paint}`;
        break;
      }
      case OP.REPLAY_RANGE: detail = `range=[${p.u32()}+${p.u32()}]`; break;
      case OP.DRAW_SUBTREE_PICTURE: detail = `subtree=${p.u32()} at=[${fx(f32(p, 2))}]`; break;
      default: detail = `${e.payloadLen}B`;
    }
    push(`${i}: ${names[e.opcode] ?? e.opcode} ${detail}`.trimEnd());
  }
  if (entries.length > n) push(`… ${entries.length - n} more`);
  // Subtree op streams: the picture caches per-subtree draws.
  for (let si = 0; si < frame.subtrees.length && si < 8; si++) {
    const subEntries = parseOps(frame.subtrees[si]);
    push(`— subtree ${si}: ${subEntries.length} ops`);
  }
  return lines.join('\n');
}

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

// --- Test-only seam (issue #45) -------------------------------------------
// The decoder mutates module-level generated-image state. Exposing a narrow
// hook lets unit tests drive `decodeFrame` directly with a hand-built envelope
// and inspect/observe the cache identity semantics without a full CanvasKit +
// GPU surface. Not part of the public render API.
export const __generatedImageTestSeam = {
  /** Decode a raw OCIR v6 envelope without rendering. Exposed for tests. */
  decode: (bytes: Uint8Array) => decodeFrame(bytes),
  /** Number of generated-image RGBA entries currently held. */
  cacheSize: () => {
    let n = 0;
    for (const _key of generatedImageCache.keys()) n += 1;
    for (const _key of generatedImageRgba.keys()) n += 1;
    return n;
  },
  /** Look up a generated glyph's raw RGBA registered by id. */
  rgbaFor: (id: bigint) => generatedImageRgba.get(`${id}`),
  /** Reset all generated-image state (for test isolation). */
  reset: () => {
    for (const img of generatedImageCache.values()) {
      try { img?.delete?.(); } catch { /* test-only; ignore */ }
    }
    generatedImageCache.clear();
    generatedImageRgba.clear();
  },
};
