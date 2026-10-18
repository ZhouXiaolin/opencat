//! tiny-skia 路径光栅化（SVG 对齐工程，docs/svg-align/PLAN.md）。
//!
//! diff 参考是 resvg = tiny-skia 扫描线光栅化（hairline 特判 + 4 子扫描线，
//! 细斜线边缘覆盖率与几何面积差 ±20%）；Skia 解析式 AA 是几何精确的，
//! 两者在每个形状边缘像素上都差 9~75/255。`OC_PATH_RASTERIZER=tiny` 时
//! DrawPath 走 tiny-skia（与 resvg 同一 crate 同一版本）离屏渲染，再以
//! 原 blend 模式合成回 skia canvas——形状边缘与参考逐像素同源。
//! 默认关闭：JSX 侧的对齐参考是 Chrome，保持 Skia AA。

use std::sync::OnceLock;

use skia_safe::{
    AlphaType, Canvas, ColorType, Data, FilterMode, ImageInfo, MipmapMode, Paint, SamplingOptions,
    images,
};
use tiny_skia as ts;

use opencat_core::canvas::paint::{
    BlendMode, FillSpec, PaintSpec, PaintStyle, PathEffectSpec, StrokeCap, StrokeJoin,
};
use opencat_core::ir::draw_types::{EncodedPath, FillType, PathOp};

pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("OC_PATH_RASTERIZER").as_deref() == Ok("tiny"))
}

fn tiny_path(ops: &[PathOp]) -> Option<ts::Path> {
    let mut pb = ts::PathBuilder::new();
    for op in ops {
        match *op {
            PathOp::MoveTo { x, y } => pb.move_to(x, y),
            PathOp::LineTo { x, y } => pb.line_to(x, y),
            PathOp::QuadTo { cx, cy, x, y } => pb.quad_to(cx, cy, x, y),
            PathOp::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => pb.cubic_to(c1x, c1y, c2x, c2y, x, y),
            PathOp::Close => pb.close(),
            PathOp::AddRect { x, y, width, height } => {
                pb.push_rect(ts::Rect::from_xywh(x, y, width, height)?);
            }
            // canvas rrect/arc 只有 JS 侧会发；svg 后端不产生。遇到则
            // 不接管（回退 Skia），避免第二套圆角/弧线近似。
            PathOp::AddRRect { .. } => return None,
            PathOp::AddOval { x, y, width, height } => {
                pb.push_oval(ts::Rect::from_xywh(x, y, width, height)?);
            }
            PathOp::AddArc { .. } => return None,
        }
    }
    pb.finish()
}

