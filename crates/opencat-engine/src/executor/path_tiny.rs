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
    BlendMode, FillSpec, PaintSpec, PaintStyle, PathEffectSpec, StrokeCap, StrokeJoin, StrokeSpec,
};
use opencat_core::ir::draw_types::{EncodedPath, FillType, PathOp};

use super::paint::convert_blend_mode;

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

    // 设备空间 CTM（DrawOp 流的坐标系 = canvas 局部空间）。skia asAffine
    // 顺序 = [scaleX, skewY, skewX, scaleY, transX, transY]，与 tiny
    // from_row(sx, ky, kx, sy, tx, ty) 同序——按位置直传，勿按名重排。
    let m = canvas.local_to_device_as_3x3();
    let Some(a) = m.to_affine() else {
        return false;
    };
    let ctm = ts::Transform::from_row(a[0], a[1], a[2], a[3], a[4], a[5]);

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
    let max_scale = a[0].abs().max(a[1].abs()).max(a[2].abs()).max(a[3].abs());
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
            "[tiny] style={:?} local_bounds=({},{})-({},{}) ctm=({:?}) dev=({bx0:.1},{by0:.1})-({bx1:.1},{by1:.1}) pix=({x0},{y0})+({w}x{h}) clip={:?}",
            spec.style, b.left(), b.top(), b.right(), b.bottom(), a,
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
    // svg `<mask>` 不在这里乘：tiny 接管的 draw 直接画进所属 SaveLayer，
    // 层 Restore 前统一以 DstIn blend 罩 coverage（见 mask_apply_dst_in）。
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
    let Some(a) = m.to_affine() else {
        return false;
    };
    let ctm = ts::Transform::from_row(a[0], a[1], a[2], a[3], a[4], a[5]);

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
pub fn clip_path_tiny(canvas: &Canvas, ops: &[PathOp], rule: ts::FillRule) -> bool {
    if !enabled() || ops.is_empty() {
        return false;
    }
    let Some(path) = tiny_path(ops) else {
        return false;
    };
    let m = canvas.local_to_device_as_3x3();
    let Some(a) = m.to_affine() else {
        return false;
    };
    let ctm = ts::Transform::from_row(a[0], a[1], a[2], a[3], a[4], a[5]);

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
    off.fill_path(&path, &paint, rule, local_ts, None);

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

/// svg `<pattern>` 填充/描边的 tiny 路径：resvg `render_pattern_pixmap` 的
/// 同式实现——tile 像素尺寸 = tile rect × (ctm ∘ patternTransform) 的缩放
/// 分量，内容子树（picture recording）在 skia 离屏上按 scale(sx,sy) 重放成
/// tile，再以 tiny Pattern（Repeat+Bicubic）平铺 fill/stroke。tile 渲染失败
/// 返回 Err；返回 Ok(false) 表示未接管（调用方回退）。
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
    stroke: Option<&StrokeSpec>,
) -> Result<bool, super::DrawError> {
    if !enabled() || opacity <= 0.0 {
        return Ok(false);
    }
    let Some(path) = tiny_path(&encoded.ops) else {
        return Ok(false);
    };
    let m = canvas.local_to_device_as_3x3();
    let Some(a0) = m.to_affine() else {
        return Ok(false);
    };
    let ctm = ts::Transform::from_row(a0[0], a0[1], a0[2], a0[3], a0[4], a0[5]);
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
    let max_scale = a0[0].abs().max(a0[1].abs()).max(a0[2].abs()).max(a0[3].abs());
    let pad = match stroke {
        Some(s) => (s.width / 2.0) * max_scale * if s.join == StrokeJoin::Miter { s.miter_limit } else { 1.5 } + 1.5,
        None => 1.5,
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
    if let Some(s) = stroke {
        let st = ts::Stroke {
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
        off.stroke_path(&path, &paint, &st, local_ts, None);
    } else {
        let rule = match encoded.fill_type {
            FillType::Winding => ts::FillRule::Winding,
            FillType::EvenOdd => ts::FillRule::EvenOdd,
        };
        off.fill_path(&path, &paint, rule, local_ts, None);
    }

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

/// svg `<mask>` 的 tiny 路径：内容子树（user space）在 skia 离屏上按当时
/// CTM 重放成遮罩图，CPU 端按 tiny-skia `Mask::from_pixmap` 同式折算
/// （luminance：demul → 0.2126R+0.7152G+0.0722B → ×alpha → ceil；alpha：
/// 直取 alpha），region 外为 0；折算结果装进全帧 Image 入栈，所在层
/// Restore 前以 DstIn blend 画回（mask_apply_dst_in）。resvg 的 mask 渲染
/// 到独立 pixmap 后 apply_mask，与本文同构。
#[allow(clippy::too_many_arguments)]
pub fn mask_push_tiny(
    canvas: &Canvas,
    exec: &mut super::EngineDrawExecutor,
    draw: &super::DrawOpFrame,
    media: &super::EnginePreparedFrameMedia,
    subtree: opencat_core::ir::draw_types::SubtreeId,
    kind: u8,
    region: &opencat_core::ir::draw_op::Rect4,
) -> Result<bool, super::DrawError> {
    if !enabled() {
        return Ok(false);
    }
    let m = canvas.local_to_device_as_3x3();
    // skia `to_affine`/asAffine 顺序 = [scaleX, skewY, skewX, scaleY, transX,
    // transY]，与 tiny `Transform::from_row(sx, ky, kx, sy, tx, ty)` 同序
    // （tiny 0.12 注释明言「ky-kx, not kx-ky」）——按位置直传即可，勿按
    // 变量名重排（重排即转置，identity 也会烂成 y'=x）。
    let Some(a) = m.to_affine() else {
        return Ok(false);
    };
    let ctm = ts::Transform::from_row(a[0], a[1], a[2], a[3], a[4], a[5]);

    // coverage 建成全帧尺寸：region 外自然为 0（resvg 的 mask pixmap 就是
    // 画布大小，region 由 alpha_mask 乘出）。Restore 前按层基准 DstIn 罩回。
    let size = canvas.image_info().dimensions();
    let (fw, fh) = (size.width, size.height);
    if fw <= 0 || fh <= 0 {
        return Ok(false);
    }
    let mut cov = vec![0_u8; (fw * fh) as usize];

    // 退化 region（面积 0）：mask 全域不可见 = 全剪，全 0 coverage 即可。
    let mut corners = [
        ts::Point::from_xy(region.x, region.y),
        ts::Point::from_xy(region.x + region.width, region.y),
        ts::Point::from_xy(region.x, region.y + region.height),
        ts::Point::from_xy(region.x + region.width, region.y + region.height),
    ];
    ctm.map_points(&mut corners);
    let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &corners {
        bx0 = bx0.min(p.x);
        by0 = by0.min(p.y);
        bx1 = bx1.max(p.x);
        by1 = by1.max(p.y);
    }
    let empty_region = (bx1 - bx0) <= 0.0 || (by1 - by0) <= 0.0;

    if !empty_region {
        // mask 内容重放到全帧离屏（套当时的组 CTM；直接复用 skia Matrix，
        // 不经 tiny 往返）。
        let info = ImageInfo::new((fw, fh), ColorType::RGBA8888, AlphaType::Premul, None);
        let Some(mut surface) = skia_safe::surfaces::raster(&info, None, None) else {
            return Ok(false);
        };
        {
            let mask_canvas = surface.canvas();
            mask_canvas.concat(&m);
            let saved_alpha = exec.current_alpha;
            exec.current_alpha = 1.0;
            exec.mask_replay += 1;
            let replay =
                crate::executor::replay::replay_subtree(exec, mask_canvas, draw, media, subtree);
            exec.mask_replay -= 1;
            exec.current_alpha = saved_alpha;
            replay?;
        }
        let mut pix = vec![0_u8; info.compute_min_byte_size()];
        if !surface.read_pixels(&info, &mut pix, info.min_row_bytes(), (0, 0)) {
            return Ok(false);
        }

        // region 的 AA coverage（resvg alpha_mask = fill_path(rect, AA, ctm)）：
        // tiny 同 crate 同公式，逐像素乘上。
        let region_cov = region_coverage(&ctm, region, fw, fh);

        // 逐像素折算遮罩（tiny-skia mask.rs 同式）。
        for ((i, p), &rc) in pix.chunks_exact_mut(4).enumerate().zip(region_cov.iter()) {
            let (r, g, b, a) = (p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32);
            let mask_v = if kind == 1 {
                a
            } else {
                let af = a / 255.0;
                let (mut rn, mut gn, mut bn) = (r / 255.0, g / 255.0, b / 255.0);
                if a != 0.0 {
                    rn /= af;
                    gn /= af;
                    bn /= af;
                }
                let luma = rn * 0.2126 + gn * 0.7152 + bn * 0.0722;
                (luma * af * 255.0).clamp(0.0, 255.0).ceil()
            };
            // resvg：mask_pixmap.apply_mask(&alpha_mask) 用 tiny div255。
            let v = if rc == 255 {
                mask_v as u8
            } else {
                div255_round(mask_v as u32, rc as u32)
            };
            if v != 0 {
                cov[i] = v;
            }
        }
    }

    // coverage 装进全帧 premul Image（rgb=0、a=coverage），供所在层
    // Restore 前 DstIn 罩回；region 外天然 0。
    let mut buf = vec![0_u8; (fw * fh) as usize * 4];
    for (px, &c) in buf.chunks_exact_mut(4).zip(cov.iter()) {
        px[3] = c;
    }
    let minfo = ImageInfo::new((fw, fh), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(img) = images::raster_from_data(&minfo, Data::new_copy(&buf), fw as usize * 4) else {
        return Ok(false);
    };
    exec.mask_stack.push(img);
    Ok(true)
}

/// 层 Restore 前，把该层期间入栈的 mask coverage 以 DstIn blend 画回层上。
/// 全程 canvas API 无像素拷贝；skia blend 的定点乘是 round 型
/// （`(a*b+128)>>8` 系），与 resvg `apply_mask` 的 `(v+255)>>8` 同向，
/// 而 clip coverage 是截断型（0.5/255 边界差 1），故必须走 blend。
/// 嵌套 mask：多张 coverage 依次 DstIn = 亮度相乘，语义正确。
pub fn mask_apply_dst_in(canvas: &Canvas, exec: &mut super::EngineDrawExecutor) {
    let Some(mark) = exec.layer_marks.pop() else {
        return;
    };
    if exec.mask_stack.len() <= mark {
        return;
    }
    let size = canvas.image_info().dimensions();
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

/// region rect 过 CTM 后用 tiny fill_path（AA）光栅化出 0-255 coverage。
fn region_coverage(
    ctm: &ts::Transform,
    region: &opencat_core::ir::draw_op::Rect4,
    w: i32,
    h: i32,
) -> Vec<u8> {
    let mut cov = vec![0_u8; (w * h) as usize];
    let Some(rect) = ts::Rect::from_xywh(region.x, region.y, region.width, region.height) else {
        return cov;
    };
    let path = ts::PathBuilder::from_rect(rect);
    let Some(mut off) = ts::Pixmap::new(w as u32, h as u32) else {
        return cov;
    };
    let mut paint = ts::Paint::default();
    paint.set_color_rgba8(255, 255, 255, 255);
    paint.anti_alias = true;
    off.fill_path(&path, &paint, ts::FillRule::Winding, *ctm, None);
    for (dst, src) in cov.iter_mut().zip(off.pixels().iter()) {
        *dst = src.alpha();
    }
    cov
}

/// tiny-skia 的 `div255(v) = (v + 255) >> 8`（round 型定点除）。
fn div255_round(a: u32, b: u32) -> u8 {
    ((a * b + 255) >> 8).min(255) as u8
}

