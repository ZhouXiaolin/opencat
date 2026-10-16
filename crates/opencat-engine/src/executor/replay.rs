use super::paint::{build_skia_image_filter, paint_from_spec};
use super::path::path_from_encoded;
use super::{DrawError, DrawStats, PreparedRuntimeEffect};
use super::{EngineDrawExecutor, EnginePreparedFrameMedia};
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_op::{DRRectSpec, Radii4};
use opencat_core::ir::draw_op::{DrawOp, LineCap as OpLineCap, LineJoin as OpLineJoin, PointMode};
use opencat_core::ir::draw_types::{DrawOpRange, ImageRef, PathOp, RuntimeEffectChildRef, SubtreeId};
use skia_safe::{
    Canvas, CubicResampler, Data, FilterMode, Image, MipmapMode, Paint, PathBuilder, Picture,
    PictureRecorder, Point, RRect, Rect, SamplingOptions, Shader, TileMode, Vector, surfaces,
};

fn apply_global_alpha(paint: &mut Paint, alpha: f32) {
    let a = (paint.color().a() as f32 / 255.0) * alpha;
    paint.set_alpha((a * 255.0).round() as u8);
}

pub fn replay_frame(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
) -> Result<DrawStats, DrawError> {
    let stats = DrawStats {
        op_count: draw.ops.len() as u32,
        cache_hits: 0,
    };

    // CSS `backdrop-filter` 两遍法：这版 Skia 的 `SaveLayerRec::backdrop`
    // 嵌套在祖先层内会静默失效（BackdropProbes 单测），预录每个 backdrop 层
    // 之前的 op，层内 `draw_picture` + filter 打底，绕开嵌套限制。
    exec.backdrop_pictures = build_backdrop_pictures(exec, draw, media)?;
    exec.backdrop_next = 0;
    exec.backdrop_building = false;
    for op in &draw.ops {
        replay_op(exec, canvas, draw, media, op)?;
    }
    exec.backdrop_pictures.clear();
    Ok(stats)
}

fn build_backdrop_pictures(
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
) -> Result<Vec<Picture>, DrawError> {
    let count = draw
        .ops
        .iter()
        .filter(|op| matches!(op, DrawOp::SaveBackdropLayer { .. }))
        .count();
    if count == 0 {
        return Ok(Vec::new());
    }
    // 预录期间计数器停用：前缀里若再遇到更早的 backdrop 层，走原始 Skia
    // 路径（嵌套 backdrop 本就失效，不影响「层前内容」的近似）。
    exec.backdrop_building = true;
    let mut pictures = Vec::with_capacity(count);
    for (i, op) in draw.ops.iter().enumerate() {
        if !matches!(op, DrawOp::SaveBackdropLayer { .. }) {
            continue;
        }
        let mut recorder = PictureRecorder::new();
        // cull 只参与 bounds 估计，op 可画出界：给一个覆盖任意合成的巨矩形。
        let rec_canvas =
            recorder.begin_recording(Rect::new(-1.0e6, -1.0e6, 1.0e6, 1.0e6), false);
        replay_range(
            exec,
            rec_canvas,
            draw,
            media,
            DrawOpRange {
                start_op: 0,
                op_len: i as u32,
            },
        )?;
        drop(rec_canvas);
        pictures.push(
            recorder
                .finish_recording_as_picture(None)
                .ok_or_else(|| {
                    DrawError("backdrop 前缀录制失败（recorder 处于无效状态）".to_string())
                })?,
        );
    }
    exec.backdrop_building = false;
    Ok(pictures)
}

fn replay_range(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    range: DrawOpRange,
) -> Result<(), DrawError> {
    let start = range.start_op as usize;
    let end = start + range.op_len as usize;
    if end > draw.ops.len() {
        return Err(DrawError(format!(
            "ReplayRange out of bounds: {start}..{end} (len={})",
            draw.ops.len()
        )));
    }

    for op in &draw.ops[start..end] {
        replay_op(exec, canvas, draw, media, op)?;
    }
    Ok(())
}

fn rect_union(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.min(b.left),
        a.top.min(b.top),
        a.right.max(b.right),
        a.bottom.max(b.bottom),
    )
}

fn op_bounds(draw: &DrawOpFrame, op: &DrawOp) -> Option<Rect> {
    match op {
        DrawOp::SaveLayer {
            bounds: Some(rect), ..
        }
        | DrawOp::Rect { rect, .. }
        | DrawOp::RRect { rect, .. }
        | DrawOp::Oval { rect, .. }
        | DrawOp::Arc { rect, .. }
        | DrawOp::RuntimeEffect { dst: rect, .. }
        | DrawOp::ImageRect { dst: rect, .. } => Some(Rect::new(
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
        )),
        DrawOp::DRRect { outer, .. } => Some(Rect::new(
            outer.rect.x,
            outer.rect.y,
            outer.rect.x + outer.rect.width,
            outer.rect.y + outer.rect.height,
        )),
        DrawOp::Circle { cx, cy, radius, .. } => Some(Rect::new(
            cx - radius,
            cy - radius,
            cx + radius,
            cy + radius,
        )),
        DrawOp::Line { x0, y0, x1, y1, .. } => Some(Rect::new(
            x0.min(*x1),
            y0.min(*y1),
            x0.max(*x1),
            y0.max(*y1),
        )),
        DrawOp::ReplayRange { range } => range_bounds(draw, *range),
        DrawOp::ReplaySubtreePicture { subtree, .. } => subtree_bounds(draw, *subtree),
        _ => None,
    }
}

fn range_bounds(draw: &DrawOpFrame, range: DrawOpRange) -> Option<Rect> {
    let start = range.start_op as usize;
    let end = start.checked_add(range.op_len as usize)?;
    let ops = draw.ops.get(start..end)?;
    ops.iter()
        .filter_map(|op| op_bounds(draw, op))
        .reduce(rect_union)
}

fn replay_subtree(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    subtree: SubtreeId,
) -> Result<(), DrawError> {
    let Some(ops) = draw.subtrees.get(subtree.0 as usize) else {
        return Err(DrawError(format!(
            "Subtree out of bounds: {} (len={})",
            subtree.0,
            draw.subtrees.len()
        )));
    };
    for op in ops {
        replay_op(exec, canvas, draw, media, op)?;
    }
    Ok(())
}

fn subtree_bounds(draw: &DrawOpFrame, subtree: SubtreeId) -> Option<Rect> {
    let ops = draw.subtrees.get(subtree.0 as usize)?;
    ops.iter()
        .filter_map(|op| op_bounds(draw, op))
        .reduce(rect_union)
}

fn picture_shader_for_range(
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    range: DrawOpRange,
    fallback_bounds: Rect,
) -> Result<Option<Shader>, DrawError> {
    // RuntimeEffect picture children are sampled in the destination shader
    // space (`dst`/`xy`), not in a tight local content box. If we pass only
    // the recorded ops' minimal bounds here, Skia will clamp sampling to that
    // narrow strip and the rest of the destination will smear the edge pixel.
    // Keep the picture shader aligned to at least the destination bounds while
    // still expanding to include any recorded content that spills outside it.
    let bounds = range_bounds(draw, range)
        .map(|recorded| rect_union(recorded, fallback_bounds))
        .unwrap_or(fallback_bounds);
    let mut recorder = PictureRecorder::new();
    let picture_canvas = recorder.begin_recording(bounds, false);
    let mut picture_exec = EngineDrawExecutor::new();
    picture_exec.begin_frame();
    replay_range(&mut picture_exec, picture_canvas, draw, media, range)?;
    let Some(picture): Option<Picture> = recorder.finish_recording_as_picture(Some(&bounds)) else {
        return Ok(None);
    };
    let shader = picture.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        FilterMode::Linear,
        None::<&skia_safe::Matrix>,
        Some(&bounds),
    );
    exec.compiled_pictures
        .insert(picture.unique_id() as u64, picture);
    Ok(Some(shader))
}