/// 尝试用 tiny-skia 画一条路径。返回 false 表示未接管（调用方回退 Skia）。
pub fn draw_path_tiny(
    canvas: &Canvas,
    encoded: &EncodedPath,
    spec: &PaintSpec,
    global_alpha: f32,
) -> bool {
    if !enabled() {
        return false;
    }
    // P1 只接管纯色；渐变/图案路径仍走 Skia（P2 接 tiny shader）。
    let FillSpec::Solid(rgba) = spec.fill else {
        return false;
    };
    let Some(path) = tiny_path(&encoded.ops) else {
        return false;
    };
    // Stroke 样式必须带 stroke 参数（协议约定）；缺失则不接管。
    if matches!(spec.style, PaintStyle::Stroke) && spec.stroke.is_none() {
        return false;
    }

    // 设备空间 CTM（DrawOp 流的坐标系 = canvas 局部空间）。
    let m = canvas.local_to_device_as_3x3();
    let Some([sx, kx, tx, ky, sy, ty]) = m.to_affine() else {
        return false;
    };
    let ctm = ts::Transform::from_row(sx, kx, tx, ky, sy, ty);

    // 离屏范围 = 路径包围盒过 CTM，外扩 stroke 半宽（含 miter 尖角）与
    // AA 裙边；再与设备裁剪盒求交，跳过空区。
    let b = path.bounds();
    let mut corners = [
        ts::Point::from_xy(b.left(), b.top()),
        ts::Point::from_xy(b.right(), b.top()),
        ts::Point::from_xy(b.left(), b.bottom()),
        ts::Point::from_xy(b.right(), b.bottom()),
    ];
    ctm.map_points(&mut corners);
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &corners {
        bx0 = bx0.min(p.x);
        by0 = by0.min(p.y);
        bx1 = bx1.max(p.x);
        by1 = by1.max(p.y);
    }
    let max_scale = sx.abs().max(sy.abs()).max(kx.abs()).max(ky.abs());
    let pad = match spec.style {
        PaintStyle::Stroke => {
            let s = spec.stroke.as_ref().unwrap();
            let miter = if s.join == StrokeJoin::Miter {
                s.miter_limit
            } else {
                1.5
            };
            (s.width / 2.0) * max_scale * miter + 1.5
        }
        PaintStyle::Fill => 1.5,
    };
    let mut x0 = (bx0 - pad).floor() as i32;
    let mut y0 = (by0 - pad).floor() as i32;
    let mut x1 = (bx1 + pad).ceil() as i32;
    let mut y1 = (by1 + pad).ceil() as i32;
    if let Some(clip) = canvas.device_clip_bounds() {
        x0 = x0.max(clip.left());
        y0 = y0.max(clip.top());
        x1 = x1.min(clip.right());
        y1 = y1.min(clip.bottom());
    }
    let (w, h) = ((x1 - x0).max(0), (y1 - y0).max(0));
    if std::env::var("OC_PATH_DEBUG").is_ok() {
        eprintln!(
            "[tiny] style={:?} local_bounds=({},{})-({},{}) ctm=({sx},{kx},{tx},{ky},{sy},{ty}) dev=({bx0:.1},{by0:.1})-({bx1:.1},{by1:.1}) pix=({x0},{y0})+({w}x{h}) clip={:?}",
            spec.style, b.left(), b.top(), b.right(), b.bottom(),
            canvas.device_clip_bounds(),
        );
    }
    if w == 0 || h == 0 {
        return true;
    }

    let mut paint = ts::Paint::default();
    paint.anti_alias = spec.anti_alias;
    paint.set_color_rgba8(
        (rgba[0] * 255.0).round() as u8,
        (rgba[1] * 255.0).round() as u8,
        (rgba[2] * 255.0).round() as u8,
        ((rgba[3] * global_alpha).clamp(0.0, 1.0) * 255.0).round() as u8,
    );

    // 设备像素坐标 = ctm(p) - (x0,y0)：平移在后（device 空间），缩放/旋转
    // 在前。concat(a,b)=a∘b（b 先作用），故 translate 是 pre_concat 的 self。
    let local_ts = ts::Transform::from_translate(-(x0 as f32), -(y0 as f32)).pre_concat(ctm);
    let Some(mut pixmap) = ts::Pixmap::new(w as u32, h as u32) else {
        return false;
    };
    match spec.style {
        PaintStyle::Fill => {
            let rule = match encoded.fill_type {
                FillType::Winding => ts::FillRule::Winding,
                FillType::EvenOdd => ts::FillRule::EvenOdd,
            };
            pixmap.fill_path(&path, &paint, rule, local_ts, None);
        }
        PaintStyle::Stroke => {
            let s = spec.stroke.as_ref().unwrap();
            let mut st = ts::Stroke {
                width: s.width,
                miter_limit: s.miter_limit,
                line_cap: match s.cap {
                    StrokeCap::Butt => ts::LineCap::Butt,
                    StrokeCap::Round => ts::LineCap::Round,
                    StrokeCap::Square => ts::LineCap::Square,
                },
                line_join: match s.join {
                    StrokeJoin::Miter => ts::LineJoin::Miter,
                    StrokeJoin::Round => ts::LineJoin::Round,
                    StrokeJoin::Bevel => ts::LineJoin::Bevel,
                },
                dash: None,
            };
            if let Some(PathEffectSpec::Dash { intervals, phase }) = &spec.path_effect {
                st.dash = ts::StrokeDash::new(intervals.clone(), *phase);
            }
            pixmap.stroke_path(&path, &paint, &st, local_ts, None);
        }
    }

    // tiny-skia 内存即 premul RGBA8888，与 skia 同布局，零转换上传。
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(image) = images::raster_from_data(&info, Data::new_copy(pixmap.data()), w as usize * 4)
    else {
        return false;
    };
    let mut draw_paint = Paint::default();
    draw_paint.set_alpha(255);
    draw_paint.set_blend_mode(convert_blend_mode(spec.blend_mode));
    // pixmap 是设备空间的，draw_image 的坐标却会被当前 CTM 再变换一次
    // （组平移会让内容整体偏移，背景色 case 的右下四分之一即是此因）。
    // 在恒等矩阵下 blit。
    canvas.save();
    canvas.reset_matrix();
    canvas.draw_image_with_sampling_options(
        image,
        (x0 as f32, y0 as f32),
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
        Some(&draw_paint),
    );
    canvas.restore();
    true
}

