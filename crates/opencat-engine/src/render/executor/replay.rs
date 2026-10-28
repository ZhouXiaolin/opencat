use super::paint::{build_skia_image_filter, paint_from_spec};
use super::path::path_from_encoded;
use super::{DrawError, DrawStats};
use super::{EngineDrawExecutor, EnginePreparedFrameMedia};
use opencat_core::canvas::paint::{StrokeCap, StrokeJoin, StrokeSpec};
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_op::{DRRectSpec, Radii4};
use opencat_core::ir::draw_op::{DrawOp, LineCap as OpLineCap, LineJoin as OpLineJoin, PointMode};
use opencat_core::ir::draw_types::{DrawOpRange, ImageRef, PathOp, RuntimeEffectChildRef, SubtreeId};
use skia_safe::{
    AlphaType, Canvas, ColorType, CubicResampler, Data, FilterMode, Image, ImageInfo, Matrix,
    MipmapMode, Paint, PathBuilder, Picture, PictureRecorder, Point, RRect, Rect, SamplingOptions,
    Shader, TileMode, Vector, surfaces,
};

/// feImage 检测：filter 链里有 feImage primitive（需子树预渲染）。
fn filter_chain_has_image(spec: &opencat_core::canvas::paint::ImageFilterSpec) -> bool {
    use opencat_core::canvas::paint::{FilterPrimKind, ImageFilterSpec};
    match spec {
        ImageFilterSpec::Chain(chains) => chains.iter().any(|c| {
            c.primitives
                .iter()
                .any(|p| matches!(p.kind, FilterPrimKind::Image { .. }))
        }),
        _ => false,
    }
}

/// feImage 的子树离屏光栅化：引用内容按文档 user space 像素化进
/// region 大小的图（平移 -region.xy，不套组 CTM——filter 树坐标即
/// user space，后续 1:1 放回 region 原点）。同 mask 子树 replay：期间
/// 屏蔽组 mask 的 DstIn 副作用、组 alpha 归一。
fn render_filter_images(
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    spec: &opencat_core::canvas::paint::ImageFilterSpec,
) -> std::collections::HashMap<u32, Image> {
    use opencat_core::canvas::paint::{FilterPrimKind, ImageFilterSpec};
    let mut jobs: Vec<(u32, Rect, Rect)> = Vec::new();
    if let ImageFilterSpec::Chain(chains) = spec {
        for chain in chains {
            let Some(region) = chain.source_crop else {
                continue;
            };
            let sk_region = Rect::new(
                region.x0 as f32,
                region.y0 as f32,
                region.x1 as f32,
                region.y1 as f32,
            );
            for p in &chain.primitives {
                if let FilterPrimKind::Image { subtree } = &p.kind {
                    if !jobs.iter().any(|(s, _, _)| s == subtree) {
                        // subregion 缺省时退化为 region 本身（平移 0）。
                        let sub = p.crop.unwrap_or(region);
                        jobs.push((
                            *subtree,
                            sk_region,
                            Rect::new(sub.x0 as f32, sub.y0 as f32, sub.x1 as f32, sub.y1 as f32),
                        ));
                    }
                }
            }
        }
    }

    let mut out = std::collections::HashMap::new();
    for (subtree, region, sub) in jobs {
        let w = region.width().ceil() as i32;
        let h = region.height().ceil() as i32;
        if w <= 0 || h <= 0 {
            continue;
        }
        let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
        let Some(mut surface) = surfaces::raster(&info, None, None) else {
            continue;
        };
        {
            let c = surface.canvas();
            // 内容画到 subregion 起点（resvg apply_image 的放置：内容点 p →
            // sub.xy + scale·p；无旋转组下 scale 与 CTM 相消为 1）。旋转组的
            // scale 分量近似实测更差（resvg 的放置在设备空间，user space 里
            // 单乘 scale 不等价），先不模拟。
            c.translate((sub.left - region.left, sub.top - region.top));
            let saved_alpha = exec.current_alpha;
            exec.current_alpha = 1.0;
            exec.mask_replay += 1;
            let r = replay_subtree(exec, c, draw, media, SubtreeId(subtree));
            exec.mask_replay -= 1;
            exec.current_alpha = saved_alpha;
            if r.is_err() {
                continue;
            }
        }
        out.insert(subtree, surface.image_snapshot());
    }
    out
}

fn apply_global_alpha(paint: &mut Paint, alpha: f32) {
    let a = (paint.color().a() as f32 / 255.0) * alpha;
    paint.set_alpha((a * 255.0).round() as u8);
}