fn picture_shader_for_subtree(
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    subtree: SubtreeId,
    fallback_bounds: Rect,
) -> Result<Option<Shader>, DrawError> {
    let bounds = subtree_bounds(draw, subtree).unwrap_or(fallback_bounds);
    let mut recorder = PictureRecorder::new();
    let picture_canvas = recorder.begin_recording(bounds, false);
    let mut picture_exec = EngineDrawExecutor::new();
    picture_exec.begin_frame();
    replay_subtree(&mut picture_exec, picture_canvas, draw, media, subtree)?;
    let Some(picture): Option<Picture> = recorder.finish_recording_as_picture(Some(&bounds)) else {
        return Ok(None);
    };
    let shader = picture.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        FilterMode::Linear,
        None::<&skia_safe::Matrix>,
        Some(&bounds),
    );
    exec.compiled_pictures
        .insert(picture.unique_id() as u64, picture);
    Ok(Some(shader))
}

/// True when `src` maps onto `dst` pixel-for-pixel under the canvas' current
/// total matrix: no scaling, no sub-pixel offset. Such a draw is a straight
/// copy, so a reconstruction filter can only blur it.
///
/// `src` is `None` for `drawImageRect`'s whole-image form, which Skia treats as
/// `(0, 0, image_w, image_h)`.
fn is_identity_blit(canvas: &Canvas, src_size: (f32, f32), src: Option<&Rect>, dst: &Rect) -> bool {
    let (sx0, sy0, sx1, sy1) = match src {
        Some(r) => (r.left, r.top, r.right, r.bottom),
        None => (0.0, 0.0, src_size.0, src_size.1),
    };
    let m = canvas.total_matrix();
    let p0 = m.map_point(Point::new(dst.left, dst.top));
    let p1 = m.map_point(Point::new(dst.right, dst.top));
    let p2 = m.map_point(Point::new(dst.left, dst.bottom));
    let near = |a: f32, b: f32| (a - b).abs() < 0.01;
    let integral = |v: f32| (v - v.round()).abs() < 0.01;
    near(p1.x - p0.x, sx1 - sx0)
        && near(p1.y - p0.y, 0.0)
        && near(p2.x - p0.x, 0.0)
        && near(p2.y - p0.y, sy1 - sy0)
        && integral(p0.x)
        && integral(p0.y)
}

/// Net scale in device space: device pixels per source pixel, after the canvas'
/// current total matrix. Layout sizes lie when an ancestor transform scales the
/// canvas, so sampling decisions must use this value, not `dst`'s layout size.
///
/// `src` is `None` for `drawImageRect`'s whole-image form, which Skia treats as
/// `(0, 0, image_w, image_h)`.
fn device_scale(canvas: &Canvas, src_size: (f32, f32), src: Option<&Rect>, dst: &Rect) -> f32 {
    let (sx0, sy0, sx1, sy1) = match src {
        Some(r) => (r.left, r.top, r.right, r.bottom),
        None => (0.0, 0.0, src_size.0, src_size.1),
    };
    let m = canvas.total_matrix();
    let p0 = m.map_point(Point::new(dst.left, dst.top));
    let p1 = m.map_point(Point::new(dst.right, dst.top));
    let p2 = m.map_point(Point::new(dst.left, dst.bottom));
    let dev_w = (p1.x - p0.x).hypot(p1.y - p0.y);
    let dev_h = (p2.x - p0.x).hypot(p2.y - p0.y);
    let sw = (sx1 - sx0).abs();
    let sh = (sy1 - sy0).abs();
    let sx = if sw > 0.0 { dev_w / sw } else { 1.0 };
    let sy = if sh > 0.0 { dev_h / sh } else { 1.0 };
    sx.max(sy)
}

/// 分带并行光栅一个 runtime effect，并把各带拼回 `canvas`。
///
/// 调用前提（不满足时返回 `None`，由调用方整块绘制）：
/// - liangyi 后端声明该内核可按整数像素行分带（[`PreparedRuntimeEffect::plan`]）；
/// - canvas 当前总矩阵是整数平移（分带用的局部光栅面按像素行对齐）；
/// - 无 child 采样（有 child 时坐标不是像素的纯函数，分带会改像素）。
///
/// 像素论证：内核输出只依赖自身 `xy`；带边界落在整数像素行上，两条相邻带
/// 的矩形并集正好是 dst，且互不重叠。所以每条带独立光栅后按 SrcOver 依次
/// 画回，结果与整块绘制逐位相同。
fn draw_runtime_effect_banded(
    canvas: &Canvas,
    prepared: &PreparedRuntimeEffect,
    uniform_bytes: &[u8],
    dst: Rect,
) -> Option<()> {
    use opencat_liangyi::ExecutionPlan;

    if prepared.plan != ExecutionPlan::Rows {
        return None;
    }
    let width = dst.width().round();
    let height = dst.height().round();
    if width < 1.0 || height < 1.0 {
        return None;
    }
    // 分带要求 dst 在设备空间是整数平移：矩阵只能是平移，且平移量为整像素。
    // 否则带图像拼回时要重采样，结果不再与整块绘制逐位相同。
    let m = canvas.total_matrix();
    if m.skew_x() != 0.0 || m.skew_y() != 0.0 || m.scale_x() != 1.0 || m.scale_y() != 1.0 {
        return None;
    }
    let integral = |v: f32| (v - v.round()).abs() < 1e-4;
    if !integral(m.translate_x()) || !integral(m.translate_y()) {
        return None;
    }
    // 带图像在拼回时才受 clip 约束，而整块路径在光栅时受约束。两者要一致，
    // 就必须让 dst 完全落在当前设备 clip 内（此时 clip 对两条路径都不起作用）。
    let dev = m.map_rect(dst).0;
    if let Some(clip) = canvas.device_clip_bounds() {
        let contains = dev.left >= clip.left as f32
            && dev.top >= clip.top as f32
            && dev.right <= clip.right as f32
            && dev.bottom <= clip.bottom as f32;
        if !contains {
            return None;
        }
    }
    let (w, h) = (width as u32, height as u32);

    let uniform_data = Data::new_copy(uniform_bytes);
    // 只捕获 Send+Sync 的键与源码：`RuntimeEffect` 句柄本身不能跨线程共享。
    let hash = prepared.hash;
    let sksl = prepared.sksl.as_str();
    let parts = opencat_liangyi::bands::try_parallel(w, h, |band| -> Option<Image> {
        // 每个线程自建 surface 与 RuntimeEffect（skia_safe 对象非 Send）。
        let effect = crate::consumer::compile_runtime_effect_cached(hash, sksl).ok()?;
        let shader = effect.make_shader(uniform_data.clone(), &[], None::<&skia_safe::Matrix>)?;
        let mut surface = surfaces::raster_n32_premul((w as i32, band.height as i32))?;
        let band_canvas = surface.canvas();
        // 内核按 `xy - u_oc_rect.xy` 取本地坐标：把 rect 原点平移 -dst.xy，
        // 再把带顶移回 0，于是本带第 0 行对应 rect 的第 band.y0 行。
        band_canvas.translate((-dst.left, -dst.top - band.y0 as f32));
        let mut paint = Paint::default();
        paint.set_shader(Some(shader));
        band_canvas.draw_rect(dst, &paint);
        drop(band_canvas);
        Some(surface.image_snapshot())
    })?;

    let paint = Paint::default();
    // 全有或全无：任一条带失败就整块回退，绝不把半张结果画上 canvas
    // （否则调用方的整块回退会在已画的带上再叠一层）。
    let mut images = Vec::with_capacity(parts.len());
    for (band, image) in parts {
        images.push((band, image?));
    }
    for (band, image) in images {
        canvas.draw_image_with_sampling_options(
            &image,
            Point::new(dst.left, dst.top + band.y0 as f32),
            SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
            Some(&paint),
        );
    }
    Some(())
}