/// svg `<image>` 的 tiny Pattern 重采样：resvg `render_raster` 是
/// `fill_rect(自然尺寸矩形, Pattern(Pad, Bicubic, identity), image 变换)`，
/// 这里做同一件事——Pattern 重采样与参考同 crate 同版本。返回 false 表示
/// 未接管（调用方回退 Skia draw_image_rect）。
pub fn draw_image_tiny(
    canvas: &Canvas,
    pixmap: &ts::Pixmap,
    dst: &opencat_core::ir::draw_op::Rect4,
    global_alpha: f32,
) -> bool {
    if !enabled() || global_alpha < 0.999 {
        return false;
    }
    let m = canvas.local_to_device_as_3x3();
    let Some([sx, kx, tx, ky, sy, ty]) = m.to_affine() else {
        return false;
    };
    let ctm = ts::Transform::from_row(sx, kx, tx, ky, sy, ty);

    // Pad 模式边缘 clamp、paint 无 AA（resvg 同）：内容不出 rect，离屏只留
    // 1px 裙边防整数化误差。
    let mut corners = [
        ts::Point::from_xy(dst.x, dst.y),
        ts::Point::from_xy(dst.x + dst.width, dst.y),
        ts::Point::from_xy(dst.x, dst.y + dst.height),
        ts::Point::from_xy(dst.x + dst.width, dst.y + dst.height),
    ];
    ctm.map_points(&mut corners);
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &corners {
        bx0 = bx0.min(p.x);
        by0 = by0.min(p.y);
        bx1 = bx1.max(p.x);
        by1 = by1.max(p.y);
    }
    let mut x0 = (bx0 - 1.0).floor() as i32;
    let mut y0 = (by0 - 1.0).floor() as i32;
    let mut x1 = (bx1 + 1.0).ceil() as i32;
    let mut y1 = (by1 + 1.0).ceil() as i32;
    if let Some(clip) = canvas.device_clip_bounds() {
        x0 = x0.max(clip.left());
        y0 = y0.max(clip.top());
        x1 = x1.min(clip.right());
        y1 = y1.min(clip.bottom());
    }
    let (w, h) = ((x1 - x0).max(0), (y1 - y0).max(0));
    if w == 0 || h == 0 {
        return true;
    }

    let local_ts = ts::Transform::from_translate(-(x0 as f32), -(y0 as f32)).pre_concat(ctm);
    let Some(mut off) = ts::Pixmap::new(w as u32, h as u32) else {
        return false;
    };
    let shader = ts::Pattern::new(
        pixmap.as_ref(),
        ts::SpreadMode::Pad,
        ts::FilterQuality::Bicubic,
        1.0,
        ts::Transform::default(),
    );
    let mut paint = ts::Paint::default();
    paint.shader = shader;
    let Some(rect) = ts::Rect::from_xywh(dst.x, dst.y, dst.width, dst.height) else {
        return false;
    };
    off.fill_rect(rect, &paint, local_ts, None);

    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(image) =
        images::raster_from_data(&info, Data::new_copy(off.data()), w as usize * 4)
    else {
        return false;
    };
    let mut draw_paint = Paint::default();
    draw_paint.set_alpha(255);
    // pixmap 是设备空间的，draw_image 的坐标会被当前 CTM 再变换一次，在
    // 恒等矩阵下 blit（同 draw_path_tiny）。
    canvas.save();
    canvas.reset_matrix();
    canvas.draw_image_with_sampling_options(
        image,
        (x0 as f32, y0 as f32),
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
        Some(&draw_paint),
    );
    canvas.restore();
    true
}

