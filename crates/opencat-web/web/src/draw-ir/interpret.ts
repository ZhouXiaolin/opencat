/// OCIR DrawOp 解释面：`renderEncodedDrawFrame` 是 wire → CanvasKit 的
/// 主重放入口。op 流在这里被解释为 canvas 调用；字节解码在 decode、
/// CK 构造在 ck-adapter。svg `<mask>` 的 coverage 栈（mask_stack /
/// layer_marks / mask_replay 三件套）是 executor 级状态，与 replay.rs
/// 对齐。

import type { Canvas, CanvasKit, Image, ImageFilter, Paint, Path, PathBuilder, Rect, Surface } from 'canvaskit-wasm';
import { OP } from '../generated/ocir-schema.generated';
import {
  OP_SAVE,
  OP_SAVE_LAYER,
  OP_SAVE_BACKDROP_LAYER,
  OP_RESTORE,
  OP_RESTORE_TO_COUNT,
  OP_TRANSLATE,
  OP_SCALE,
  OP_ROTATE,
  OP_SKEW,
  OP_CONCAT,
  OP_SET_FILL_STYLE,
  OP_SET_STROKE_STYLE,
  OP_SET_LINE_WIDTH,
  OP_SET_LINE_CAP,
  OP_SET_LINE_JOIN,
  OP_SET_LINE_DASH,
  OP_CLEAR_LINE_DASH,
  OP_SET_GLOBAL_ALPHA,
  OP_SET_ANTI_ALIAS,
  OP_SET_BLEND_MODE,
  OP_BEGIN_PATH,
  OP_FILL_PATH,
  OP_STROKE_PATH,
  OP_CLIP_PATH,
  OP_CLEAR,
  OP_PAINT,
  OP_RECT,
  OP_R_RECT,
  OP_D_RRECT,
  OP_OVAL,
  OP_CIRCLE,
  OP_ARC,
  OP_LINE,
  OP_POINTS,
  OP_DRAW_PATH,
  OP_IMAGE,
  OP_IMAGE_RECT,
  OP_RUNTIME_EFFECT,
  OP_REPLAY_RANGE,
  OP_DRAW_SUBTREE_PICTURE,
  OP_FILL_PATTERN,
  OP_STROKE_PATTERN,
  OP_LOTTIE_RECT,
  OP_PATH_FIRST,
  OP_PATH_LAST,
  OP_PATH_MOVE_TO,
  OP_PATH_LINE_TO,
  OP_PATH_QUAD_TO,
  OP_PATH_CUBIC_TO,
  OP_PATH_CLOSE,
  OP_PATH_ADD_RECT,
  OP_PATH_ADD_RRECT,
  OP_PATH_ADD_OVAL,
  OP_PATH_ADD_ARC,
} from './decode';
import {
  type DecodedFrame,
  type DecodedImageRef,
  type EncodedDrawFrame,
  type ExecuteSubtreeOnCanvas,
  type OpEntry,
  Payload,
  type Rect4,
  decodeFrame,
  parseOps,
  validateFrameMedia,
  MediaValidationError,
  NO_PAINT,
  readImageRef,
  type ExecuteRangeOnCanvas,
} from './decode';
import {
  type RenderState,
  type StyleGradientSpec,
  type Dispose,
  initialRenderState,
  buildPaint,
  buildPaintById,
  buildScriptPaint,
  parseStyleGradientStops,
  buildStyleGradientShader,
  buildPathById,
  applyPathPayload,
  ckRect,
  ckRRect,
  imageBounds,
  resolveLottieAnimation,
  resolveImage,
  drawRuntimeEffect,
  mapFillType,
  mapPointMode,
  buildImageFilter,
  readRect4,
  readDRRect,
  colorU32ToF32,
} from './ck-adapter';