fn replay_op(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    op: &DrawOp,
) -> Result<(), DrawError> {
    match op {
        DrawOp::Save => {
            canvas.save();
            Ok(())
        }
        DrawOp::Restore => {
            canvas.restore();
            Ok(())
        }
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
            backdrop,
        } => {
            let sk_rect = bounds.map(|r| Rect::new(r.x, r.y, r.x + r.width, r.y + r.height));
            match paint {
                Some(pid) => {
                    // The layer alpha must compose with the paint (it used to
                    // be dropped whenever a paint was present, which made
                    // filtered layers render fully opaque).
                    let spec = &draw.paints[pid.0 as usize];
                    let mut sk_paint = paint_from_spec(spec);
                    if *alpha < 1.0 {
                        apply_global_alpha(&mut sk_paint, *alpha);
                    }
                    // CSS `backdrop-filter` semantics (op flag `backdrop`):
                    // the image filter applies to what is already on the
                    // canvas, and must not ALSO filter the layer's own
                    // content.
                    let backdrop_imf = if *backdrop {
                        spec.image_filter
                            .as_ref()
                            .and_then(super::paint::build_skia_image_filter)
                    } else {
                        None
                    };
                    if backdrop_imf.is_some() {
                        sk_paint.set_image_filter(None);
                    }
                    let mut rec = skia_safe::canvas::SaveLayerRec::default();
                    if let Some(ref imf) = backdrop_imf {
                        rec = rec.backdrop(imf);
                    }
                    let rec = rec.paint(&sk_paint);
                    let rec = match sk_rect {
                        Some(ref r) => rec.bounds(r),
                        None => rec,
                    };
                    canvas.save_layer(&rec);
                }
                None => {
                    canvas.save_layer_alpha(sk_rect, (*alpha * 255.0) as u32);
                }
            }
            Ok(())
        }
        DrawOp::SaveBackdropLayer { bounds, paint } => {
            // CSS `backdrop-filter`: the image filter processes what is
            // already drawn beneath the layer bounds; the layer's own content
            // composites unfiltered on top.
            let sk_rect = bounds.map(|r| Rect::new(r.x, r.y, r.x + r.width, r.y + r.height));
            let Some(pid) = paint else {
                canvas.save_layer_alpha(sk_rect, 255);
                return Ok(());
            };
            let spec = &draw.paints[pid.0 as usize];
            let backdrop = spec.image_filter.as_ref().and_then(build_skia_image_filter);
            let mut sk_paint = paint_from_spec(spec);
            // Detach the filter from the content paint — it belongs to
            // the backdrop slot, not the layer contents.
            sk_paint.set_image_filter(None);

            // Two-pass: this Skia silently drops `SaveLayerRec::backdrop`
            // when the layer nests inside another layer (BackdropProbes).
            // Draw the pre-recorded prefix as the layer base instead —
            // `draw_picture` with an image filter rasterizes the whole
            // picture offscreen, filters it, then composites, which matches
            // "blur what is already beneath" exactly.
            let slot = exec.backdrop_next;
            let use_snapshot =
                !exec.backdrop_building && backdrop.is_some() && slot < exec.backdrop_pictures.len();
            if !exec.backdrop_building {
                exec.backdrop_next += 1;
            }
            if use_snapshot {
                let picture = &exec.backdrop_pictures[slot];
                // 层不设 bounds：box-shadow 需要越界绘制。backdrop 的显示
                // 范围由 `bounds`（元素 border box）单独裁剪——CSS 语义里
                // 毛玻璃只出现在元素自身区域内。
                let rec = skia_safe::canvas::SaveLayerRec::default().paint(&sk_paint);
                canvas.save_layer(&rec);
                canvas.save();
                if let Some(ref r) = sk_rect {
                    canvas.clip_rect(
                        Rect::new(r.left, r.top, r.right, r.bottom),
                        skia_safe::ClipOp::Intersect,
                        false,
                    );
                }
                // picture 记录了录制时的矩阵（从 identity 开始的前缀），回放
                // 必须回到同一坐标系，否则前缀内容被当前 CTM 二次变换。
                canvas.set_matrix(&skia_safe::M44::default());
                let mut base = Paint::default();
                base.set_image_filter(backdrop);
                canvas.draw_picture(picture, None, Some(&base));
                canvas.restore();
            } else {
                let mut rec = skia_safe::canvas::SaveLayerRec::default().paint(&sk_paint);
                if let Some(ref filter) = backdrop {
                    rec = rec.backdrop(filter);
                }
                rec = match sk_rect {
                    Some(ref r) => rec.bounds(r),
                    None => rec,
                };
                canvas.save_layer(&rec);
            }
            Ok(())
        }
        DrawOp::RestoreToCount { count } => {
            canvas.restore_to_count(*count as usize);
            Ok(())
        }

        DrawOp::Translate { x, y } => {
            canvas.translate((*x, *y));
            Ok(())
        }
        DrawOp::Scale { x, y } => {
            canvas.scale((*x, *y));
            Ok(())
        }
        DrawOp::Rotate { degrees, cx, cy } => {
            canvas.rotate(*degrees, Some(Point::new(*cx, *cy)));
            Ok(())
        }
        DrawOp::Skew { sx, sy } => {
            canvas.skew((*sx, *sy));
            Ok(())
        }
        DrawOp::Concat { matrix } => {
            canvas.concat(&skia_safe::Matrix::new_all(
                matrix[0], matrix[3], matrix[6], matrix[1], matrix[4], matrix[7], matrix[2],
                matrix[5], matrix[8],
            ));
            Ok(())
        }

        DrawOp::SetFillStyle { color } => {
            let c = skia_safe::Color::from_argb(color.a, color.r, color.g, color.b);
            exec.current_fill_paint.set_color(c);
            Ok(())
        }
        DrawOp::SetStrokeStyle { color } => {
            let c = skia_safe::Color::from_argb(color.a, color.r, color.g, color.b);
            exec.current_stroke_paint.set_color(c);
            Ok(())
        }
        DrawOp::SetLineWidth { width } => {
            exec.current_stroke_paint.set_stroke_width(*width);
            Ok(())
        }
        DrawOp::SetLineCap { cap } => {
            exec.current_stroke_paint.set_stroke_cap(match cap {
                OpLineCap::Butt => skia_safe::paint::Cap::Butt,
                OpLineCap::Round => skia_safe::paint::Cap::Round,
                OpLineCap::Square => skia_safe::paint::Cap::Square,
            });
            Ok(())
        }
        DrawOp::SetLineJoin { join } => {
            exec.current_stroke_paint.set_stroke_join(match join {
                OpLineJoin::Miter => skia_safe::paint::Join::Miter,
                OpLineJoin::Round => skia_safe::paint::Join::Round,
                OpLineJoin::Bevel => skia_safe::paint::Join::Bevel,
            });
            Ok(())
        }
        DrawOp::SetLineDash { intervals, phase } => {
            let start = intervals.start as usize;
            let end = start + intervals.len as usize;
            if end > draw.f32_pool.len() {
                return Err(DrawError(format!("SetLineDash f32_pool out of bounds")));
            }
            let dash: Vec<f32> = draw.f32_pool[start..end].to_vec();
            if let Some(pe) = skia_safe::PathEffect::dash(&dash, *phase) {
                exec.current_stroke_paint.set_path_effect(pe);
            }
            Ok(())
        }
        DrawOp::ClearLineDash => {
            exec.current_stroke_paint
                .set_path_effect(None::<skia_safe::PathEffect>);
            Ok(())
        }
        DrawOp::SetGlobalAlpha { alpha } => {
            exec.current_alpha = alpha.clamp(0.0, 1.0);
            Ok(())
        }
        DrawOp::SetAntiAlias { enabled } => {
            exec.current_fill_paint.set_anti_alias(*enabled);
            exec.current_stroke_paint.set_anti_alias(*enabled);
            Ok(())
        }

        DrawOp::BeginPath => {
            exec.current_path = Some(PathBuilder::new());
            Ok(())
        }
        DrawOp::Path(path_op) => {
            if let Some(ref mut pb) = exec.current_path {
                apply_path_op(pb, path_op);
            }
            Ok(())
        }
        DrawOp::FillPath => {
            if let Some(mut pb) = exec.current_path.take() {
                let path = pb.detach();
                let mut paint = exec.current_fill_paint.clone();
                apply_global_alpha(&mut paint, exec.current_alpha);
                canvas.draw_path(&path, &paint);
                exec.current_path = Some(PathBuilder::new());
            }
            Ok(())
        }
        DrawOp::StrokePath => {
            if let Some(mut pb) = exec.current_path.take() {
                let path = pb.detach();
                let mut paint = exec.current_stroke_paint.clone();
                apply_global_alpha(&mut paint, exec.current_alpha);
                canvas.draw_path(&path, &paint);
                exec.current_path = Some(PathBuilder::new());
            }
            Ok(())
        }
        DrawOp::ClipPath { anti_alias } => {
            if let Some(mut pb) = exec.current_path.take() {
                let path = pb.detach();
                canvas.clip_path(&path, skia_safe::ClipOp::Intersect, *anti_alias);
                exec.current_path = Some(PathBuilder::new());
            }
            Ok(())
        }

        DrawOp::Clear { color } => {
            canvas.clear(skia_safe::Color4f {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            });
            Ok(())
        }
        DrawOp::Paint { paint: pid } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_paint(&paint);
            Ok(())
        }
        DrawOp::Rect { rect, paint: pid } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_rect(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                &paint,
            );
            Ok(())
        }
        DrawOp::RRect {
            rect,
            radii,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            let r = RRect::new_rect_radii(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                &radii4_to_vectors(radii),
            );
            canvas.draw_rrect(&r, &paint);
            Ok(())
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            let o = drrect_to_skia(outer);
            let i = drrect_to_skia(inner);
            canvas.draw_drrect(&o, &i, &paint);
            Ok(())
        }
        DrawOp::Oval { rect, paint: pid } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_oval(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                &paint,
            );
            Ok(())
        }
        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_circle((*cx, *cy), *radius, &paint);
            Ok(())
        }
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_arc(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                *start,
                *sweep,
                *use_center,
                &paint,
            );
            Ok(())
        }
        DrawOp::Line {
            x0,
            y0,
            x1,
            y1,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_line((*x0, *y0), (*x1, *y1), &paint);
            Ok(())
        }
        DrawOp::Points {
            mode,
            points,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            let start = points.start as usize;
            let end = start + points.len as usize;
            if end > draw.f32_pool.len() {
                return Err(DrawError(format!("Points f32_pool out of bounds")));
            }
            let pts: Vec<Point> = draw.f32_pool[start..end]
                .chunks(2)
                .map(|c| Point::new(c[0], c[1]))
                .collect();
            let sk_mode = match mode {
                PointMode::Points => skia_safe::canvas::PointMode::Points,
                PointMode::Lines => skia_safe::canvas::PointMode::Lines,
                PointMode::Polygon => skia_safe::canvas::PointMode::Polygon,
            };
            canvas.draw_points(sk_mode, &pts, &paint);
            Ok(())
        }
        DrawOp::DrawPath { path, paint: pid } => {
            let sk_path = path_from_encoded(&draw.paths[path.0 as usize]);
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_path(&sk_path, &paint);
            Ok(())
        }
        DrawOp::Image {
            image,
            x,
            y,
            paint: pid,
        } => {
            if let Some(idx) = media.image_index.get(image) {
                let sk_image = &media.images[*idx];
                // 预缩小解码后 canvas.drawImage 的"自然尺寸"语义补偿：解码图
                // 是声明尺寸的 1/scale 倍，补足 dst 再画，保持像素尺寸不变。
                // 同一资产既走 ImageRect（贡献预缩目标）又走 drawImage 时，
                // 两条路径都继续以声明尺寸呈现。
                let decode_scale = media.image_scales.get(*idx).copied().unwrap_or(1.0);
                if (decode_scale - 1.0).abs() > 1e-6 {
                    let dst = Rect::new(
                        *x,
                        *y,
                        *x + sk_image.width() as f32 / decode_scale,
                        *y + sk_image.height() as f32 / decode_scale,
                    );
                    let mut paint = match pid {
                        Some(pid) => paint_from_spec(&draw.paints[pid.0 as usize]),
                        None => Paint::default(),
                    };
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image_rect_with_sampling_options(
                        sk_image,
                        None,
                        dst,
                        SamplingOptions::default(),
                        &paint,
                    );
                } else if let Some(pid) = pid {
                    let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image(sk_image, (*x, *y), Some(&paint));
                } else {
                    canvas.draw_image(sk_image, (*x, *y), None);
                }
            }
            Ok(())
        }
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint: pid,
        } => {
            if let Some(idx) = media.image_index.get(image) {
                let sk_image = &media.images[*idx];
                // 预缩小解码：src 以声明（原图）坐标给出，按解码缩放比映射到
                // 解码像素坐标。scale=1（native/lazy）时不变；src 与图像尺寸
                // 同比缩放，下方采样判定所依据的比值不受影响。
                let decode_scale = media.image_scales.get(*idx).copied().unwrap_or(1.0);
                let src_rect = src.map(|r| {
                    Rect::new(
                        r.x * decode_scale,
                        r.y * decode_scale,
                        (r.x + r.width) * decode_scale,
                        (r.y + r.height) * decode_scale,
                    )
                });
                let dst_rect = Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                let src_arg = src_rect
                    .as_ref()
                    .map(|r| (r, skia_safe::canvas::SrcRectConstraint::Fast));
                // r6: ImageRect 重采样质量。实测（examples/codex-five.xml 的 fig 1.2×
                // 放大 vs 参考视频）：Chrome 对 CSS 缩放的 video 元素用高质量 cubic
                // 重建，Mitchell(B=1/3,C=1/3) 把 fig 躯干窗 p8 从 10.0% 压到 3.9%、
                // p32 从 1.12% 压到 0.01%（catmull_rom 5.8%，nearest/旧默认 10%）。
                // 因此 VideoFrame（声明式 <video>，媒体路径）默认 = Mitchell cubic；
                // Static（JS canvas drawImageRect 的像素精确语义）保持 Skia 默认。
                // OPENCAT_IMG_SAMPLING=catmull|nearest|linear|legacy 仅供调试回退
                // （作用于 VideoFrame 路径）。
                let sampling = match std::env::var("OPENCAT_IMG_SAMPLING").as_deref() {
                    Ok("catmull") => SamplingOptions {
                        use_cubic: true,
                        cubic: CubicResampler::catmull_rom(),
                        ..SamplingOptions::default()
                    },
                    Ok("nearest") => SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                    Ok("linear") => SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    _ => match image {
                        ImageRef::VideoFrame { .. } => {
                            // Mitchell cubic is a *smoothing* reconstruction: on a
                            // 1:1 integer-aligned blit it only removes detail (a
                            // full-frame `object-cover` video at native size lost
                            // ~30% of its grain, p8 0.69 against the decoded
                            // source). Chrome's cubic reconstruction only applies
                            // to genuinely scaled video, so keep cubic for scaling
                            // and skip resampling when the draw is a straight copy.
                            let src_size = (sk_image.width() as f32, sk_image.height() as f32);
                            if is_identity_blit(canvas, src_size, src_rect.as_ref(), &dst_rect) {
                                SamplingOptions::new(FilterMode::Nearest, MipmapMode::None)
                            } else if device_scale(canvas, src_size, src_rect.as_ref(), &dst_rect)
                                <= 1.0 + 1e-3
                            {
                                // 设备空间没有放大（含缩小，以及亚像素平移的 1:1）：
                                // 用线性。cubic 核宽固定，缩小采样时既慢又混叠；
                                // 亚像素平移的 1:1 用线性即可平滑过渡。实测
                                // hyperframes-launch 帧 300/400（设备空间 1:1、
                                // 布局侧 1.42x wrapper）linear 的 p8 比 Mitchell
                                // 低约 0.003，且快约 2.6 倍。
                                SamplingOptions::new(FilterMode::Linear, MipmapMode::None)
                            } else {
                                SamplingOptions {
                                    use_cubic: true,
                                    cubic: CubicResampler::mitchell(),
                                    ..SamplingOptions::default()
                                }
                            }
                        }
                        ImageRef::Static { .. } | ImageRef::Generated { .. } => {
                            SamplingOptions::default()
                        }
                    },
                };
                if let Some(pid) = pid {
                    let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image_rect_with_sampling_options(
                        sk_image,
                        src_arg,
                        dst_rect,
                        sampling,
                        &paint,
                    );
                } else {
                    let mut paint = Paint::default();
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image_rect_with_sampling_options(
                        sk_image,
                        src_arg,
                        dst_rect,
                        sampling,
                        &paint,
                    );
                }
            }
            Ok(())
        }

        DrawOp::RuntimeEffect {
            effect,
            uniforms,
            children,
            dst,
        } => {
            let effect_idx = effect.0 as usize;
            if effect_idx < media.runtime_effects.len() {
                let prepared = &media.runtime_effects[effect_idx];
                let uniform_idx = uniforms.0 as usize;
                if uniform_idx >= draw.byte_ranges.len() {
                    return Err(DrawError(format!(
                        "RuntimeEffect bytes_range out of bounds"
                    )));
                }
                let uniform_bytes = {
                    let start = draw.byte_ranges[uniform_idx].start as usize;
                    let len = draw.byte_ranges[uniform_idx].len as usize;
                    if start + len > draw.bytes.len() {
                        return Err(DrawError(format!("RuntimeEffect bytes out of bounds")));
                    }
                    &draw.bytes[start..start + len]
                };
                let mut inputs: Vec<skia_safe::runtime_effect::ChildPtr> = Vec::new();
                let child_start = children.start as usize;
                let child_end = child_start + children.len as usize;
                if child_end > draw.children.len() {
                    return Err(DrawError(format!("RuntimeEffect children out of bounds")));
                }
                for child_ref in &draw.children[child_start..child_end] {
                    match child_ref {
                        RuntimeEffectChildRef::Image(img_ref) => {
                            if let Some(idx) = media.image_index.get(img_ref) {
                                // 约定（skill canvaskit §Effect lambda）：image
                                // child 的 tile 空间原点 = effect dst 本地原点，
                                // 与 `uv = xy - rect.xy` 对齐 —— 用本地矩阵平移
                                // dst.xy，使 child.eval(uv) 取到"该矩形自己的"
                                // 像素（与 lambda CPU 后端 ChildImage 采样一致）。
                                let mut local =
                                    skia_safe::Matrix::new_identity();
                                local.set_translate((-dst.x, -dst.y));
                                if let Some(shader) = skia_safe::shaders::image(
                                    &media.images[*idx],
                                    (skia_safe::TileMode::Clamp, skia_safe::TileMode::Clamp),
                                    &skia_safe::SamplingOptions::default(),
                                    Some(&local),
                                ) {
                                    inputs.push(shader.into());
                                } else {
                                    inputs.push(skia_safe::shaders::empty().into());
                                }
                            } else {
                                inputs.push(skia_safe::shaders::empty().into());
                            }
                        }
                        RuntimeEffectChildRef::Picture(range) => {
                            let fallback_bounds =
                                Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                            let shader = picture_shader_for_range(
                                exec,
                                draw,
                                media,
                                *range,
                                fallback_bounds,
                            )?
                            .unwrap_or_else(skia_safe::shaders::empty);
                            inputs.push(shader.into());
                        }
                        RuntimeEffectChildRef::SubtreePicture(subtree) => {
                            let fallback_bounds =
                                Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                            let shader = picture_shader_for_subtree(
                                exec,
                                draw,
                                media,
                                *subtree,
                                fallback_bounds,
                            )?
                            .unwrap_or_else(skia_safe::shaders::empty);
                            inputs.push(shader.into());
                        }
                        RuntimeEffectChildRef::Shader(_) => {
                            inputs.push(skia_safe::shaders::empty().into());
                        }
                    }
                }
                let uniform_data = skia_safe::Data::new_copy(uniform_bytes);
                // 无 child 且 liangyi 声明可分带时，先尝试分带并行光栅。
                if inputs.is_empty()
                    && draw_runtime_effect_banded(
                        canvas,
                        prepared,
                        uniform_bytes,
                        Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height),
                    )
                    .is_some()
                {
                    return Ok(());
                }
                let shader =
                    prepared
                        .effect
                        .make_shader(uniform_data, &inputs, None::<&skia_safe::Matrix>);
                if let Some(s) = shader {
                    let mut paint = Paint::default();
                    paint.set_shader(Some(s));
                    canvas.draw_rect(
                        Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height),
                        &paint,
                    );
                }
            }
            Ok(())
        }

        DrawOp::ReplayRange { range } => {
            // 子树有自己的 op 空间：backdrop 槽位指针存取，防止子树内的
            // （当前不会下发的）backdrop 层错位消费顶层 plan。
            let saved = exec.backdrop_next;
            let r = replay_range(exec, canvas, draw, media, *range);
            exec.backdrop_next = saved;
            r
        }

        DrawOp::ReplaySubtreePicture { subtree, x, y } => {
            canvas.save();
            canvas.translate(Vector::new(*x, *y));
            let saved = exec.backdrop_next;
            let result = replay_subtree(exec, canvas, draw, media, *subtree);
            exec.backdrop_next = saved;
            canvas.restore();
            result
        }

        DrawOp::DrawSubtreePicture { .. } => Ok(()),

        DrawOp::ScriptRuntimeEffect { .. } => Ok(()),

        DrawOp::LottieRect {
            bundle_id,
            frame,
            dst,
        } => {
            #[cfg(target_os = "linux")]
            if let Some(anim) = exec.lottie_cache.get(bundle_id) {
                anim.seek_frame(*frame as f64);
                let size = anim.size();
                let iw = size.width.max(1.0);
                let ih = size.height.max(1.0);
                canvas.save();
                let clip_rect = Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                canvas.clip_rect(clip_rect, None, Some(false));
                canvas.translate((dst.x, dst.y));
                canvas.scale((dst.width / iw, dst.height / ih));
                anim.render(canvas, None);
                canvas.restore();
            }
            #[cfg(not(target_os = "linux"))]
            {
                // No native Skottie renderer (feature off); skip the op.
                let _ = (frame, dst);
            }
            Ok(())
        }
    }
}