/// clip 路径的 tiny coverage：离屏 AA fill 成覆盖率图，再以 clip Shader
/// （恒等矩阵下 = 设备空间）求交到 canvas。resvg 的 clip 形状就是同一
/// 扫描线光栅化器 fill 的（clip.rs → fill_path），边缘覆盖率逐像素同源；
/// skia 解析式 AA 的 clip 边缘与之差半格。失败返回 false，调用方回退
/// skia clip_path。只吃 svg 后端产生的 ops；空 clip 即不裁任何东西。
pub fn clip_path_tiny(canvas: &Canvas, ops: &[PathOp]) -> bool {
    if !enabled() || ops.is_empty() {
        return false;
    }
    let Some(path) = tiny_path(ops) else {
        return false;
    };
    let m = canvas.local_to_device_as_3x3();
    let Some([sx, kx, tx, ky, sy, ty]) = m.to_affine() else {
        return false;
    };
    let ctm = ts::Transform::from_row(sx, kx, tx, ky, sy, ty);

    let b = path.bounds();
    let mut corners = [
        ts::Point::from_xy(b.left(), b.top()),
        ts::Point::from_xy(b.right(), b.top()),
        ts::Point::from_xy(b.left(), b.bottom()),
        ts::Point::from_xy(b.right(), b.bottom()),
    ];
    ctm.map_points(&mut corners);
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &corners {
        bx0 = bx0.min(p.x);
        by0 = by0.min(p.y);
        bx1 = bx1.max(p.x);
        by1 = by1.max(p.y);
    }
    let mut x0 = (bx0 - 1.5).floor() as i32;
    let mut y0 = (by0 - 1.5).floor() as i32;
    let mut x1 = (bx1 + 1.5).ceil() as i32;
    let mut y1 = (by1 + 1.5).ceil() as i32;
    if let Some(clip) = canvas.device_clip_bounds() {
        x0 = x0.max(clip.left());
        y0 = y0.max(clip.top());
        x1 = x1.min(clip.right());
        y1 = y1.min(clip.bottom());
    }
    let (w, h) = ((x1 - x0).max(0), (y1 - y0).max(0));
    if w == 0 || h == 0 {
        // 与当前设备 clip 无交集：clip 之后什么也画不出来，恒等 clip 即可。
        return true;
    }

    let local_ts = ts::Transform::from_translate(-(x0 as f32), -(y0 as f32)).pre_concat(ctm);
    let Some(mut off) = ts::Pixmap::new(w as u32, h as u32) else {
        return false;
    };
    let mut paint = ts::Paint::default();
    paint.anti_alias = true;
    paint.set_color_rgba8(255, 255, 255, 255);
    off.fill_path(&path, &paint, ts::FillRule::Winding, local_ts, None);

    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(image) =
        images::raster_from_data(&info, Data::new_copy(off.data()), w as usize * 4)
    else {
        return false;
    };
    // shader 的采样空间 = 设置时刻的矩阵。coverage 图内容对应设备区域
    // (x0,y0) 起，image shader 从图 (0,0) 铺起，local matrix 必须补上这
    // 段平移；矩阵复位保证采样即设备像素。
    let mut lm = skia_safe::Matrix::default();
    lm.set_translate((x0 as f32, y0 as f32));
    let Some(shader) = image.to_shader(
        None,
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
        Some(&lm),
    ) else {
        return false;
    };
    // shader 的采样空间 = 设置时刻的矩阵。coverage 图已是设备像素，先把
    // 矩阵复位再挂 clip（clip 随状态栈存活，不受矩阵复位影响），随后恢复。
    let saved = canvas.local_to_device();
    canvas.reset_matrix();
    canvas.clip_shader(shader, skia_safe::ClipOp::Intersect);
    canvas.set_matrix(&saved);
    true
}