// ---------------------------------------------------------------------------
// svg 结构原语的 Skia 原生实现（pattern / mask）：usvg 解析、DrawOp 协议、
// skia 单一执行端，无第三方光栅化器。
// ---------------------------------------------------------------------------

/// `DrawOp::FillPattern`/`StrokePattern`：tile 子树按设备密度（tile 尺寸 ×
/// (CTM ∘ patternTransform) 的 scale 分量）在 skia 离屏重放成 tile 图，再以
/// Repeat image shader 平铺出路径填充/描边。shader 局部矩阵即 SVG 的 pattern
/// 坐标映射：texel → scale(1/sx,1/sy) → translate(tile.xy) → patternTransform
/// → user space。返回 Ok(false) 表示 shader 不可得，调用方按跳过处理。
#[allow(clippy::too_many_arguments)]
fn fill_pattern_skia(
    canvas: &Canvas,
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    encoded: &opencat_core::ir::draw_types::EncodedPath,
    subtree: SubtreeId,
    tile: &opencat_core::ir::draw_op::Rect4,
    pattern_transform: &[f32; 6],
    opacity: f32,
    stroke: Option<&StrokeSpec>,
) -> Result<bool, DrawError> {
    if opacity <= 0.0 {
        return Ok(true);
    }
    let ctm = canvas.local_to_device_as_3x3();
    let pt = Matrix::from_affine(pattern_transform);
    let combined = Matrix::concat(&ctm, &pt);
    let sx = combined.scale_x().hypot(combined.skew_x());
    let sy = combined.skew_y().hypot(combined.scale_y());
    if !(sx > 0.0) || !(sy > 0.0) {
        return Ok(true);
    }
    let tile_w = (tile.width * sx).round() as i32;
    let tile_h = (tile.height * sy).round() as i32;
    if tile_w <= 0 || tile_h <= 0 {
        return Ok(true);
    }

    // tile 内容按设备像素密度重放（内容内嵌的 DrawPath 同一执行链）。
    // pattern 内容的 alpha 与引用处透明度无关（opacity 在 shader 上）。
    let info = ImageInfo::new((tile_w, tile_h), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(mut surface) = surfaces::raster(&info, None, None) else {
        return Ok(false);
    };
    {
        let tile_canvas = surface.canvas();
        tile_canvas.scale((sx, sy));
        let saved_alpha = exec.current_alpha;
        exec.current_alpha = 1.0;
        exec.mask_replay += 1;
        let replay = replay_subtree(exec, tile_canvas, draw, media, subtree);
        exec.mask_replay -= 1;
        exec.current_alpha = saved_alpha;
        replay?;
    }
    let tile_image = surface.image_snapshot();

    let mut shader_lm = pt;
    shader_lm.pre_translate((tile.x, tile.y));
    shader_lm.pre_scale((1.0 / sx, 1.0 / sy), None);
    let Some(shader) = tile_image.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions {
            use_cubic: true,
            cubic: CubicResampler::catmull_rom(),
            ..SamplingOptions::default()
        },
        Some(&shader_lm),
    ) else {
        return Ok(false);
    };

    let sk_path = path_from_encoded(encoded);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_shader(shader);
    paint.set_alpha((opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
    match stroke {
        Some(s) => {
            paint.set_style(skia_safe::paint::Style::Stroke);
            paint.set_stroke_width(s.width);
            paint.set_stroke_miter(s.miter_limit);
            paint.set_stroke_cap(match s.cap {
                StrokeCap::Butt => skia_safe::paint::Cap::Butt,
                StrokeCap::Round => skia_safe::paint::Cap::Round,
                StrokeCap::Square => skia_safe::paint::Cap::Square,
            });
            paint.set_stroke_join(match s.join {
                StrokeJoin::Miter => skia_safe::paint::Join::Miter,
                StrokeJoin::Round => skia_safe::paint::Join::Round,
                StrokeJoin::Bevel => skia_safe::paint::Join::Bevel,
            });
        }
        None => {
            paint.set_style(skia_safe::paint::Style::Fill);
        }
    }
    canvas.draw_path(&sk_path, &paint);
    Ok(true)
}

/// `DrawOp::MaskClip`：mask 内容子树在 skia 全帧离屏上重放（套当时的组
/// CTM，AA clip 到 mask region），CPU 按 SVG spec 折算成单通道 coverage
/// （luminance：premul 域直算 `0.2126R+0.7152G+0.0722B`——premul 的 rgb 已含
/// ×alpha 项，与 spec 的 luma(unpremul)×alpha 相等；alpha：直取），region 外
/// 为 0。coverage 入 `mask_stack`，所在层 Restore 前以 DstIn blend 罩回
/// （`mask_apply_dst_in`）。
#[allow(clippy::too_many_arguments)]
fn mask_push_skia(
    canvas: &Canvas,
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    subtree: SubtreeId,
    kind: u8,
    region: &opencat_core::ir::draw_op::Rect4,
) -> Result<(), DrawError> {
    let m = canvas.local_to_device_as_3x3();
    let size = canvas.image_info().dimensions();
    let (fw, fh) = (size.width, size.height);
    if fw <= 0 || fh <= 0 {
        return Ok(());
    }
    let mut cov = vec![0_u8; (fw * fh) as usize];

    // 退化 region（面积 0）：mask 全域不可见 = 全剪，全 0 coverage 即可。
    let corners = [
        Point::new(region.x, region.y),
        Point::new(region.x + region.width, region.y),
        Point::new(region.x, region.y + region.height),
        Point::new(region.x + region.width, region.y + region.height),
    ];
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in corners {
        let q = m.map_point(p);
        bx0 = bx0.min(q.x);
        by0 = by0.min(q.y);
        bx1 = bx1.max(q.x);
        by1 = by1.max(q.y);
    }
    let empty_region = (bx1 - bx0) <= 0.0 || (by1 - by0) <= 0.0;

    if !empty_region {
        let info = ImageInfo::new((fw, fh), ColorType::RGBA8888, AlphaType::Premul, None);
        let Some(mut surface) = surfaces::raster(&info, None, None) else {
            return Ok(());
        };
        {
            let mask_canvas = surface.canvas();
            mask_canvas.concat(&m);
            mask_canvas.clip_rect(
                Rect::new(region.x, region.y, region.x + region.width, region.y + region.height),
                Some(skia_safe::ClipOp::Intersect),
                true,
            );
            let saved_alpha = exec.current_alpha;
            exec.current_alpha = 1.0;
            exec.mask_replay += 1;
            let replay = replay_subtree(exec, mask_canvas, draw, media, subtree);
            exec.mask_replay -= 1;
            exec.current_alpha = saved_alpha;
            replay?;
        }
        let mut pix = vec![0_u8; info.compute_min_byte_size()];
        if !surface.read_pixels(&info, &mut pix, info.min_row_bytes(), (0, 0)) {
            return Ok(());
        }

        for (dst, px) in cov.iter_mut().zip(pix.chunks_exact(4)) {
            let (r, g, b, a) = (px[0] as f32, px[1] as f32, px[2] as f32, px[3] as f32);
            let v = if kind == 1 {
                a
            } else {
                (r * 0.2126 + g * 0.7152 + b * 0.0722).round().min(255.0)
            };
            *dst = v as u8;
        }
    }

    // coverage 装进全帧 premul Image（rgb=0、a=coverage），Restore 前 DstIn。
    let mut buf = vec![0_u8; (fw * fh) as usize * 4];
    for (px, &c) in buf.chunks_exact_mut(4).zip(cov.iter()) {
        px[3] = c;
    }
    let minfo = ImageInfo::new((fw, fh), ColorType::RGBA8888, AlphaType::Premul, None);
    if let Some(img) = skia_safe::images::raster_from_data(&minfo, Data::new_copy(&buf), fw as usize * 4)
    {
        exec.mask_stack.push(img);
    }
    Ok(())
}

/// 层 Restore 前，把该层期间入栈的 mask coverage 以 DstIn blend 画回层上。
/// 全程 canvas API 无像素拷贝。嵌套 mask：多张 coverage 依次 DstIn = 亮度
/// 相乘，语义正确。
fn mask_apply_dst_in(canvas: &Canvas, exec: &mut EngineDrawExecutor) {
    let Some(mark) = exec.layer_marks.pop() else {
        return;
    };
    if exec.mask_stack.len() <= mark {
        return;
    }
    let mut paint = Paint::default();
    paint.set_blend_mode(skia_safe::BlendMode::DstIn);
    paint.set_anti_alias(false);
    canvas.save();
    canvas.reset_matrix();
    for img in exec.mask_stack.drain(mark..) {
        canvas.draw_image_with_sampling_options(
            &img,
            (0.0, 0.0),
            SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
            Some(&paint),
        );
    }
    canvas.restore();
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

pub(crate) fn replay_subtree(
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
            // svg `<mask>`：层内容完成后、弹层前，把该层期间入栈的 coverage
            // 以 DstIn blend 罩回（全程 canvas API 无像素拷贝）。多余的
            // Restore 不弹记录。
            if exec.mask_replay == 0 {
                mask_apply_dst_in(canvas, exec);
            }
            canvas.restore();
            Ok(())
        }
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
            backdrop,
        } => {
            if exec.mask_replay == 0 {
                exec.layer_marks.push(exec.mask_stack.len());
            }
            let sk_rect = bounds.map(|r| Rect::new(r.x, r.y, r.x + r.width, r.y + r.height));
            match paint {
                Some(pid) => {
                    // The layer alpha must compose with the paint (it used to
                    // be dropped whenever a paint was present, which made
                    // filtered layers render fully opaque).
            let spec = &draw.paints[pid.0 as usize];
            let mut sk_paint = paint_from_spec(spec);
            // feImage：引用子树先离屏光栅化成图（paint_from_spec 内部构建
            // filter 时图还没就位，这里显式重建并覆盖）。
            if spec.image_filter.as_ref().is_some_and(filter_chain_has_image) {
                let images = render_filter_images(
                    exec,
                    draw,
                    media,
                    spec.image_filter.as_ref().unwrap(),
                );
                super::paint::set_filter_images(images);
                let imf = spec
                    .image_filter
                    .as_ref()
                    .and_then(build_skia_image_filter);
                sk_paint.set_image_filter(imf);
                super::paint::clear_filter_images();
            }
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
        DrawOp::SaveBackdropLayer { bounds, radius, paint } => {
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
                    // CSS 裁剪 backdrop-filter 到圆角 border box：任一角有
                    // 半径就用 RRect，否则退回方角 clip。
                    let rect = Rect::new(r.left, r.top, r.right, r.bottom);
                    if radius.iter().any(|rad| *rad > 0.0) {
                        let rrect = RRect::new_rect_radii(
                            rect,
                            &[
                                Point::new(radius[0], radius[0]),
                                Point::new(radius[1], radius[1]),
                                Point::new(radius[2], radius[2]),
                                Point::new(radius[3], radius[3]),
                            ],
                        );
                        canvas.clip_rrect(rrect, skia_safe::ClipOp::Intersect, true);
                    } else {
                        canvas.clip_rect(rect, skia_safe::ClipOp::Intersect, false);
                    }
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
        DrawOp::ClipPath { anti_alias, fill_type } => {
            if let Some(mut pb) = exec.current_path.take() {
                let mut path = pb.detach();
                path.set_fill_type(match fill_type {
                    opencat_core::ir::draw_types::FillType::Winding => {
                        skia_safe::PathFillType::Winding
                    }
                    opencat_core::ir::draw_types::FillType::EvenOdd => {
                        skia_safe::PathFillType::EvenOdd
                    }
                });
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
            let encoded = &draw.paths[path.0 as usize];
            let spec = &draw.paints[pid.0 as usize];
            let sk_path = path_from_encoded(encoded);
            let mut paint = paint_from_spec(spec);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_path(&sk_path, &paint);
            Ok(())
        }
        DrawOp::FillPattern {
            path,
            subtree,
            tile,
            pattern_transform,
            opacity,
        } => {
            // svg `<pattern>`：tile 子树离屏 → Repeat image shader 平铺填充。
            let encoded = &draw.paths[path.0 as usize];
            fill_pattern_skia(
                canvas, exec, draw, media, encoded, *subtree, tile, pattern_transform, *opacity,
                None,
            )?;
            Ok(())
        }
        DrawOp::StrokePattern {
            path,
            subtree,
            tile,
            pattern_transform,
            opacity,
            stroke,
        } => {
            // svg `<pattern>` 描边：同 FillPattern，平铺结果沿描边样式。
            let encoded = &draw.paths[path.0 as usize];
            fill_pattern_skia(
                canvas, exec, draw, media, encoded, *subtree, tile, pattern_transform, *opacity,
                Some(stroke),
            )?;
            Ok(())
        }
        DrawOp::MaskClip {
            subtree,
            kind,
            region,
        } => {
            // svg `<mask>`：coverage 折算入栈；所在层 Restore 前以 DstIn 罩回。
            mask_push_skia(canvas, exec, draw, media, *subtree, *kind, region)
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
                        ImageRef::Encoded { .. } => {
                            // svg `<image>` 高质量重采样：放大缩小都走 Mitchell
                            // cubic、不落 mipmap。1:1 blit 时 cubic 退化为精确
                            // 拷贝，与 VideoFrame 同款走 Nearest。
                            let src_size = (sk_image.width() as f32, sk_image.height() as f32);
                            if is_identity_blit(canvas, src_size, src_rect.as_ref(), &dst_rect) {
                                SamplingOptions::new(FilterMode::Nearest, MipmapMode::None)
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