fn apply_path_op(builder: &mut PathBuilder, op: &PathOp) {
    match *op {
        PathOp::MoveTo { x, y } => {
            builder.move_to((x, y));
        }
        PathOp::LineTo { x, y } => {
            builder.line_to((x, y));
        }
        PathOp::QuadTo { cx, cy, x, y } => {
            builder.quad_to((cx, cy), (x, y));
        }
        PathOp::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => {
            builder.cubic_to((c1x, c1y), (c2x, c2y), (x, y));
        }
        PathOp::Close => {
            builder.close();
        }
        PathOp::AddRect {
            x,
            y,
            width,
            height,
        } => {
            builder.add_rect(Rect::new(x, y, x + width, y + height), None, None);
        }
        PathOp::AddRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            let r = RRect::new_rect_xy(Rect::new(x, y, x + width, y + height), radius, radius);
            builder.add_rrect(&r, None, None);
        }
        PathOp::AddOval {
            x,
            y,
            width,
            height,
        } => {
            builder.add_oval(Rect::new(x, y, x + width, y + height), None, None);
        }
        PathOp::AddArc {
            x,
            y,
            width,
            height,
            start_angle,
            sweep_angle,
        } => {
            builder.add_arc(
                Rect::new(x, y, x + width, y + height),
                start_angle,
                sweep_angle,
            );
        }
    }
}