/// svg `<pattern>` 填充的 tiny 路径：resvg `render_pattern_pixmap` 的同式
/// 实现——tile 像素尺寸 = tile rect × (ctm ∘ patternTransform) 的缩放分量，
/// 内容子树（picture recording）在 skia 离屏上按 scale(sx,sy) 重放成 tile，
/// 再以 tiny Pattern（Repeat+Bicubic）平铺 fill。tile 渲染失败返回 Err；
/// 返回 Ok(false) 表示未接管（调用方回退）。
#[allow(clippy::too_many_arguments)]
pub fn fill_pattern_tiny(
    canvas: &Canvas,
    exec: &mut super::EngineDrawExecutor,
    draw: &opencat_core::ir::draw_frame::DrawOpFrame,
    media: &super::EnginePreparedFrameMedia,
    encoded: &EncodedPath,
    subtree: opencat_core::ir::draw_types::SubtreeId,
    tile: &opencat_core::ir::draw_op::Rect4,
    pattern_transform: &[f32; 6],
    opacity: f32,
) -> Result<bool, super::DrawError> {
    if !enabled() || opacity <= 0.0 {
        return Ok(false);
    }
    let Some(path) = tiny_path(&encoded.ops) else {
        return Ok(false);
    };
    let m = canvas.local_to_device_as_3x3();
    let Some([sx0, kx0, tx0, ky0, sy0, ty0]) = m.to_affine() else {
        return Ok(false);
    };
    let ctm = ts::Transform::from_row(sx0, kx0, tx0, ky0, sy0, ty0);
    let pt = ts::Transform::from_row(
        pattern_transform[0],
        pattern_transform[1],
        pattern_transform[2],
        pattern_transform[3],
        pattern_transform[4],
        pattern_transform[5],
    );

    // resvg：(transform ∘ pattern.transform).get_scale()。
    let (sx, sy) = ctm.pre_concat(pt).get_scale();
    if !(sx > 0.0) || !(sy > 0.0) {
        return Ok(false);
    }
    let tile_w = (tile.width * sx).round() as u32;
    let tile_h = (tile.height * sy).round() as u32;
    if tile_w == 0 || tile_h == 0 {
        return Ok(false);
    }

    // tile 内容：skia 离屏 + 子树重放（内容内嵌的 DrawPath 也会走 tiny）。
    // pattern 内容的 alpha 与引用处透明度无关（opacity 在 shader 上）。
    let info = ImageInfo::new(
        (tile_w as i32, tile_h as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let Some(mut surface) = skia_safe::surfaces::raster(&info, None, None) else {
        return Ok(false);
    };
    {
        let tile_canvas = surface.canvas();
        tile_canvas.scale((sx, sy));
        let saved_alpha = exec.current_alpha;
        exec.current_alpha = 1.0;
        let replay = crate::executor::replay::replay_subtree(exec, tile_canvas, draw, media, subtree);
        exec.current_alpha = saved_alpha;
        replay?;
    }
    let mut tile_pix = vec![0_u8; info.compute_min_byte_size()];
    if !surface.read_pixels(&info, &mut tile_pix, info.min_row_bytes(), (0, 0)) {
        return Ok(false);
    }
    let Some(tile_size) = ts::IntSize::from_wh(tile_w, tile_h) else {
        return Ok(false);
    };
    let Some(tile_pixmap) = ts::Pixmap::from_vec(tile_pix, tile_size) else {
        return Ok(false);
    };

    // resvg 的 shader 变换链：pt ∘ translate(rect.x,rect.y) ∘ scale(1/sx,1/sy)。
    let patt_ts = ts::Transform::default()
        .pre_concat(pt)
        .pre_translate(tile.x, tile.y)
        .pre_scale(1.0 / sx, 1.0 / sy);

    let b = path.bounds();
    let mut corners = [
        ts::Point::from_xy(b.left(), b.top()),
        ts::Point::from_xy(b.right(), b.top()),
        ts::Point::from_xy(b.left(), b.bottom()),
        ts::Point::from_xy(b.right(), b.bottom()),
    ];
    ctm.map_points(&mut corners);
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &corners {
        bx0 = bx0.min(p.x);
        by0 = by0.min(p.y);
        bx1 = bx1.max(p.x);
        by1 = by1.max(p.y);
    }
    let mut x0 = (bx0 - 1.5).floor() as i32;
    let mut y0 = (by0 - 1.5).floor() as i32;
    let mut x1 = (bx1 + 1.5).ceil() as i32;
    let mut y1 = (by1 + 1.5).ceil() as i32;
    if let Some(clip) = canvas.device_clip_bounds() {
        x0 = x0.max(clip.left());
        y0 = y0.max(clip.top());
        x1 = x1.min(clip.right());
        y1 = y1.min(clip.bottom());
    }
    let (w, h) = ((x1 - x0).max(0), (y1 - y0).max(0));
    if w == 0 || h == 0 {
        return Ok(true);
    }

    let local_ts = ts::Transform::from_translate(-(x0 as f32), -(y0 as f32)).pre_concat(ctm);
    let Some(mut off) = ts::Pixmap::new(w as u32, h as u32) else {
        return Ok(false);
    };
    let mut paint = ts::Paint::default();
    paint.anti_alias = true;
    paint.shader = ts::Pattern::new(
        tile_pixmap.as_ref(),
        ts::SpreadMode::Repeat,
        ts::FilterQuality::Bicubic,
        opacity,
        patt_ts,
    );
    let rule = match encoded.fill_type {
        FillType::Winding => ts::FillRule::Winding,
        FillType::EvenOdd => ts::FillRule::EvenOdd,
    };
    off.fill_path(&path, &paint, rule, local_ts, None);

    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(image) = images::raster_from_data(&info, Data::new_copy(off.data()), w as usize * 4)
    else {
        return Ok(false);
    };
    let mut draw_paint = Paint::default();
    draw_paint.set_alpha(255);
    canvas.save();
    canvas.reset_matrix();
    canvas.draw_image_with_sampling_options(
        image,
        (x0 as f32, y0 as f32),
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
        Some(&draw_paint),
    );
    canvas.restore();
    Ok(true)
}

fn convert_blend_mode(m: BlendMode) -> skia_safe::BlendMode {
    match m {
        BlendMode::Clear => skia_safe::BlendMode::Clear,
        BlendMode::Src => skia_safe::BlendMode::Src,
        BlendMode::Dst => skia_safe::BlendMode::Dst,
        BlendMode::SrcOver => skia_safe::BlendMode::SrcOver,
        BlendMode::DstOver => skia_safe::BlendMode::DstOver,
        BlendMode::SrcIn => skia_safe::BlendMode::SrcIn,
        BlendMode::DstIn => skia_safe::BlendMode::DstIn,
        BlendMode::SrcOut => skia_safe::BlendMode::SrcOut,
        BlendMode::DstOut => skia_safe::BlendMode::DstOut,
        BlendMode::SrcATop => skia_safe::BlendMode::SrcATop,
        BlendMode::DstATop => skia_safe::BlendMode::DstATop,
        BlendMode::Xor => skia_safe::BlendMode::Xor,
        BlendMode::Plus => skia_safe::BlendMode::Plus,
        BlendMode::Modulate => skia_safe::BlendMode::Modulate,
        BlendMode::Screen => skia_safe::BlendMode::Screen,
        BlendMode::Overlay => skia_safe::BlendMode::Overlay,
        BlendMode::Darken => skia_safe::BlendMode::Darken,
        BlendMode::Lighten => skia_safe::BlendMode::Lighten,
        BlendMode::ColorDodge => skia_safe::BlendMode::ColorDodge,
        BlendMode::ColorBurn => skia_safe::BlendMode::ColorBurn,
        BlendMode::HardLight => skia_safe::BlendMode::HardLight,
        BlendMode::SoftLight => skia_safe::BlendMode::SoftLight,
        BlendMode::Difference => skia_safe::BlendMode::Difference,
        BlendMode::Exclusion => skia_safe::BlendMode::Exclusion,
        BlendMode::Multiply => skia_safe::BlendMode::Multiply,
        BlendMode::Hue => skia_safe::BlendMode::Hue,
        BlendMode::Saturation => skia_safe::BlendMode::Saturation,
        BlendMode::Color => skia_safe::BlendMode::Color,
        BlendMode::Luminosity => skia_safe::BlendMode::Luminosity,
    }
}