type RenderEncodedDrawFrameOptions = {
  surface?: Surface;
};

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

  // svg `<mask>` coverage（MASK_CLIP）：mask 内容子树在全帧离屏上重放、
  // 折算成单通道 coverage 入栈，所在层 Restore 前以 DstIn 罩回。栈是
  // executor 级（跨 subtree 重放共享）——对齐 replay.rs 的 mask_stack /
  // layer_marks / mask_replay 三件套。maskReplayDepth 对应 mask_replay
  // （mask/feImage 子树与 pattern tile 重放：coverage 保留、层基准屏蔽）；
  // maskIsolationDepth 对应 picture_exec 隔离（DrawSubtreePicture 与
  // runtime-effect child：coverage 一并丢弃）。
  const maskStack: Image[] = [];
  const maskLayerMarks: number[] = [];
  let maskReplayDepth = 0;
  let maskIsolationDepth = 0;
  const applyMasksTo = (canvas: Canvas) => {
    const mark = maskLayerMarks.pop();
    if (mark === undefined || maskStack.length <= mark) return;
    const paint = dispose(new CK.Paint());
    paint.setBlendMode(CK.BlendMode.DstIn);
    paint.setAntiAlias(false);
    canvas.save();
    // CanvasKit has no resetMatrix — concat the inverse to draw the
    // full-frame coverage image in device space (replay.rs reset_matrix).
    const inv = CK.M44.invert(canvas.getTotalMatrix());
    if (inv) canvas.concat(inv);
    for (const img of maskStack.splice(mark)) canvas.drawImage(img, 0, 0, paint);
    canvas.restore();
  };
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

  // 隔离重放包装（picture_exec 语义：coverage/层基准一并丢弃）——传给
  // runtime-effect child 与 DrawSubtreePicture 这类"独立 executor"路径。
  const executeSubtreeIsolated: ExecuteSubtreeOnCanvas = (targetCanvas, subtree) => {
    maskIsolationDepth++;
    try { executeSubtreeOnCanvas(targetCanvas, subtree); } finally { maskIsolationDepth--; }
  };
  const executeRangeIsolated = (targetCanvas: Canvas, start: number, len: number) => {
    maskIsolationDepth++;
    try { executeRangeOnCanvas(targetCanvas, start, len); } finally { maskIsolationDepth--; }
  };

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
          if (maskReplayDepth === 0 && maskIsolationDepth === 0) maskLayerMarks.push(maskStack.length);
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
          // svg `<mask>`：层内容完成后、弹层前把该层 coverage 以 DstIn 罩回。
          // mask 子树重放内部的 Restore 不弹主流程的层基准（mask_replay）。
          if (maskReplayDepth === 0 && maskIsolationDepth === 0) applyMasksTo(targetCanvas);
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
          // A solid style switch replaces any gradient (browser fillStyle
          // assignment semantics — replay.rs sets the shader to None).
          state.fillGradient = null;
          break;
        case OP_SET_STROKE_STYLE:
          state.strokeColor = colorU32ToF32(p.u32());
          state.strokeGradient = null;
          break;
        case OP.SET_STYLE_LINEAR_GRADIENT: {
          // Wire: [u8 stroke][2xf32 start][2xf32 end][u32 f32Pool start]
          // [u32 len] — semantics per replay.rs SetStyleLinearGradient
          // (Clamp tile, premul stop interpolation, stroke picks the paint).
          const stroke = p.u8() !== 0;
          const start = p.f32Array(2) as [number, number];
          const end = p.f32Array(2) as [number, number];
          const grad = { type: 'linear', start, end, ...parseStyleGradientStops(p, frame) } as const;
          if (stroke) state.strokeGradient = grad;
          else state.fillGradient = grad;
          break;
        }
        case OP.SET_STYLE_RADIAL_GRADIENT: {
          // Wire: [u8 stroke][3xf32 start circle][3xf32 end circle]
          // [u32 f32Pool start][u32 len] — two-point conical per replay.rs
          // SetStyleRadialGradient.
          const stroke = p.u8() !== 0;
          const start = p.f32Array(3) as [number, number, number];
          const end = p.f32Array(3) as [number, number, number];
          const grad = { type: 'radial', start, end, ...parseStyleGradientStops(p, frame) } as const;
          if (stroke) state.strokeGradient = grad;
          else state.fillGradient = grad;
          break;
        }
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
        case OP_SET_BLEND_MODE:
          // u8 wire blend enum（SET_BLEND_MODE 每帧由 core 下发，见
          // buildScriptPaint 的 setBlendMode）。未知值落到 mapBlendMode 的
          // SrcOver 兜底。
          state.blendMode = p.u8();
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
          // shader local matrix maps user space onto the tile. `rasterizeAt`
          // pins the recording resolution to a fixed bitmap size (browser
          // canvas bitmap semantics) instead of the device-derived scale.
          const pathId = p.u32();
          const subtree = p.u32();
          const tile = readRect4(p);
          const pt = [p.f32(), p.f32(), p.f32(), p.f32(), p.f32(), p.f32()];
          const opacity = p.f32();
          const rasterizeFlag = p.u8();
          const rasterizeAt = rasterizeFlag ? [p.f32(), p.f32()] : null;
          const ctm = targetCanvas.getTotalMatrix();
          const ptM = Float32Array.of(pt[0], pt[1], pt[2], pt[3], pt[4], pt[5], 0, 0, 1);
          const m = CK.Matrix.multiply(ctm as unknown as number[], ptM);
          const sx = Math.sqrt(m[0] * m[0] + m[3] * m[3]);
          const sy = Math.sqrt(m[1] * m[1] + m[4] * m[4]);
          if (!(sx > 0) || !(sy > 0) || tile.width <= 0 || tile.height <= 0 || opacity <= 0) break;
          const [sxT, syT] = rasterizeAt
            ? [rasterizeAt[0] / tile.width, rasterizeAt[1] / tile.height]
            : [sx, sy];
          const tileW = Math.max(1, Math.round(tile.width * sxT));
          const tileH = Math.max(1, Math.round(tile.height * syT));
          const recorder = new CK.PictureRecorder();
          const tileCanvas = recorder.beginRecording(CK.XYWHRect(0, 0, tileW, tileH));
          tileCanvas.scale(sxT, syT);
          // pattern tile replay must not touch the shared mask layer marks
          // (replay.rs sets mask_replay for the tile offscreen too).
          maskReplayDepth++;
          try { executeSubtreeOnCanvas(tileCanvas, subtree); } finally { maskReplayDepth--; }
          const tilePicture = recorder.finishRecordingAsPicture();
          recorder.delete();
          const localMatrix = CK.Matrix.multiply(
            CK.Matrix.multiply(ptM, CK.Matrix.translated(tile.x, tile.y)),
            CK.Matrix.scaled(1 / sxT, 1 / syT),
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
          // pattern tile replay must not touch the shared mask layer marks
          // (replay.rs sets mask_replay for the tile offscreen too).
          maskReplayDepth++;
          try { executeSubtreeOnCanvas(tileCanvas, subtree); } finally { maskReplayDepth--; }
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
          drawRuntimeEffect(CK, targetCanvas, frame, p, executeRangeIsolated, executeSubtreeIsolated, resolveFrameImage, dispose);
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
          executeSubtreeIsolated(targetCanvas, subtree);
          targetCanvas.restore();
          break;
        case OP.MASK_CLIP: {
          // svg `<mask>` push（payload: [u32 subtree][u8 kind][4xf32 region]）。
          // 对齐 replay.rs mask_push_skia：mask 子树在**全帧**离屏 raster 上
          // 重放（套当前 CTM、AA clip 到 region、fresh state = globalAlpha
          // 恒 1），读回按 kind 折算单通道 coverage（1=alpha；否则 premul
          // 域 luma 直算 0.2126R+0.7152G+0.0722B，与 spec 的
          // luma(unpremul)×alpha 相等），coverage 图入栈。尺寸取主 surface
          // ——与 engine 的 canvas.image_info() 等价（瓦片路径不产 MASK_CLIP）。
          const subtree = p.u32();
          const kind = p.u8();
          const region = readRect4(p);
          const fw = options.surface?.width() ?? 0;
          const fh = options.surface?.height() ?? 0;
          if (fw <= 0 || fh <= 0) break;
          const off = dispose(CK.MakeSurface(fw, fh));
          if (!off) break;
          const mc = off.getCanvas();
          mc.concat(targetCanvas.getTotalMatrix());
          mc.clipRect(
            [region.x, region.y, region.x + region.width, region.y + region.height],
            CK.ClipOp.Intersect,
            true,
          );
          maskReplayDepth++;
          try {
            executeSubtreeOnCanvas(mc, subtree);
          } finally {
            maskReplayDepth--;
          }
          const pix = mc.readPixels(0, 0, {
            width: fw,
            height: fh,
            colorType: CK.ColorType.RGBA_8888,
            alphaType: CK.AlphaType.Premul,
            colorSpace: CK.ColorSpace.SRGB,
          });
          if (!pix) break;
          const buf = new Uint8Array(fw * fh * 4);
          for (let i = 0; i < fw * fh; i++) {
            const r = pix[i * 4], g = pix[i * 4 + 1], b = pix[i * 4 + 2], a = pix[i * 4 + 3];
            const v = kind === 1 ? a : Math.min(255, Math.round(r * 0.2126 + g * 0.7152 + b * 0.0722));
            buf[i * 4 + 3] = v;
          }
          const img = dispose(CK.MakeImage(
            {
              width: fw,
              height: fh,
              colorType: CK.ColorType.RGBA_8888,
              alphaType: CK.AlphaType.Premul,
              colorSpace: CK.ColorSpace.SRGB,
            },
            buf,
            fw * 4,
          ));
          if (img && maskIsolationDepth === 0) maskStack.push(img);
          break;
        }
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