fn radii4_to_vectors(r: &Radii4) -> [Vector; 4] {
    [
        Vector::new(r.top_left, r.top_left),
        Vector::new(r.top_right, r.top_right),
        Vector::new(r.bottom_right, r.bottom_right),
        Vector::new(r.bottom_left, r.bottom_left),
    ]
}

fn drrect_to_skia(spec: &DRRectSpec) -> RRect {
    let rect = Rect::new(
        spec.rect.x,
        spec.rect.y,
        spec.rect.x + spec.rect.width,
        spec.rect.y + spec.rect.height,
    );
    RRect::new_rect_radii(rect, &radii4_to_vectors(&spec.radii))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencat_core::canvas::paint::{FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
    use opencat_core::ir::draw_op::Rect4;
    use opencat_core::ir::draw_types::{
        BytesRangeId, ChildRange, DrawOpRange, EffectId, EffectRef,
    };
    use skia_safe::{
        AlphaType, ColorType, Data, ImageInfo, RuntimeEffect, images,
        image::CachingHint, surfaces,
    };

    fn pixel_rgba(frame: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let index = (y * width + x) * 4;
        [
            frame[index],
            frame[index + 1],
            frame[index + 2],
            frame[index + 3],
        ]
    }

    /// 分带并行光栅与整块光栅必须逐位相同 —— 这是 liangyi 声明
    /// `ExecutionPlan::Rows` 的全部理由。用够大的 rect 触发分带（小 rect
    /// 走整块），再用同一内核整块渲染一次做对照。
    #[test]
    fn banded_runtime_effect_matches_whole_raster_bit_for_bit() {
        // 坐标依赖型内核：分带若不正确，带间会错位或边缘重复。
        let source =
            "(uv, u) => { const g = fract((uv.x * 0.05 + uv.y * 0.013) * 1.0); return [g, g * 0.5, 1 - g, 1]; }";
        let compiled = opencat_liangyi::compile_effect(source, &Default::default())
            .expect("lambda should compile to sksl");
        assert_eq!(compiled.backend, opencat_liangyi::Backend::Sksl);
        let sksl = compiled.sksl.expect("sksl backend carries code");
        // 纯坐标内核必须被后端标记为可按行分带。
        assert_eq!(
            opencat_liangyi::sksl_execution_plan(&sksl),
            opencat_liangyi::ExecutionPlan::Rows,
        );

        let (w, h) = (256u32, 256u32);
        let rt = RuntimeEffect::make_for_shader(&sksl, None).expect("sksl compiles");
        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: compiled.hash,
            sksl: sksl.clone(),
        });
        let uniforms: Vec<f32> = vec![0.0, 0.0, w as f32, h as f32];
        let mut uniform_bytes = Vec::new();
        for v in &uniforms {
            uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        frame.bytes.extend_from_slice(&uniform_bytes);
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange {
                start: 0,
                len: uniform_bytes.len() as u32,
            });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: w as f32,
                height: h as f32,
            },
        });

        let read = |surface: &mut skia_safe::Surface| -> Vec<u8> {
            let image = surface.image_snapshot();
            let info = ImageInfo::new(
                (w as i32, h as i32),
                ColorType::RGBA8888,
                AlphaType::Premul,
                None,
            );
            let mut rgba = vec![0u8; (w * h * 4) as usize];
            assert!(image.read_pixels(
                &info,
                rgba.as_mut_slice(),
                w as usize * 4,
                (0, 0),
                CachingHint::Allow,
            ));
            rgba
        };

        // 分带路径：plan=Rows 且 rect 足够大。
        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![PreparedRuntimeEffect {
                effect: rt.clone(),
                plan: opencat_liangyi::ExecutionPlan::Rows,
                hash: compiled.hash,
                sksl: sksl.clone(),
            }],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut banded_surface =
            surfaces::raster_n32_premul((w as i32, h as i32)).expect("surface");
        replay_op(
            &mut exec,
            banded_surface.canvas(),
            &frame,
            &media,
            &frame.ops[0],
        )
        .expect("banded replay");
        let banded = read(&mut banded_surface);

        // 整块对照：同一内核声明为 Whole。
        let whole_media = EnginePreparedFrameMedia {
            runtime_effects: vec![PreparedRuntimeEffect {
                effect: rt,
                plan: opencat_liangyi::ExecutionPlan::Whole,
                hash: compiled.hash,
                sksl,
            }],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut whole_surface =
            surfaces::raster_n32_premul((w as i32, h as i32)).expect("surface");
        replay_op(
            &mut exec,
            whole_surface.canvas(),
            &frame,
            &whole_media,
            &frame.ops[0],
        )
        .expect("whole replay");
        let whole = read(&mut whole_surface);

        let diff = banded
            .iter()
            .zip(&whole)
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(diff, 0, "banded raster differs from whole raster in {diff} bytes");
    }

    /// 用户函数发射出的 SKSL 必须真能被 Skia 接受。liangyi 自身不能依赖
    /// skia-safe，所以只有这里能证明 codegen 产出的函数定义、`for` 循环、
    /// 形参重赋值与调用语法合法。
    #[test]
    fn user_function_kernel_compiles_in_skia() {
        let source = "(uv, u) => {\n\
          const hash = (p) => fract(sin(dot(p, [127.1, 311.7])) * 43758.5453);\n\
          const noise = (p) => {\n\
            const i = floor(p);\n\
            const f = fract(p);\n\
            const w = f * f * (3.0 - 2.0 * f);\n\
            return mix(mix(hash(i), hash(i + [1.0, 0.0]), w.x),\n\
                       mix(hash(i + [0.0, 1.0]), hash(i + [1.0, 1.0]), w.x), w.y);\n\
          };\n\
          const fbm = (p) => {\n\
            let v = 0.0;\n\
            let a = 0.5;\n\
            for (let o = 0; o < 5; o++) {\n\
              v = v + a * noise(p);\n\
              p = [0.8 * p.x - 0.6 * p.y, 0.6 * p.x + 0.8 * p.y] * 2.02;\n\
              a = a * 0.5;\n\
            }\n\
            return v;\n\
          };\n\
          const n = fbm(uv * 0.01);\n\
          return [n, n * 0.5, 1.0 - n, 1.0];\n\
        }";
        let compiled =
            opencat_liangyi::compile_effect(source, &Default::default()).expect("compile");
        assert_eq!(compiled.backend, opencat_liangyi::Backend::Sksl);
        let sksl = compiled.sksl.expect("sksl backend carries code");
        assert!(sksl.contains("float oc_f2(float2 oc_l"), "{sksl}");
        assert!(sksl.contains("for (float oc_l"), "{sksl}");
        RuntimeEffect::make_for_shader(&sksl, None)
            .expect("Skia must accept the emitted user functions");
    }

    #[test]
    fn canvas_draw_script_bounds_clip_then_script_clip_keeps_content() {
        // Mirror `render_draw_script`: it first clips the canvas to the element
        // bounds, then the script's own clip follows. Content drawn after both
        // clips must still land in the intersection.
        let mut frame = DrawOpFrame::default();
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        // 1) render_draw_script's own bounds clip (8x8 element)
        frame.ops.push(DrawOp::Save);
        frame.ops.push(DrawOp::BeginPath);
        frame.ops.push(DrawOp::Path(PathOp::AddRect {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 8.0,
        }));
        frame.ops.push(DrawOp::ClipPath { anti_alias: true });
        // 2) the script's own triangle clip (left half)
        frame.ops.push(DrawOp::Save);
        frame.ops.push(DrawOp::BeginPath);
        frame.ops.push(DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 8.0 }));
        frame.ops.push(DrawOp::Path(PathOp::Close));
        frame.ops.push(DrawOp::ClipPath { anti_alias: true });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });
        frame.ops.push(DrawOp::Restore);
        frame.ops.push(DrawOp::Restore);

        let media = EnginePreparedFrameMedia::default();
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface should create");
        let canvas = surface.canvas();
        for op in &frame.ops {
            replay_op(&mut exec, canvas, &frame, &media, op)
                .expect("clip ops should replay");
        }

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 8 * 8 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            8 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        // (3,4) lies strictly inside the triangle (the hypotenuse passes
        // through x=2 at y=4, so sampling there would measure a correct
        // ~75%-coverage anti-aliased edge instead of full paint).
        let inside = pixel_rgba(&rgba, 8, 3, 4);
        let outside = pixel_rgba(&rgba, 8, 6, 4);
        assert!(
            inside[0] > 200 && inside[3] > 200,
            "content inside both clips must be painted, got {inside:?}"
        );
        assert_eq!(
            outside[3], 0,
            "content outside the script clip must be culled, got {outside:?}"
        );
    }

    #[test]
    fn canvas_clip_coverage_matrix_dump() {
        // Diagnostic: dump center-row pixel coverage for clip variants.
        let variants: Vec<(&str, bool, bool)> = vec![
            // (name, include_bounds_clip, triangle_clip_aa)
            ("clip-only-aa", false, true),
            ("clip-only-noaa", false, false),
            ("bounds+clip-aa", true, true),
            ("bounds+clip-noaa", true, false),
        ];
        for (name, with_bounds, tri_aa) in variants {
            let mut frame = DrawOpFrame::default();
            frame.paints.push(PaintSpec {
                fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
                style: PaintStyle::Fill,
                ..Default::default()
            });
            frame.ops.push(DrawOp::Save);
            if with_bounds {
                frame.ops.push(DrawOp::BeginPath);
                frame.ops.push(DrawOp::Path(PathOp::AddRect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                }));
                frame.ops.push(DrawOp::ClipPath { anti_alias: false });
            }
            frame.ops.push(DrawOp::BeginPath);
            frame.ops.push(DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 }));
            frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 0.0 }));
            frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 8.0 }));
            frame.ops.push(DrawOp::Path(PathOp::Close));
            frame.ops.push(DrawOp::ClipPath { anti_alias: tri_aa });
            frame.ops.push(DrawOp::Rect {
                rect: Rect4 { x: 0.0, y: 0.0, width: 8.0, height: 8.0 },
                paint: opencat_core::ir::draw_types::PaintId(0),
            });
            frame.ops.push(DrawOp::Restore);

            let media = EnginePreparedFrameMedia::default();
            let mut exec = EngineDrawExecutor::new();
            exec.begin_frame();
            let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface");
            let canvas = surface.canvas();
            for op in &frame.ops {
                replay_op(&mut exec, canvas, &frame, &media, op).unwrap();
            }
            let image = surface.image_snapshot();
            let info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
            let mut rgba = vec![0_u8; 8 * 8 * 4];
            assert!(image.read_pixels(&info, rgba.as_mut_slice(), 8 * 4, (0, 0), CachingHint::Allow));
            let row: Vec<u8> = (0..8).map(|x| pixel_rgba(&rgba, 8, x, 4)[3]).collect();
            eprintln!("clip-matrix {name}: alpha row4 = {row:?}");
        }
    }

    #[test]
    fn canvas_clip_path_intersects_subsequent_draws() {
        // Repro for the codex-five bootstrap: a script canvas that builds a
        // path, clips to it, then draws — the draw must land inside the clip.
        let mut frame = DrawOpFrame::default();
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        // Clip to the left half of the 8x8 canvas.
        frame.ops.push(DrawOp::BeginPath);
        frame.ops.push(DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 8.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 0.0, y: 8.0 }));
        frame.ops.push(DrawOp::Path(PathOp::Close));
        frame.ops.push(DrawOp::ClipPath { anti_alias: true });
        // Fill the whole canvas red; only the left half may receive paint.
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });

        let media = EnginePreparedFrameMedia::default();
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface should create");
        let canvas = surface.canvas();
        for op in &frame.ops {
            replay_op(&mut exec, canvas, &frame, &media, op)
                .expect("clip ops should replay");
        }

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 8 * 8 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            8 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        let inside = pixel_rgba(&rgba, 8, 2, 4);
        let outside = pixel_rgba(&rgba, 8, 6, 4);
        assert!(
            inside[0] > 200 && inside[3] > 200,
            "draw inside the clip must be painted, got {inside:?}"
        );
        assert_eq!(
            outside[3], 0,
            "draw outside the clip must be culled, got {outside:?}"
        );
    }

    #[test]
    fn runtime_effect_picture_child_samples_draw_op_range() {
        let sksl = r#"
uniform shader child;

half4 main(float2 coord) {
    return child.eval(coord);
}
"#;
        let rt = RuntimeEffect::make_for_shader(sksl, None).expect("runtime effect should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: 0xCAFE,
            sksl: sksl.to_string(),
        });
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: 0 });
        frame
            .children
            .push(RuntimeEffectChildRef::Picture(DrawOpRange {
                start_op: 0,
                op_len: 1,
            }));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 1.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });

        let effect_op = DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
        };

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt.into()],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface should create");
        let canvas = surface.canvas();

        replay_op(&mut exec, canvas, &frame, &media, &effect_op)
            .expect("runtime effect op should replay");

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 8 * 8 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            8 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        assert_eq!(
            pixel_rgba(&rgba, 8, 4, 4),
            [0, 255, 0, 255],
            "RuntimeEffect Picture child should sample the recorded draw range"
        );
    }

    #[test]
    fn runtime_effect_picture_child_respects_translated_range_bounds() {
        let sksl = r#"
uniform shader child;

half4 main(float2 coord) {
    return child.eval(coord);
}
"#;
        let rt = RuntimeEffect::make_for_shader(sksl, None).expect("runtime effect should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: 0xBEEF,
            sksl: sksl.to_string(),
        });
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: 0 });
        frame
            .children
            .push(RuntimeEffectChildRef::Picture(DrawOpRange {
                start_op: 0,
                op_len: 2,
            }));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 1.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Translate { x: 4.0, y: 2.0 });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });

        let effect_op = DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 16.0,
                height: 16.0,
            },
        };

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt.into()],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((16, 16)).expect("surface should create");
        let canvas = surface.canvas();

        replay_op(&mut exec, canvas, &frame, &media, &effect_op)
            .expect("runtime effect op should replay");

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((16, 16), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 16 * 16 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            16 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        assert_eq!(
            pixel_rgba(&rgba, 16, 6, 4),
            [0, 255, 0, 255],
            "Picture child shader should preserve translated content bounds"
        );
        assert_eq!(
            pixel_rgba(&rgba, 16, 1, 1),
            [0, 0, 0, 0],
            "Pixels outside the translated picture should remain transparent"
        );
    }

    #[test]
    fn runtime_effect_picture_child_samples_content_translated_outside_local_op_bounds() {
        let sksl = r#"
uniform shader child;

half4 main(float2 coord) {
    return child.eval(coord);
}
"#;
        let rt = RuntimeEffect::make_for_shader(sksl, None).expect("runtime effect should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: 0xFACE,
            sksl: sksl.to_string(),
        });
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: 0 });
        frame
            .children
            .push(RuntimeEffectChildRef::Picture(DrawOpRange {
                start_op: 0,
                op_len: 2,
            }));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 1.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Translate { x: 12.0, y: 4.0 });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 4.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });

        let effect_op = DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 12.0,
            },
        };

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt.into()],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((20, 12)).expect("surface should create");
        let canvas = surface.canvas();

        replay_op(&mut exec, canvas, &frame, &media, &effect_op)
            .expect("runtime effect op should replay");

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((20, 12), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 20 * 12 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            20 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        assert_eq!(
            pixel_rgba(&rgba, 20, 13, 5),
            [0, 255, 0, 255],
            "Picture child shader should sample translated content even when it lies outside the local op bounds"
        );
        assert_eq!(
            pixel_rgba(&rgba, 20, 1, 1),
            [0, 0, 0, 0],
            "Pixels outside the translated content should remain transparent"
        );
    }

    /// CSS chained text-shadow semantics for the codex-five bootstrap: nested
    /// `SaveLayer`s with `keep_content: true` DropShadow filters must compound
    /// — the inner layer's output (shadow 1 + content) is the input of the
    /// outer shadow filter, so BOTH shadow lobes appear around the body. A
    /// regression that dropped the inner output (or kept `drop_shadow_only`)
    /// would lose one lobe.
    #[test]
    fn nested_keep_content_shadow_layers_compound() {
        use skia_safe::surfaces;

        fn keep_shadow_paint(dx: f32, dy: f32, sigma: f32) -> PaintSpec {
            PaintSpec {
                fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
                style: PaintStyle::Fill,
                anti_alias: true,
                blend_mode: opencat_core::canvas::paint::BlendMode::SrcOver,
                image_filter: Some(ImageFilterSpec::DropShadow {
                    dx,
                    dy,
                    sigma_x: sigma,
                    sigma_y: sigma,
                    color: [0.0, 0.0, 0.0, 1.0],
                    keep_content: true,
                }),
                ..Default::default()
            }
        }

        // 64x48 white canvas; content = small opaque square in the middle.
        // Shadow A offset (+24, 0), shadow B offset (-24, 0), sigma 4 each.
        let mut frame = DrawOpFrame::default();
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 1.0, 1.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.paints.push(keep_shadow_paint(24.0, 0.0, 4.0));
        frame.paints.push(keep_shadow_paint(-24.0, 0.0, 4.0));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.2, 0.2, 0.2, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 { x: 0.0, y: 0.0, width: 64.0, height: 48.0 },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });
        frame.ops.push(DrawOp::SaveLayer {
            bounds: None,
            paint: Some(opencat_core::ir::draw_types::PaintId(1)),
            alpha: 1.0,
            backdrop: false,
        });
        frame.ops.push(DrawOp::SaveLayer {
            bounds: None,
            paint: Some(opencat_core::ir::draw_types::PaintId(2)),
            alpha: 1.0,
            backdrop: false,
        });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 { x: 28.0, y: 18.0, width: 8.0, height: 12.0 },
            paint: opencat_core::ir::draw_types::PaintId(3),
        });
        frame.ops.push(DrawOp::Restore);
        frame.ops.push(DrawOp::Restore);

        let media = EnginePreparedFrameMedia::default();
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((64, 48)).expect("surface");
        let canvas = surface.canvas();
        for op in &frame.ops {
            replay_op(&mut exec, canvas, &frame, &media, op).expect("shadow chain replay");
        }
        let image = surface.image_snapshot();
        let info = ImageInfo::new((64, 48), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 64 * 48 * 4];
        assert!(image.read_pixels(&info, rgba.as_mut_slice(), 64 * 4, (0, 0), CachingHint::Allow));

        fn luminance(rgba: &[u8], w: usize, x: usize, y: usize) -> u8 {
            let i = (y * w + x) * 4;
            rgba[i]
        }
        // Content square itself.
        let body = luminance(&rgba, 64, 32, 24);
        assert!(body < 80, "body square must be drawn, got {body}");
        // Right lobe (shadow A at +24): x≈58, y=24 must be darkened.
        let right = luminance(&rgba, 64, 57, 24);
        assert!(right < 220, "chained outer shadow lobe (+24) missing, got {right}");
        // Left lobe (shadow B at -24): x≈6, y=24 must be darkened — this only
        // happens when the inner keep_content output feeds the outer filter.
        let left = luminance(&rgba, 64, 6, 24);
        assert!(left < 220, "chained inner shadow lobe (-24) missing, got {left}");
    }

    /// lambda → SKSL → RuntimeEffect 的 raster 端到端：水平渐变亮度递增，
    /// 且 u_oc_rect 偏移生效（dst 从 (4,2) 起绘制，内部坐标仍从 0 数起）。
    #[test]
    fn lambda_sksl_gradient_raster_e2e() {
        let source = "(uv, u) => { const g = uv.x / u.w; return [g, g, g, 1]; }";
        let mut spec = opencat_liangyi::EffectSpec::default();
        spec.uniforms
            .push(("w".to_string(), opencat_liangyi::program::Ty::Float));
        let compiled =
            opencat_liangyi::compile_effect(source, &spec)
                .expect("lambda should compile");
        assert_eq!(
            compiled.backend,
            opencat_liangyi::Backend::Sksl
        );
        let sksl = compiled.sksl.expect("sksl backend carries code");

        let rt = RuntimeEffect::make_for_shader(&sksl, None).expect("generated SKSL should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef { hash: compiled.hash, sksl: sksl.clone() });
        // uniforms: w=8.0 f32，随后 u_oc_rect = dst(0,0,8,8)
        let mut uniform_bytes = Vec::new();
        for v in [8.0f32, 0.0, 0.0, 8.0, 8.0] {
            uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        frame.bytes.extend_from_slice(&uniform_bytes);
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: uniform_bytes.len() as u32 });
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 { x: 0.0, y: 0.0, width: 8.0, height: 8.0 },
        });

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt.into()],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface");
        let canvas = surface.canvas();
        replay_op(
            &mut exec,
            canvas,
            &frame,
            &media,
            &frame.ops[0],
        )
        .expect("lambda runtime effect should replay");

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 8 * 8 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            8 * 4,
            (0, 0),
            CachingHint::Allow,
        ));
        // 水平亮度单调递增（premul 但 alpha=1）；byte index = col*4（row 0）
        let lum = |col: usize| rgba[col * 4];
        assert!(lum(0) < lum(3) && lum(3) < lum(6), "gradient not increasing: {} {} {}", lum(0), lum(3), lum(6));
    }

    /// 后端一致性：同一 float-only lambda，SKSL（skia raster）与 f64 解释器
    /// 输出逐通道容差比对。
    #[test]
    fn lambda_backend_consistency_sksl_vs_interpreter() {
        let source =
            "(uv, u) => { const g = smoothstep(0.0, u.w, uv.x); return [g, 0.5, 1 - g, 1]; }";
        let mut spec = opencat_liangyi::EffectSpec::default();
        spec.uniforms
            .push(("w".to_string(), opencat_liangyi::program::Ty::Float));

        // CPU（解释器）
        let mut cpu_spec = spec.clone();
        cpu_spec.backend_override =
            Some(opencat_liangyi::Backend::Cpu);
        let cpu = opencat_liangyi::compile_effect(source, &cpu_spec)
            .expect("cpu compile");
        let ctx = opencat_liangyi::interp::InterpCtx {
            uniforms: &[opencat_liangyi::program::Val::F(8.0)],
            rect: [0.0, 0.0, 8.0, 8.0],
            children: &[],
            scan: None,
        };
        let cpu_rgba =
            opencat_liangyi::interp::render(&cpu.program, 8, 8, &ctx);

        // SKSL（skia raster）
        let mut sksl_spec = spec.clone();
        sksl_spec.backend_override =
            Some(opencat_liangyi::Backend::Sksl);
        let sksl_c = opencat_liangyi::compile_effect(source, &sksl_spec)
            .expect("sksl compile");
        let rt = RuntimeEffect::make_for_shader(sksl_c.sksl.as_deref().unwrap(), None)
            .expect("generated SKSL should compile");
        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef { hash: sksl_c.hash, sksl: sksl_c.sksl.clone().unwrap() });
        let mut uniform_bytes = Vec::new();
        for v in [8.0f32, 0.0, 0.0, 8.0, 8.0] {
            uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        frame.bytes.extend_from_slice(&uniform_bytes);
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: uniform_bytes.len() as u32 });
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 { x: 0.0, y: 0.0, width: 8.0, height: 8.0 },
        });
        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt.into()],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface");
        replay_op(&mut exec, surface.canvas(), &frame, &media, &frame.ops[0]).expect("replay");
        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut sksl_rgba = vec![0_u8; 8 * 8 * 4];
        assert!(image.read_pixels(
            &image_info,
            sksl_rgba.as_mut_slice(),
            8 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        for i in 0..8 * 8 * 4 {
            let (a, b) = (cpu_rgba[i], sksl_rgba[i]);
            assert!(
                (a as i32 - b as i32).abs() <= 3,
                "px {i}: cpu {a} vs sksl {b}"
            );
        }
    }

    /// lambda → SKSL 的 generated image child：child.eval(uv) 以 dst 本地
    /// 坐标取像素（tile 原点 = dst 原点）。
    #[test]
    fn lambda_sksl_generated_child_samples_local_space() {
        use opencat_core::ir::GeneratedImageId;

        let source =
            "(uv, tex, u) => { const c = tex.eval(uv); return [c.r, c.g, c.b, 1]; }";
        let mut spec = opencat_liangyi::EffectSpec::default();
        let compiled =
            opencat_liangyi::compile_effect(source, &spec)
                .expect("lambda should compile");
        let sksl = compiled.sksl.expect("sksl");
        let rt = RuntimeEffect::make_for_shader(&sksl, None).expect("compile");

        // 2×1 child：红、绿两像素（straight）
        let child_rgba: Vec<u8> = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let gid = GeneratedImageId(0x1A2B);
        let child_info =
            ImageInfo::new((2, 1), ColorType::RGBA8888, AlphaType::Unpremul, None);
        let child_img = images::raster_from_data(
            &child_info,
            Data::new_copy(&child_rgba),
            2 * 4,
        )
        .expect("child image should decode");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef { hash: compiled.hash, sksl });
        frame.children.push(RuntimeEffectChildRef::Image(ImageRef::Generated { id: gid }));
        // u_oc_rect = dst(0,0,2,1)（无用户 uniform）
        let mut uniform_bytes = Vec::new();
        for v in [0.0f32, 0.0, 2.0, 1.0] {
            uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        frame.bytes.extend_from_slice(&uniform_bytes);
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: uniform_bytes.len() as u32 });
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 { x: 0.0, y: 0.0, width: 2.0, height: 1.0 },
        });

        let media = EnginePreparedFrameMedia {
            images: vec![child_img],
            image_index: {
                let mut m = std::collections::HashMap::new();
                m.insert(ImageRef::Generated { id: gid }, 0usize);
                m
            },
            runtime_effects: vec![rt.into()],
            image_scales: Vec::new(),
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((2, 1)).expect("surface");
        replay_op(&mut exec, surface.canvas(), &frame, &media, &frame.ops[0]).expect("replay");
        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((2, 1), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 2 * 1 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            2 * 4,
            (0, 0),
            CachingHint::Allow,
        ));
        assert_eq!(rgba[0..3].to_vec(), vec![255, 0, 0], "left = red");
        assert_eq!(rgba[4..7].to_vec(), vec![0, 255, 0], "right = green");
    }
}
