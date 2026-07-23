use crate::canvas::Rect;
use crate::canvas::glyph::FontEdging;
use crate::canvas::paint::{
    BlendMode, BlurStyle, FillSpec, ImageFilterSpec, MaskFilterSpec, PaintSpec, PaintStyle,
    PathEffectSpec, ShaderSpec, StrokeCap, StrokeJoin, StrokeSpec, TileMode,
};
use crate::display::list::{
    BitmapDisplayItem, DisplayItem, DisplayRect, DrawScriptDisplayItem, RectDisplayItem,
    SvgPathDisplayItem, TimelineDisplayItem, TimelineTransitionDisplay,
};
use crate::display::tree::{DisplayNode, HiddenChildDisplayNode};
use crate::ir::GeneratedImageTable;
use crate::ir::draw_op::{
    ColorU8, DRRectSpec, DrawOp, LineCap, LineJoin, PointMode as DrawPointMode, Radii4, Rect4,
};
use crate::ir::draw_types::ImageRef;
use crate::ir::draw_types::{
    ChildRange, DrawOpRange, EncodedPath, FillType, PaintId, PathOp, RuntimeEffectChildRef,
    ScriptRuntimeEffectChild, SubtreeId,
};
use crate::media::VideoFrameRequest;
use crate::parse::gl_transition;
use crate::parse::transition::{
    GlTransition, LightLeakTransition, SlideDirection, TransitionKind, WipeDirection,
};
use crate::probe::catalog::VideoInfoMeta;
use crate::render::builder::DrawOpBuilder;
use crate::style::{
    BackgroundFill, BorderRadius, BorderStyle, BoxShadow, ColorToken, CssFilter, CssFilterKind,
    DropShadow, GradientDirection, InsetShadow, ObjectFit,
};

use super::RenderError;
use super::ctx::RenderCtx;

use kurbo::BezPath;

fn color_matrix_for_filter_op(kind: CssFilterKind, value: f32) -> Option<[f32; 20]> {
    match kind {
        CssFilterKind::Blur => None,
        CssFilterKind::Brightness => Some([
            value, 0.0, 0.0, 0.0, 0.0, 0.0, value, 0.0, 0.0, 0.0, 0.0, 0.0, value, 0.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
        ]),
        CssFilterKind::Contrast => {
            let intercept = 0.5 * (1.0 - value);
            Some([
                value, 0.0, 0.0, 0.0, intercept, 0.0, value, 0.0, 0.0, intercept, 0.0, 0.0, value,
                0.0, intercept, 0.0, 0.0, 0.0, 1.0, 0.0,
            ])
        }
        CssFilterKind::Grayscale => {
            let a = value;
            let b = 1.0 - a;
            Some([
                b + a * 0.2126,
                a * 0.7152,
                a * 0.0722,
                0.0,
                0.0,
                a * 0.2126,
                b + a * 0.7152,
                a * 0.0722,
                0.0,
                0.0,
                a * 0.2126,
                a * 0.7152,
                b + a * 0.0722,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
        CssFilterKind::HueRotate => {
            let radians = value * std::f32::consts::PI / 180.0;
            let cos = radians.cos();
            let sin = radians.sin();
            Some([
                0.2126 + cos * 0.7874 - sin * 0.2126,
                0.7152 - cos * 0.7152 - sin * 0.7152,
                0.0722 - cos * 0.0722 + sin * 0.9278,
                0.0,
                0.0,
                0.2126 - cos * 0.2126 + sin * 0.1437,
                0.7152 + cos * 0.2848 + sin * 0.1400,
                0.0722 - cos * 0.0722 - sin * 0.2837,
                0.0,
                0.0,
                0.2126 - cos * 0.2126 - sin * 0.7874,
                0.7152 - cos * 0.7152 + sin * 0.7152,
                0.0722 + cos * 0.9278 + sin * 0.0722,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
        CssFilterKind::Invert => {
            let b = 1.0 - 2.0 * value;
            Some([
                b, 0.0, 0.0, 0.0, value, 0.0, b, 0.0, 0.0, value, 0.0, 0.0, b, 0.0, value, 0.0,
                0.0, 0.0, 1.0, 0.0,
            ])
        }
        CssFilterKind::Saturate => {
            let a = (1.0 - value) * 0.2126;
            let b = (1.0 - value) * 0.7152;
            let c = (1.0 - value) * 0.0722;
            Some([
                a + value,
                b,
                c,
                0.0,
                0.0,
                a,
                b + value,
                c,
                0.0,
                0.0,
                a,
                b,
                c + value,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
        CssFilterKind::Sepia => {
            let a = value;
            let b = 1.0 - a;
            Some([
                b + a * 0.393,
                a * 0.769,
                a * 0.189,
                0.0,
                0.0,
                a * 0.349,
                b + a * 0.686,
                a * 0.168,
                0.0,
                0.0,
                a * 0.272,
                a * 0.534,
                b + a * 0.131,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
            ])
        }
    }
}

pub(crate) fn css_filter_image_filter(filter: &CssFilter) -> Option<ImageFilterSpec> {
    let mut image_filter = None;
    for op in &filter.ops {
        if op.is_identity() {
            continue;
        }
        let next = match op.kind {
            CssFilterKind::Blur => ImageFilterSpec::Blur {
                sigma_x: op.value,
                sigma_y: op.value,
                crop_rect: None,
            },
            kind => {
                let matrix = color_matrix_for_filter_op(kind, op.value)?;
                ImageFilterSpec::ColorFilter(Box::new(
                    crate::canvas::paint::ColorFilterSpec::Matrix(matrix),
                ))
            }
        };
        image_filter = Some(match image_filter {
            Some(inner) => ImageFilterSpec::Compose(Box::new(next), Box::new(inner)),
            None => next,
        });
    }
    image_filter
}

#[cfg(feature = "profile")]
use tracing::{Level, event, span};

// ── Paint conversion ─────────────────────────────────────────────────

/// Convert `ColorToken` to `[f32; 4]` with channels in 0.0–1.0.
pub fn color_token_to_rgba(ct: &ColorToken) -> [f32; 4] {
    let (r, g, b, a) = ct.rgba();
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

/// Convert `BackgroundFill` to `PaintSpec` (fill-only, no stroke).
pub fn background_fill_to_paint_spec(fill: &BackgroundFill) -> PaintSpec {
    PaintSpec {
        fill: background_fill_to_fill_spec(fill),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    }
}

/// Convert `BackgroundFill` to `FillSpec`.
pub fn background_fill_to_fill_spec(fill: &BackgroundFill) -> FillSpec {
    match fill {
        BackgroundFill::Solid { color } => FillSpec::Solid(color_token_to_rgba(color)),
        BackgroundFill::LinearGradient { direction, stops } => {
            let shader = linear_gradient_to_shader_spec(*direction, stops);
            FillSpec::Shader(shader)
        }
        BackgroundFill::RadialGradient { center, stops } => {
            let shader = radial_gradient_to_shader_spec(center, stops);
            FillSpec::Shader(shader)
        }
        BackgroundFill::ArbitraryGradient { gradient } => {
            let shader = arbitrary_gradient_to_shader_spec(gradient);
            FillSpec::Shader(shader)
        }
    }
}

/// Build a `MaskFilterSpec::Blur` (Normal style) from a `BoxShadow`, plus the shadow color.
pub fn box_shadow_to_mask_filter(shadow: &BoxShadow) -> (MaskFilterSpec, [f32; 4]) {
    let color = color_token_to_rgba(&shadow.color);
    let filter = MaskFilterSpec::Blur {
        sigma: shadow.blur_sigma,
        style: BlurStyle::Normal,
        respect_ctm: true,
    };
    (filter, color)
}

/// Build a `MaskFilterSpec::Blur` (Inner style) from an `InsetShadow`, plus the shadow color.
pub fn inset_shadow_to_mask_filter(shadow: &InsetShadow) -> (MaskFilterSpec, [f32; 4]) {
    let color = color_token_to_rgba(&shadow.color);
    let filter = MaskFilterSpec::Blur {
        sigma: shadow.blur_sigma,
        style: BlurStyle::Inner,
        respect_ctm: true,
    };
    (filter, color)
}

/// Build an `ImageFilterSpec::DropShadow` from a `DropShadow`, plus the shadow color.
pub fn drop_shadow_to_image_filter(shadow: &DropShadow) -> (ImageFilterSpec, [f32; 4]) {
    let color = color_token_to_rgba(&shadow.color);
    let filter = ImageFilterSpec::DropShadow {
        dx: shadow.offset_x,
        dy: shadow.offset_y,
        sigma_x: shadow.blur_sigma,
        sigma_y: shadow.blur_sigma,
        color,
    };
    (filter, color)
}

/// Build the (stops, colors) pair from arbitrary `GradientStop`s.
fn gradient_stops_colors(stops: &[crate::style::GradientStop]) -> (Vec<f32>, Vec<[f32; 4]>) {
    (
        stops.iter().map(|s| s.pos).collect(),
        stops
            .iter()
            .map(|s| color_token_to_rgba(&s.color))
            .collect(),
    )
}

/// Convert a linear gradient definition to a `ShaderSpec::LinearGradient`.
///
/// The gradient is always horizontal/vertical based on direction, expressed in
/// unit-square coordinates and mapped to the rect by the renderer.
fn linear_gradient_to_shader_spec(
    direction: GradientDirection,
    stops: &[crate::style::GradientStop],
) -> ShaderSpec {
    let (from_pt, to_pt) = direction_endpoints(&direction);
    let (stops, colors) = gradient_stops_colors(stops);

    ShaderSpec::LinearGradient {
        from: from_pt,
        to: to_pt,
        stops,
        colors,
        tile_mode: TileMode::Clamp,
        local_matrix: None,
    }
}

/// Convert a radial gradient definition to a `ShaderSpec::RadialGradient`.
///
/// `center` 为单位正方形内的圆心。半径取圆心到四个角的最远距离（`farthest-corner`），
/// 与 linear 渐变一致地在单位正方形坐标系内表达，由渲染层映射到实际 rect。
fn radial_gradient_to_shader_spec(
    center: &[f32; 2],
    stops: &[crate::style::GradientStop],
) -> ShaderSpec {
    let (stops, colors) = gradient_stops_colors(stops);
    // farthest-corner：到单位正方形四角的最大距离。
    let dx = center[0].max(1.0 - center[0]);
    let dy = center[1].max(1.0 - center[1]);
    let radius = (dx * dx + dy * dy).sqrt();

    ShaderSpec::RadialGradient {
        center: *center,
        radius,
        stops,
        colors,
        tile_mode: TileMode::Clamp,
        local_matrix: None,
    }
}

/// Return (from, to) unit-square endpoints for a `GradientDirection`.
fn direction_endpoints(dir: &GradientDirection) -> ([f32; 2], [f32; 2]) {
    match dir {
        GradientDirection::ToRight => ([0.0, 0.0], [1.0, 0.0]),
        GradientDirection::ToLeft => ([1.0, 0.0], [0.0, 0.0]),
        GradientDirection::ToBottom => ([0.0, 0.0], [0.0, 1.0]),
        GradientDirection::ToTop => ([0.0, 1.0], [0.0, 0.0]),
        GradientDirection::ToBottomRight => ([0.0, 0.0], [1.0, 1.0]),
    }
}

/// 将 CSS 渐变角度（deg，0=向上、90=向右）转为单位正方形内的起点/终点。
fn angle_endpoints(angle_deg: f32) -> ([f32; 2], [f32; 2]) {
    let rad = angle_deg.to_radians();
    let dir_x = rad.sin();
    let dir_y = -rad.cos();
    // CSS 渐变线长度 = |cos θ|·w + |sin θ|·h 的半长，单位正方形下 w=h=1。
    let half_len = dir_x.abs() + dir_y.abs();
    let cx = 0.5;
    let cy = 0.5;
    (
        [cx - dir_x * half_len, cy - dir_y * half_len],
        [cx + dir_x * half_len, cy + dir_y * half_len],
    )
}

/// 构造行优先 3×3 缩放矩阵 `[f32; 9]`（用于 ShaderSpec::local_matrix）。
fn scale_matrix(sx: f32, sy: f32) -> [f32; 9] {
    [sx, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 1.0]
}

/// Convert an arbitrary CSS-syntax gradient into a canvas `ShaderSpec`.
///
/// 无 `size` 时在单位正方形内表达（由渲染层映射到 rect）；有 `size` 时在像素空间
/// 表达并附带一个把单位正方形缩放到 rect 尺寸的 local_matrix，使像素瓦片铺满节点。
fn arbitrary_gradient_to_shader_spec(gradient: &crate::style::ArbitraryGradient) -> ShaderSpec {
    use crate::style::ArbitraryGradient;

    let (stops, colors) = gradient_stops_colors(gradient.stops());
    let tile_mode = if gradient.repeat() {
        TileMode::Repeat
    } else {
        TileMode::Clamp
    };

    match gradient {
        ArbitraryGradient::LinearGradient {
            angle_deg,
            direction,
            size,
            ..
        } => {
            let size = *size;
            // 优先用角度；其次用预设方向；默认向右。
            let (from_pt, to_pt) = if let Some(angle) = angle_deg {
                angle_endpoints(*angle)
            } else if let Some(dir) = direction {
                direction_endpoints(dir)
            } else {
                direction_endpoints(&GradientDirection::ToRight)
            };

            if let Some([w, h]) = size {
                // 像素空间：把单位正方形坐标缩放到 [0,0]..[w,h]。
                ShaderSpec::LinearGradient {
                    from: [from_pt[0] * w, from_pt[1] * h],
                    to: [to_pt[0] * w, to_pt[1] * h],
                    stops,
                    colors,
                    tile_mode,
                    local_matrix: Some(scale_matrix(w, h)),
                }
            } else {
                ShaderSpec::LinearGradient {
                    from: from_pt,
                    to: to_pt,
                    stops,
                    colors,
                    tile_mode,
                    local_matrix: None,
                }
            }
        }
        ArbitraryGradient::RadialGradient { center, size, .. } => {
            let size = *size;
            if let Some([w, h]) = size {
                let dx = center[0].max(1.0 - center[0]);
                let dy = center[1].max(1.0 - center[1]);
                let radius = (dx * dx + dy * dy).sqrt();
                ShaderSpec::RadialGradient {
                    center: [center[0] * w, center[1] * h],
                    radius: radius * w.max(h),
                    stops,
                    colors,
                    tile_mode,
                    local_matrix: Some(scale_matrix(w, h)),
                }
            } else {
                let dx = center[0].max(1.0 - center[0]);
                let dy = center[1].max(1.0 - center[1]);
                let radius = (dx * dx + dy * dy).sqrt();
                ShaderSpec::RadialGradient {
                    center: *center,
                    radius,
                    stops,
                    colors,
                    tile_mode,
                    local_matrix: None,
                }
            }
        }
    }
}

// ── Script conversion ────────────────────────────────────────────────

/// Convert `ColorU8` to `[f32; 4]` with channels in 0.0–1.0.
pub fn script_color_to_rgba(c: ColorU8) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}

/// Apply a global alpha multiplier to a `ColorU8`, returning `[f32; 4]`.
pub fn script_color_with_alpha(c: ColorU8, global_alpha: f32) -> [f32; 4] {
    let mut rgba = script_color_to_rgba(c);
    rgba[3] *= global_alpha;
    rgba
}

/// Convert `ColorU8` + global alpha into a `FillSpec::Solid`.
pub fn to_fill_spec(c: ColorU8, global_alpha: f32) -> FillSpec {
    FillSpec::Solid(script_color_with_alpha(c, global_alpha))
}

/// Convert `LineCap` to canvas `StrokeCap`.
pub fn script_line_cap(c: LineCap) -> StrokeCap {
    match c {
        LineCap::Butt => StrokeCap::Butt,
        LineCap::Round => StrokeCap::Round,
        LineCap::Square => StrokeCap::Square,
    }
}

/// Convert `LineJoin` to canvas `StrokeJoin`.
pub fn script_line_join(j: LineJoin) -> StrokeJoin {
    match j {
        LineJoin::Miter => StrokeJoin::Miter,
        LineJoin::Round => StrokeJoin::Round,
        LineJoin::Bevel => StrokeJoin::Bevel,
    }
}

/// Convert draw `PointMode` to canvas `PointMode`.
pub fn script_point_mode(m: DrawPointMode) -> crate::canvas::PointMode {
    match m {
        DrawPointMode::Points => crate::canvas::PointMode::Points,
        DrawPointMode::Lines => crate::canvas::PointMode::Lines,
        DrawPointMode::Polygon => crate::canvas::PointMode::Polygon,
    }
}

/// Parse font edging from string.
pub fn script_font_edging(name: &str) -> FontEdging {
    match name {
        "alias" => FontEdging::Alias,
        "antiAlias" => FontEdging::AntiAlias,
        "subpixelAntiAlias" => FontEdging::SubpixelAntiAlias,
        _ => FontEdging::AntiAlias,
    }
}

// ── Rect helpers ─────────────────────────────────────────────────────

pub(crate) fn rect_to_rect4(r: Rect) -> Rect4 {
    Rect4 {
        x: r.x0 as f32,
        y: r.y0 as f32,
        width: r.width() as f32,
        height: r.height() as f32,
    }
}

fn radii_to_radii4(r: [f32; 4]) -> Radii4 {
    Radii4 {
        top_left: r[0],
        top_right: r[1],
        bottom_right: r[2],
        bottom_left: r[3],
    }
}

pub fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(
        r.x as f64,
        r.y as f64,
        (r.x + r.width) as f64,
        (r.y + r.height) as f64,
    )
}

fn kurbo_rect_xywh(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x as f64, y as f64, (x + width) as f64, (y + height) as f64)
}

fn effective_corner_radius(rect: &Rect, radius: &BorderRadius) -> [f32; 4] {
    let w = rect.width() as f32;
    let h = rect.height() as f32;
    let clamp = |r: f32| {
        if r <= 0.0 {
            0.0
        } else {
            r.min(w / 2.0).min(h / 2.0)
        }
    };
    [
        clamp(radius.top_left),
        clamp(radius.top_right),
        clamp(radius.bottom_right),
        clamp(radius.bottom_left),
    ]
}

fn spread_radius(radius: &BorderRadius, spread: f32) -> BorderRadius {
    BorderRadius {
        top_left: (radius.top_left + spread).max(0.0),
        top_right: (radius.top_right + spread).max(0.0),
        bottom_right: (radius.bottom_right + spread).max(0.0),
        bottom_left: (radius.bottom_left + spread).max(0.0),
    }
}

fn push_rrect_path(builder: &mut DrawOpBuilder, r: Rect4, radii: Radii4) {
    let x = r.x;
    let y = r.y;
    let x1 = x + r.width;
    let y1 = y + r.height;
    let tl = radii.top_left;
    let tr = radii.top_right;
    let br = radii.bottom_right;
    let bl = radii.bottom_left;

    builder.push(DrawOp::BeginPath);
    builder.push(DrawOp::Path(PathOp::MoveTo { x: x + tl, y }));
    builder.push(DrawOp::Path(PathOp::LineTo { x: x1 - tr, y }));
    if tr > 0.0 {
        builder.push(DrawOp::Path(PathOp::QuadTo {
            cx: x1,
            cy: y,
            x: x1,
            y: y + tr,
        }));
    }
    builder.push(DrawOp::Path(PathOp::LineTo { x: x1, y: y1 - br }));
    if br > 0.0 {
        builder.push(DrawOp::Path(PathOp::QuadTo {
            cx: x1,
            cy: y1,
            x: x1 - br,
            y: y1,
        }));
    }
    builder.push(DrawOp::Path(PathOp::LineTo { x: x + bl, y: y1 }));
    if bl > 0.0 {
        builder.push(DrawOp::Path(PathOp::QuadTo {
            cx: x,
            cy: y1,
            x,
            y: y1 - bl,
        }));
    }
    builder.push(DrawOp::Path(PathOp::LineTo { x, y: y + tl }));
    if tl > 0.0 {
        builder.push(DrawOp::Path(PathOp::QuadTo {
            cx: x,
            cy: y,
            x: x + tl,
            y,
        }));
    }
    builder.push(DrawOp::Path(PathOp::Close));
}

fn push_draw_rrect(builder: &mut DrawOpBuilder, rect: Rect, radii: [f32; 4], paint_id: PaintId) {
    builder.push(DrawOp::RRect {
        rect: rect_to_rect4(rect),
        radii: radii_to_radii4(radii),
        paint: paint_id,
    });
}

pub fn draw_box_shadow(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    border_radius: &BorderRadius,
    shadow: &BoxShadow,
) {
    let shadow_bounds = if shadow.spread != 0.0 {
        bounds.outset(shadow.spread, shadow.spread, shadow.spread, shadow.spread)
    } else {
        bounds
    };
    let rect = kurbo_rect(shadow_bounds.translate(shadow.offset_x, shadow.offset_y));
    let sr = spread_radius(border_radius, shadow.spread);
    let radii = effective_corner_radius(&rect, &sr);

    let (mask_filter, color) = box_shadow_to_mask_filter(shadow);
    let paint = PaintSpec {
        fill: FillSpec::Solid(color),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: Some(mask_filter),
        path_effect: None,
    };
    let paint_id = builder.intern_paint(paint);

    if radii.iter().any(|&r| r > 0.0) {
        push_draw_rrect(builder, rect, radii, paint_id);
    } else {
        builder.push(DrawOp::Rect {
            rect: rect_to_rect4(rect),
            paint: paint_id,
        });
    }
}

pub fn draw_inset_shadow(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    border_radius: &BorderRadius,
    shadow: &InsetShadow,
) {
    let shadow_bounds = if shadow.spread != 0.0 {
        bounds.outset(shadow.spread, shadow.spread, shadow.spread, shadow.spread)
    } else {
        bounds
    };
    let rect = kurbo_rect(shadow_bounds.translate(shadow.offset_x, shadow.offset_y));
    let sr = spread_radius(border_radius, shadow.spread);
    let radii = effective_corner_radius(&rect, &sr);

    let (mask_filter, color) = inset_shadow_to_mask_filter(shadow);
    let paint = PaintSpec {
        fill: FillSpec::Solid(color),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: Some(mask_filter),
        path_effect: None,
    };
    let paint_id = builder.intern_paint(paint);

    builder.push(DrawOp::Save);
    clip_bounds(builder, bounds, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        push_draw_rrect(builder, rect, radii, paint_id);
    } else {
        builder.push(DrawOp::Rect {
            rect: rect_to_rect4(rect),
            paint: paint_id,
        });
    }
    builder.push(DrawOp::Restore);
}

pub fn clip_bounds(builder: &mut DrawOpBuilder, bounds: DisplayRect, border_radius: &BorderRadius) {
    let rect = kurbo_rect(bounds);
    let radii = effective_corner_radius(&rect, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        push_rrect_path(builder, rect_to_rect4(rect), radii_to_radii4(radii));
        builder.push(DrawOp::ClipPath { anti_alias: true });
    } else {
        let r4 = rect_to_rect4(rect);
        builder.push(DrawOp::BeginPath);
        builder.push(DrawOp::Path(PathOp::AddRect {
            x: r4.x,
            y: r4.y,
            width: r4.width,
            height: r4.height,
        }));
        builder.push(DrawOp::ClipPath { anti_alias: true });
    }
}

pub fn draw_item_drop_shadow(
    ctx: &mut RenderCtx,
    bounds: DisplayRect,
    shadow: &DropShadow,
    draw: impl FnOnce(&mut RenderCtx) -> Result<(), RenderError>,
) -> Result<(), RenderError> {
    let (left, top, right, bottom) = shadow.outsets();
    let shadow_bounds = kurbo_rect(bounds.outset(left, top, right, bottom));

    let (image_filter, _color) = drop_shadow_to_image_filter(shadow);
    let paint = PaintSpec {
        fill: FillSpec::Solid([0.0; 4]),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: Some(image_filter),
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };
    let paint_id = ctx.builder.intern_paint(paint);
    ctx.builder.push(DrawOp::SaveLayer {
        bounds: Some(rect_to_rect4(shadow_bounds)),
        paint: Some(paint_id),
        alpha: 1.0,
    });

    let result = draw(ctx);

    ctx.builder.push(DrawOp::Restore);
    result
}

fn apply_blur_effect(spec: &mut PaintSpec, blur_sigma: Option<f32>) {
    if let Some(sigma) = blur_sigma
        && sigma > 0.0
    {
        spec.mask_filter = Some(MaskFilterSpec::Blur {
            sigma,
            style: BlurStyle::Normal,
            respect_ctm: true,
        });
    }
}

fn build_stroke_paint(
    color: &[f32; 4],
    width: f32,
    border_style: &BorderStyle,
    blur_sigma: Option<f32>,
) -> PaintSpec {
    let mut p = PaintSpec {
        fill: FillSpec::Solid(*color),
        style: PaintStyle::Stroke,
        stroke: Some(StrokeSpec {
            width,
            cap: StrokeCap::Butt,
            ..StrokeSpec::default()
        }),
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };
    apply_blur_effect(&mut p, blur_sigma);

    match border_style {
        BorderStyle::Solid => {}
        BorderStyle::Dashed => {
            let unit = width.max(1.0) * 2.0;
            p.path_effect = Some(PathEffectSpec::Dash {
                intervals: vec![unit, unit],
                phase: 0.0,
            });
        }
        BorderStyle::Dotted => {
            if let Some(ref mut s) = p.stroke {
                s.cap = StrokeCap::Round;
            }
            let gap = width.max(1.0) * 2.0;
            p.path_effect = Some(PathEffectSpec::Dash {
                intervals: vec![0.0, gap],
                phase: 0.0,
            });
        }
    }
    p
}

pub fn draw_node_border(
    builder: &mut DrawOpBuilder,
    rect: &Rect,
    radius: &BorderRadius,
    border_width: Option<f32>,
    border_top_width: Option<f32>,
    border_right_width: Option<f32>,
    border_bottom_width: Option<f32>,
    border_left_width: Option<f32>,
    border_color: Option<ColorToken>,
    border_style: Option<BorderStyle>,
    blur_sigma: Option<f32>,
) {
    let Some(color) = border_color else {
        return;
    };
    let uniform = border_width.unwrap_or(0.0);
    let top_w = border_top_width.unwrap_or(uniform);
    let right_w = border_right_width.unwrap_or(uniform);
    let bottom_w = border_bottom_width.unwrap_or(uniform);
    let left_w = border_left_width.unwrap_or(uniform);
    if top_w <= 0.0 && right_w <= 0.0 && bottom_w <= 0.0 && left_w <= 0.0 {
        return;
    }

    let stroke_style = border_style.unwrap_or_default();
    let rgba = color_token_to_rgba(&color);

    match stroke_style {
        BorderStyle::Solid => {
            draw_border_fill_ring(
                builder, rect, radius, top_w, right_w, bottom_w, left_w, &rgba, blur_sigma,
            );
        }
        BorderStyle::Dashed | BorderStyle::Dotted => {
            draw_per_side_borders(
                builder,
                rect,
                radius,
                top_w,
                right_w,
                bottom_w,
                left_w,
                &rgba,
                &stroke_style,
                blur_sigma,
            );
        }
    }
}

fn draw_border_fill_ring(
    builder: &mut DrawOpBuilder,
    outer_rect: &Rect,
    outer_radius: &BorderRadius,
    top_w: f32,
    right_w: f32,
    bottom_w: f32,
    left_w: f32,
    color: &[f32; 4],
    blur_sigma: Option<f32>,
) {
    let inner_left = (outer_rect.x0 as f32 + left_w.max(0.0)) as f64;
    let inner_top = (outer_rect.y0 as f32 + top_w.max(0.0)) as f64;
    let inner_right = (outer_rect.x1 as f32 - right_w.max(0.0)) as f64;
    let inner_bottom = (outer_rect.y1 as f32 - bottom_w.max(0.0)) as f64;

    let mut paint = PaintSpec {
        fill: FillSpec::Solid(*color),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };
    apply_blur_effect(&mut paint, blur_sigma);

    let outer_rrect_r4 = rect_to_rect4(*outer_rect);
    let outer_radii = effective_corner_radius(outer_rect, outer_radius);
    let outer_rrect_radii = radii_to_radii4(outer_radii);

    if inner_right <= inner_left || inner_bottom <= inner_top {
        let paint_id = builder.intern_paint(paint);
        builder.push(DrawOp::RRect {
            rect: outer_rrect_r4,
            radii: outer_rrect_radii,
            paint: paint_id,
        });
        return;
    }

    let inner_rect = Rect::new(inner_left, inner_top, inner_right, inner_bottom);
    let inner_radius = BorderRadius {
        top_left: (outer_radius.top_left - top_w.max(left_w)).max(0.0),
        top_right: (outer_radius.top_right - top_w.max(right_w)).max(0.0),
        bottom_right: (outer_radius.bottom_right - bottom_w.max(right_w)).max(0.0),
        bottom_left: (outer_radius.bottom_left - bottom_w.max(left_w)).max(0.0),
    };
    let inner_radii = effective_corner_radius(&inner_rect, &inner_radius);

    let paint_id = builder.intern_paint(paint);
    builder.push(DrawOp::DRRect {
        outer: DRRectSpec {
            rect: outer_rrect_r4,
            radii: outer_rrect_radii,
        },
        inner: DRRectSpec {
            rect: rect_to_rect4(inner_rect),
            radii: radii_to_radii4(inner_radii),
        },
        paint: paint_id,
    });
}

fn draw_per_side_borders(
    builder: &mut DrawOpBuilder,
    rect: &Rect,
    radius: &BorderRadius,
    top_w: f32,
    right_w: f32,
    bottom_w: f32,
    left_w: f32,
    color: &[f32; 4],
    border_style: &BorderStyle,
    blur_sigma: Option<f32>,
) {
    let left = rect.x0 as f32;
    let top = rect.y0 as f32;
    let right = rect.x1 as f32;
    let bottom = rect.y1 as f32;
    let radii = effective_corner_radius(rect, radius);
    let r_tl = radii[0];
    let r_tr = radii[1];
    let r_br = radii[2];
    let r_bl = radii[3];

    if top_w > 0.0 {
        let y = top + top_w / 2.0;
        let x0 = if top_w == left_w && r_tl > 0.0 {
            left + r_tl
        } else if left_w > 0.0 && top_w == left_w {
            left + left_w
        } else {
            left
        };
        let x1 = if top_w == right_w && r_tr > 0.0 {
            right - r_tr
        } else if right_w > 0.0 && top_w == right_w {
            right - right_w
        } else {
            right
        };
        if x1 > x0 {
            let paint = build_stroke_paint(color, top_w, border_style, blur_sigma);
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Line {
                x0,
                y0: y,
                x1,
                y1: y,
                paint: paint_id,
            });
        }
    }

    if right_w > 0.0 {
        let x = right - right_w / 2.0;
        let y0 = if right_w == top_w && r_tr > 0.0 {
            top + r_tr
        } else if top_w > 0.0 && right_w == top_w {
            top + top_w
        } else {
            top
        };
        let y1 = if right_w == bottom_w && r_br > 0.0 {
            bottom - r_br
        } else if bottom_w > 0.0 && right_w == bottom_w {
            bottom - bottom_w
        } else {
            bottom
        };
        if y1 > y0 {
            let paint = build_stroke_paint(color, right_w, border_style, blur_sigma);
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Line {
                x0: x,
                y0,
                x1: x,
                y1,
                paint: paint_id,
            });
        }
    }

    if bottom_w > 0.0 {
        let y = bottom - bottom_w / 2.0;
        let x0 = if bottom_w == left_w && r_bl > 0.0 {
            left + r_bl
        } else if left_w > 0.0 && bottom_w == left_w {
            left + left_w
        } else {
            left
        };
        let x1 = if bottom_w == right_w && r_br > 0.0 {
            right - r_br
        } else if right_w > 0.0 && bottom_w == right_w {
            right - right_w
        } else {
            right
        };
        if x1 > x0 {
            let paint = build_stroke_paint(color, bottom_w, border_style, blur_sigma);
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Line {
                x0,
                y0: y,
                x1,
                y1: y,
                paint: paint_id,
            });
        }
    }

    if left_w > 0.0 {
        let x = left + left_w / 2.0;
        let y0 = if left_w == top_w && r_tl > 0.0 {
            top + r_tl
        } else if top_w > 0.0 && left_w == top_w {
            top + top_w
        } else {
            top
        };
        let y1 = if left_w == bottom_w && r_bl > 0.0 {
            bottom - r_bl
        } else if bottom_w > 0.0 && left_w == bottom_w {
            bottom - bottom_w
        } else {
            bottom
        };
        if y1 > y0 {
            let paint = build_stroke_paint(color, left_w, border_style, blur_sigma);
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Line {
                x0: x,
                y0,
                x1: x,
                y1,
                paint: paint_id,
            });
        }
    }

    let draw_corner_arc = |builder: &mut DrawOpBuilder,
                           cx: f32,
                           cy: f32,
                           corner_r: f32,
                           width: f32,
                           start_deg: f32| {
        let arc_r = (corner_r - width / 2.0).max(0.0);
        if arc_r <= 0.0 {
            return;
        }
        let oval = kurbo_rect_xywh(cx - arc_r, cy - arc_r, 2.0 * arc_r, 2.0 * arc_r);
        let paint = build_stroke_paint(color, width, border_style, blur_sigma);
        let paint_id = builder.intern_paint(paint);
        builder.push(DrawOp::Arc {
            rect: rect_to_rect4(oval),
            start: start_deg,
            sweep: 90.0,
            use_center: false,
            paint: paint_id,
        });
    };

    if r_tl > 0.0 && top_w > 0.0 && top_w == left_w {
        draw_corner_arc(builder, left + r_tl, top + r_tl, r_tl, top_w, 180.0);
    }
    if r_tr > 0.0 && top_w > 0.0 && top_w == right_w {
        draw_corner_arc(builder, right - r_tr, top + r_tr, r_tr, top_w, 270.0);
    }
    if r_br > 0.0 && bottom_w > 0.0 && bottom_w == right_w {
        draw_corner_arc(builder, right - r_br, bottom - r_br, r_br, bottom_w, 0.0);
    }
    if r_bl > 0.0 && bottom_w > 0.0 && bottom_w == left_w {
        draw_corner_arc(builder, left + r_bl, bottom - r_bl, r_bl, bottom_w, 90.0);
    }
}

pub fn render_rect(ctx: &mut RenderCtx, item: &RectDisplayItem) -> Result<(), RenderError> {
    let style = &item.paint;
    let has_any_border = style.border_width.is_some()
        || style.border_top_width.is_some()
        || style.border_right_width.is_some()
        || style.border_bottom_width.is_some()
        || style.border_left_width.is_some();
    if style.background.is_empty() && !has_any_border && style.inset_shadow.is_empty() {
        return Ok(());
    }

    let bounds = item.bounds;
    let rect = kurbo_rect(bounds);
    let radii = effective_corner_radius(&rect, &style.border_radius);
    let has_radius = radii.iter().any(|&r| r > 0.0);

    let builder = &mut ctx.builder;
    builder.push(DrawOp::Save);
    clip_bounds(builder, bounds, &style.border_radius);

    if let Some(sigma) = style.backdrop_blur_sigma
        && sigma > 0.0
    {
        let blur_paint = PaintSpec {
            fill: FillSpec::Solid([1.0; 4]),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: Some(ImageFilterSpec::Blur {
                sigma_x: sigma,
                sigma_y: sigma,
                crop_rect: None,
            }),
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };
        let paint_id = builder.intern_paint(blur_paint);
        builder.push(DrawOp::SaveLayer {
            bounds: Some(rect_to_rect4(rect)),
            paint: Some(paint_id),
            alpha: 1.0,
        });
    }

    if !style.background.is_empty() {
        // 多层背景：按声明顺序从底到顶绘制（第一层在最底）。
        for background in &style.background {
            let paint_spec = background_fill_to_paint_spec(background);
            let paint_id = builder.intern_paint(paint_spec);
            if has_radius {
                push_draw_rrect(builder, rect, radii, paint_id);
            } else {
                builder.push(DrawOp::Rect {
                    rect: rect_to_rect4(rect),
                    paint: paint_id,
                });
            }
        }
    }

    for shadow in &style.inset_shadow {
        draw_inset_shadow(builder, bounds, &style.border_radius, shadow);
    }

    draw_node_border(
        builder,
        &rect,
        &style.border_radius,
        style.border_width,
        style.border_top_width,
        style.border_right_width,
        style.border_bottom_width,
        style.border_left_width,
        style.border_color,
        style.border_style,
        None,
    );

    if style.backdrop_blur_sigma.unwrap_or(0.0) > 0.0 {
        builder.push(DrawOp::Restore);
    }

    builder.push(DrawOp::Restore);
    Ok(())
}

pub fn render_rect_with_shadows(
    ctx: &mut RenderCtx,
    item: &RectDisplayItem,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let bounds = item.bounds;

    for shadow in &style.box_shadow {
        draw_box_shadow(ctx.builder, bounds, &style.border_radius, shadow);
    }

    for shadow in &style.drop_shadow {
        draw_item_drop_shadow(ctx, bounds, shadow, |ctx2| render_rect(ctx2, item))?;
    }
    render_rect(ctx, item)?;

    Ok(())
}

// ── Draw script ──────────────────────────────────────────────────────

/// Sentinel: the stored DrawOp carries PaintId(u32::MAX) meaning "resolve to fill paint".
const SENTINEL_FILL: u32 = u32::MAX;
/// Sentinel: the stored DrawOp carries PaintId(u32::MAX - 1) meaning "resolve to stroke paint".
const SENTINEL_STROKE: u32 = u32::MAX - 1;

struct LocalPaintState {
    fill_color: ColorU8,
    stroke_color: ColorU8,
    line_width: f32,
    line_cap: LineCap,
    line_join: LineJoin,
    line_dash: Option<Vec<f32>>,
    line_dash_phase: f32,
    global_alpha: f32,
    anti_alias: bool,
}

impl Default for LocalPaintState {
    fn default() -> Self {
        Self {
            fill_color: ColorU8 {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            stroke_color: ColorU8 {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            line_width: 1.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            line_dash: None,
            line_dash_phase: 0.0,
            global_alpha: 1.0,
            anti_alias: true,
        }
    }
}

impl LocalPaintState {
    fn fill_paint_spec(&self) -> PaintSpec {
        let mut rgba = self.fill_color;
        rgba.a = ((rgba.a as f32 * self.global_alpha).clamp(0.0, 255.0)) as u8;
        PaintSpec {
            fill: FillSpec::Solid([
                rgba.r as f32 / 255.0,
                rgba.g as f32 / 255.0,
                rgba.b as f32 / 255.0,
                rgba.a as f32 / 255.0,
            ]),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: self.anti_alias,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        }
    }

    fn stroke_paint_spec(&self) -> PaintSpec {
        let mut rgba = self.stroke_color;
        rgba.a = ((rgba.a as f32 * self.global_alpha).clamp(0.0, 255.0)) as u8;
        let mut spec = PaintSpec {
            fill: FillSpec::Solid([
                rgba.r as f32 / 255.0,
                rgba.g as f32 / 255.0,
                rgba.b as f32 / 255.0,
                rgba.a as f32 / 255.0,
            ]),
            style: PaintStyle::Stroke,
            stroke: Some(StrokeSpec {
                width: self.line_width.max(0.0),
                cap: stroke_cap_to_canvas(self.line_cap),
                join: stroke_join_to_canvas(self.line_join),
                miter_limit: 4.0,
            }),
            anti_alias: self.anti_alias,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };
        if let Some(ref intervals) = self.line_dash {
            spec.path_effect = Some(crate::canvas::paint::PathEffectSpec::Dash {
                intervals: intervals.clone(),
                phase: self.line_dash_phase,
            });
        }
        spec
    }
}

fn stroke_cap_to_canvas(cap: LineCap) -> StrokeCap {
    match cap {
        LineCap::Butt => StrokeCap::Butt,
        LineCap::Round => StrokeCap::Round,
        LineCap::Square => StrokeCap::Square,
    }
}

fn stroke_join_to_canvas(join: LineJoin) -> crate::canvas::StrokeJoin {
    match join {
        LineJoin::Miter => crate::canvas::StrokeJoin::Miter,
        LineJoin::Round => crate::canvas::StrokeJoin::Round,
        LineJoin::Bevel => crate::canvas::StrokeJoin::Bevel,
    }
}

fn rect4_xywh(x: f32, y: f32, w: f32, h: f32) -> Rect4 {
    Rect4 {
        x,
        y,
        width: w,
        height: h,
    }
}

pub fn render_draw_script(
    ctx: &mut RenderCtx,
    item: &DrawScriptDisplayItem,
) -> Result<(), RenderError> {
    let mut state = LocalPaintState::default();

    let needs_alpha_layer = item
        .commands
        .iter()
        .any(|cmd| matches!(cmd, DrawOp::Clear { .. }));

    let clip_rect = rect4_xywh(
        item.bounds.x,
        item.bounds.y,
        item.bounds.width,
        item.bounds.height,
    );

    if needs_alpha_layer {
        ctx.builder.push(DrawOp::SaveLayer {
            bounds: Some(clip_rect),
            paint: None,
            alpha: 1.0,
        });
    } else {
        ctx.builder.push(DrawOp::Save);
        ctx.builder.push(DrawOp::BeginPath);
        ctx.builder.push(DrawOp::Path(PathOp::AddRect {
            x: item.bounds.x,
            y: item.bounds.y,
            width: item.bounds.width,
            height: item.bounds.height,
        }));
        ctx.builder.push(DrawOp::ClipPath { anti_alias: true });
    }

    for command in &item.commands {
        match command {
            DrawOp::DrawSubtreePicture { .. } => {
                execute_draw_subtree_picture(ctx, command, &item.hidden_subtree)?;
            }
            DrawOp::ScriptRuntimeEffect { .. } => {
                execute_script_runtime_effect(ctx, command, &item.hidden_subtree, &mut state)?;
            }
            _ => {
                execute_draw_op(&mut ctx.builder, command, &mut state)?;
            }
        }
    }

    ctx.builder.push(DrawOp::Restore);
    Ok(())
}

fn execute_draw_subtree_picture(
    ctx: &mut RenderCtx,
    op: &DrawOp,
    hidden_subtree: &[HiddenChildDisplayNode],
) -> Result<(), RenderError> {
    let DrawOp::DrawSubtreePicture { owner_id, x, y } = op else {
        return Ok(());
    };
    if ctx.hidden_picture_stack.contains(owner_id) {
        return Err(RenderError::InvalidArgument(format!(
            "recursive hidden canvas picture `{owner_id}`"
        )));
    }
    let subtree = record_hidden_subtree(ctx, owner_id, hidden_subtree)?;
    ctx.builder.push(DrawOp::ReplaySubtreePicture {
        subtree,
        x: *x,
        y: *y,
    });
    Ok(())
}

fn record_hidden_subtree(
    ctx: &mut RenderCtx,
    owner_id: &str,
    hidden_subtree: &[HiddenChildDisplayNode],
) -> Result<SubtreeId, RenderError> {
    if ctx.hidden_picture_stack.iter().any(|item| item == owner_id) {
        return Err(RenderError::InvalidArgument(format!(
            "recursive hidden canvas picture `{owner_id}`"
        )));
    }
    ctx.hidden_picture_stack.push(owner_id.to_string());

    let stack = ctx.hidden_picture_stack.clone();
    let catalog = ctx.catalog;
    let frame_ctx = ctx.frame_ctx;
    let display_tree = ctx.display_tree;
    let ordered_scene = ctx.ordered_scene;
    let font_db = ctx.font_db;
    let generated_images: &mut GeneratedImageTable = ctx.generated_images;
    let result = ctx.builder.record_subtree(|builder| {
        let mut subtree_ctx = RenderCtx {
            catalog,
            frame_ctx,
            display_tree,
            ordered_scene,
            builder,
            font_db,
            hidden_picture_stack: stack,
            generated_images,
        };
        for child in hidden_subtree {
            if child.owner_id == owner_id {
                render_hidden_child_node(&mut subtree_ctx, &child.node)?;
            }
        }
        Ok(())
    });

    ctx.hidden_picture_stack.pop();
    result
}

/// Expand a `DrawOp::ScriptRuntimeEffect` into a canonical `DrawOp::RuntimeEffect`,
/// resolving `ScriptRuntimeEffectChild::PictureSubtree` children into isolated
/// subtree programs referenced by `RuntimeEffectChildRef::SubtreePicture`.
fn execute_script_runtime_effect(
    ctx: &mut RenderCtx,
    op: &DrawOp,
    hidden_subtree: &[HiddenChildDisplayNode],
    _state: &mut LocalPaintState,
) -> Result<(), RenderError> {
    let DrawOp::ScriptRuntimeEffect {
        sksl,
        uniforms_bytes,
        children,
        dst,
    } = op
    else {
        return Ok(());
    };

    // Resolve children. Picture children require recording the matching
    // subtree ops into the main op stream first so we can point a
    // `DrawOpRange` at them.
    let mut resolved: Vec<RuntimeEffectChildRef> = Vec::with_capacity(children.len());
    for c in children {
        match c {
            ScriptRuntimeEffectChild::Image(img) => {
                resolved.push(RuntimeEffectChildRef::Image(img.clone()));
            }
            ScriptRuntimeEffectChild::PictureSubtree { owner_id } => {
                let subtree = record_hidden_subtree(ctx, owner_id, hidden_subtree)?;
                resolved.push(RuntimeEffectChildRef::SubtreePicture(subtree));
            }
        }
    }

    use std::hash::{Hash, Hasher};
    let mut hasher = ahash::AHasher::default();
    sksl.as_bytes().hash(&mut hasher);
    let hash = hasher.finish();
    let effect_id = ctx.builder.intern_effect(hash, sksl);
    let uniforms_id = ctx.builder.intern_bytes(uniforms_bytes);
    let child_start = ctx.builder.children_len() as u32;
    let child_len = resolved.len() as u32;
    for c in resolved {
        ctx.builder.push_child(c);
    }
    ctx.builder.push(DrawOp::RuntimeEffect {
        effect: effect_id,
        uniforms: uniforms_id,
        children: ChildRange {
            start: child_start,
            len: child_len,
        },
        dst: *dst,
    });
    Ok(())
}

fn render_hidden_child_node(ctx: &mut RenderCtx, node: &DisplayNode) -> Result<(), RenderError> {
    if node.opacity <= 0.0 {
        return Ok(());
    }

    ctx.builder.push(DrawOp::Save);
    super::dispatch::apply_transform(ctx.builder, &node.transform);

    let bounds = node.item.visual_bounds();
    let layer_state = super::dispatch::save_composite_layer(
        ctx.builder,
        node.opacity,
        &node.css_filter,
        node.backdrop_blur_sigma,
        bounds,
    );

    match &node.item {
        DisplayItem::Rect(rect) => super::helpers::render_rect_with_shadows(ctx, rect)?,
        DisplayItem::Text(text) => super::text::render_text_with_shadows(ctx, text)?,
        DisplayItem::DrawScript(script) => super::helpers::render_draw_script(ctx, script)?,
        DisplayItem::SvgPath(svg) => super::helpers::render_svg_path(ctx, svg)?,
        DisplayItem::Bitmap(bitmap) => super::helpers::render_bitmap_with_shadows(ctx, bitmap)?,
        DisplayItem::Lottie(lottie) => super::helpers::render_lottie_with_shadows(ctx, lottie)?,
        DisplayItem::Timeline(timeline) => super::helpers::render_timeline(ctx, timeline)?,
    }

    if let Some(clip) = &node.clip {
        ctx.builder.push(DrawOp::Save);
        super::dispatch::clip_bounds_with_radius(
            ctx.builder,
            Rect4 {
                x: clip.bounds.x,
                y: clip.bounds.y,
                width: clip.bounds.width,
                height: clip.bounds.height,
            },
            &clip.border_radius,
        );
    }

    if let Some(slot) = &node.draw_slot
        && !slot.commands.is_empty()
    {
        super::helpers::render_draw_script(ctx, slot)?;
    }

    for child in &node.children {
        render_hidden_child_node(ctx, child)?;
    }

    if node.clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    super::dispatch::restore_backdrop_blur_layer(ctx.builder, &layer_state);
    ctx.builder.push(DrawOp::Restore);
    Ok(())
}

fn execute_draw_op(
    b: &mut crate::render::builder::DrawOpBuilder,
    op: &DrawOp,
    state: &mut LocalPaintState,
) -> Result<(), RenderError> {
    match op {
        // ── Stack management ──────────────────────────────────────────
        DrawOp::Save => {
            b.push(DrawOp::Save);
        }
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
        } => {
            b.push(DrawOp::SaveLayer {
                bounds: *bounds,
                paint: *paint,
                alpha: *alpha,
            });
        }
        DrawOp::Restore => {
            b.push(DrawOp::Restore);
        }
        DrawOp::RestoreToCount { count } => {
            b.push(DrawOp::RestoreToCount { count: *count });
        }

        // ── Transforms ────────────────────────────────────────────────
        DrawOp::Translate { x, y } => {
            b.push(DrawOp::Translate { x: *x, y: *y });
        }
        DrawOp::Scale { x, y } => {
            b.push(DrawOp::Scale { x: *x, y: *y });
        }
        DrawOp::Rotate { degrees, cx, cy } => {
            b.push(DrawOp::Rotate {
                degrees: *degrees,
                cx: *cx,
                cy: *cy,
            });
        }
        DrawOp::Skew { sx, sy } => {
            b.push(DrawOp::Skew { sx: *sx, sy: *sy });
        }
        DrawOp::Concat { matrix } => {
            b.push(DrawOp::Concat { matrix: *matrix });
        }

        // ── Paint state setters ───────────────────────────────────────
        DrawOp::SetFillStyle { color } => {
            state.fill_color = *color;
            b.push(DrawOp::SetFillStyle { color: *color });
        }
        DrawOp::SetStrokeStyle { color } => {
            state.stroke_color = *color;
            b.push(DrawOp::SetStrokeStyle { color: *color });
        }
        DrawOp::SetLineWidth { width } => {
            state.line_width = *width;
            b.push(DrawOp::SetLineWidth { width: *width });
        }
        DrawOp::SetLineCap { cap } => {
            state.line_cap = *cap;
            b.push(DrawOp::SetLineCap { cap: *cap });
        }
        DrawOp::SetLineJoin { join } => {
            state.line_join = *join;
            b.push(DrawOp::SetLineJoin { join: *join });
        }
        DrawOp::SetLineDash { intervals, phase } => {
            state.line_dash = None;
            state.line_dash_phase = *phase;
            b.push(DrawOp::SetLineDash {
                intervals: *intervals,
                phase: *phase,
            });
        }
        DrawOp::ClearLineDash => {
            state.line_dash = None;
            state.line_dash_phase = 0.0;
            b.push(DrawOp::ClearLineDash);
        }
        DrawOp::SetGlobalAlpha { alpha } => {
            state.global_alpha = alpha.clamp(0.0, 1.0);
            b.push(DrawOp::SetGlobalAlpha {
                alpha: state.global_alpha,
            });
        }
        DrawOp::SetAntiAlias { enabled } => {
            state.anti_alias = *enabled;
            b.push(DrawOp::SetAntiAlias { enabled: *enabled });
        }

        // ── Clear ─────────────────────────────────────────────────────
        DrawOp::Clear { color } => {
            b.push(DrawOp::Clear { color: *color });
        }

        // ── Path ops (pushed as-is; paint state managed by executor) ──
        DrawOp::BeginPath => {
            b.push(DrawOp::BeginPath);
        }
        DrawOp::Path(path_op) => {
            b.push(DrawOp::Path(path_op.clone()));
        }
        DrawOp::FillPath => {
            b.push(DrawOp::FillPath);
        }
        DrawOp::StrokePath => {
            b.push(DrawOp::StrokePath);
        }
        DrawOp::ClipPath { anti_alias } => {
            b.push(DrawOp::ClipPath {
                anti_alias: *anti_alias,
            });
        }

        // ── Paint-bearing ops with sentinel resolution ────────────────
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint,
        } if paint.0 == SENTINEL_FILL => {
            let paint_id = b.intern_paint(state.fill_paint_spec());
            b.push(DrawOp::Arc {
                rect: *rect,
                start: *start,
                sweep: *sweep,
                use_center: *use_center,
                paint: paint_id,
            });
        }
        DrawOp::Points {
            mode,
            points,
            paint,
        } if paint.0 == SENTINEL_STROKE => {
            let paint_id = b.intern_paint(state.stroke_paint_spec());
            b.push(DrawOp::Points {
                mode: *mode,
                points: *points,
                paint: paint_id,
            });
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } if paint.0 == SENTINEL_FILL => {
            let paint_id = b.intern_paint(state.fill_paint_spec());
            b.push(DrawOp::DRRect {
                outer: *outer,
                inner: *inner,
                paint: paint_id,
            });
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } if paint.0 == SENTINEL_STROKE => {
            let paint_id = b.intern_paint(state.stroke_paint_spec());
            b.push(DrawOp::DRRect {
                outer: *outer,
                inner: *inner,
                paint: paint_id,
            });
        }
        DrawOp::Paint { paint } if paint.0 == SENTINEL_FILL => {
            let paint_id = b.intern_paint(state.fill_paint_spec());
            b.push(DrawOp::Paint { paint: paint_id });
        }

        // ── Image ops (push as-is) ────────────────────────────────────
        DrawOp::Image { image, x, y, paint } => {
            b.push(DrawOp::Image {
                image: image.clone(),
                x: *x,
                y: *y,
                paint: *paint,
            });
        }
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint,
        } => {
            b.push(DrawOp::ImageRect {
                image: image.clone(),
                src: *src,
                dst: *dst,
                paint: *paint,
            });
        }
        DrawOp::LottieRect {
            bundle_id,
            frame,
            dst,
        } => {
            b.push(DrawOp::LottieRect {
                bundle_id: bundle_id.clone(),
                frame: *frame,
                dst: *dst,
            });
        }

        // ── Fallback: push remaining variants as-is ───────────────────
        DrawOp::Rect { rect, paint } => {
            b.push(DrawOp::Rect {
                rect: *rect,
                paint: *paint,
            });
        }
        DrawOp::RRect { rect, radii, paint } => {
            b.push(DrawOp::RRect {
                rect: *rect,
                radii: *radii,
                paint: *paint,
            });
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint,
        } => {
            b.push(DrawOp::DRRect {
                outer: *outer,
                inner: *inner,
                paint: *paint,
            });
        }
        DrawOp::Oval { rect, paint } => {
            b.push(DrawOp::Oval {
                rect: *rect,
                paint: *paint,
            });
        }
        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint,
        } => {
            b.push(DrawOp::Circle {
                cx: *cx,
                cy: *cy,
                radius: *radius,
                paint: *paint,
            });
        }
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint,
        } => {
            b.push(DrawOp::Arc {
                rect: *rect,
                start: *start,
                sweep: *sweep,
                use_center: *use_center,
                paint: *paint,
            });
        }
        DrawOp::Line {
            x0,
            y0,
            x1,
            y1,
            paint,
        } => {
            b.push(DrawOp::Line {
                x0: *x0,
                y0: *y0,
                x1: *x1,
                y1: *y1,
                paint: *paint,
            });
        }
        DrawOp::Points {
            mode,
            points,
            paint,
        } => {
            b.push(DrawOp::Points {
                mode: *mode,
                points: *points,
                paint: *paint,
            });
        }
        DrawOp::Paint { paint } => {
            b.push(DrawOp::Paint { paint: *paint });
        }
        DrawOp::DrawPath { path, paint } => {
            b.push(DrawOp::DrawPath {
                path: *path,
                paint: *paint,
            });
        }
        DrawOp::RuntimeEffect {
            effect,
            uniforms,
            children,
            dst,
        } => {
            b.push(DrawOp::RuntimeEffect {
                effect: *effect,
                uniforms: *uniforms,
                children: *children,
                dst: *dst,
            });
        }
        DrawOp::ReplayRange { range } => {
            b.push(DrawOp::ReplayRange { range: *range });
        }
        DrawOp::DrawSubtreePicture { owner_id, x, y } => {
            b.push(DrawOp::DrawSubtreePicture {
                owner_id: owner_id.clone(),
                x: *x,
                y: *y,
            });
        }
        DrawOp::ReplaySubtreePicture { subtree, x, y } => {
            b.push(DrawOp::ReplaySubtreePicture {
                subtree: *subtree,
                x: *x,
                y: *y,
            });
        }
        DrawOp::ScriptRuntimeEffect { .. } => {
            // Always routed through `execute_script_runtime_effect` from
            // `render_draw_script` (it needs the canvas's `hidden_subtree` to
            // resolve PictureSubtree children). Should never reach here.
            return Err(RenderError::InvalidArgument(
                "ScriptRuntimeEffect must be expanded by execute_script_runtime_effect".into(),
            ));
        }
    }
    Ok(())
}

// ── SVG path ─────────────────────────────────────────────────────────

fn svg_path_to_ops(svg: &str) -> Option<Vec<PathOp>> {
    let bez = BezPath::from_svg(svg).ok()?;
    let mut ops = Vec::new();
    for el in bez.elements() {
        match el {
            kurbo::PathEl::MoveTo(p) => {
                ops.push(PathOp::MoveTo {
                    x: p.x as f32,
                    y: p.y as f32,
                });
            }
            kurbo::PathEl::LineTo(p) => {
                ops.push(PathOp::LineTo {
                    x: p.x as f32,
                    y: p.y as f32,
                });
            }
            kurbo::PathEl::QuadTo(p1, p2) => {
                ops.push(PathOp::QuadTo {
                    cx: p1.x as f32,
                    cy: p1.y as f32,
                    x: p2.x as f32,
                    y: p2.y as f32,
                });
            }
            kurbo::PathEl::CurveTo(p1, p2, p3) => {
                ops.push(PathOp::CubicTo {
                    c1x: p1.x as f32,
                    c1y: p1.y as f32,
                    c2x: p2.x as f32,
                    c2y: p2.y as f32,
                    x: p3.x as f32,
                    y: p3.y as f32,
                });
            }
            kurbo::PathEl::ClosePath => {
                ops.push(PathOp::Close);
            }
        }
    }
    Some(ops)
}

pub fn render_svg_path(ctx: &mut RenderCtx, item: &SvgPathDisplayItem) -> Result<(), RenderError> {
    let dst = kurbo_rect(item.bounds);

    let scale_x = dst.width() / item.view_box[2] as f64;
    let scale_y = dst.height() / item.view_box[3] as f64;
    let scale = scale_x.min(scale_y);
    if scale <= 0.0 {
        return Ok(());
    }

    let fill_paint = item.paint.fill.as_ref().map(|fill| {
        let mut spec = background_fill_to_paint_spec(fill);
        spec.style = PaintStyle::Fill;
        spec
    });

    let stroke_paint = item.paint.stroke_width.and_then(|width| {
        if width <= 0.0 {
            return None;
        }
        let stroke_color = item.paint.stroke_color?;
        let mut spec = PaintSpec {
            fill: FillSpec::Solid(color_token_to_rgba(&stroke_color)),
            style: PaintStyle::Stroke,
            stroke: Some(StrokeSpec {
                width,
                cap: StrokeCap::Round,
                join: StrokeJoin::Round,
                miter_limit: 4.0,
            }),
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };
        if let Some(dash_len) = item.paint.stroke_dasharray
            && dash_len > 0.0
        {
            let offset = item.paint.stroke_dashoffset.unwrap_or(0.0);
            spec.path_effect = Some(PathEffectSpec::Dash {
                intervals: vec![dash_len, dash_len],
                phase: offset,
            });
        }
        Some(spec)
    });

    let builder = &mut ctx.builder;

    builder.push(DrawOp::Save);

    let scale_f32 = scale as f32;
    let draw_w = item.view_box[2] * scale_f32;
    let draw_h = item.view_box[3] * scale_f32;
    let offset_x = (dst.width() as f32 - draw_w) / 2.0;
    let offset_y = (dst.height() as f32 - draw_h) / 2.0;
    builder.push(DrawOp::Translate {
        x: dst.x0 as f32 + offset_x,
        y: dst.y0 as f32 + offset_y,
    });
    builder.push(DrawOp::Scale {
        x: scale_f32,
        y: scale_f32,
    });
    builder.push(DrawOp::Translate {
        x: -item.view_box[0],
        y: -item.view_box[1],
    });

    for path_data in &item.path_data {
        if let Some(ops) = svg_path_to_ops(path_data) {
            let encoded = EncodedPath {
                fill_type: FillType::Winding,
                ops,
            };
            let path_id = builder.intern_path(encoded);

            if let Some(ref spec) = fill_paint {
                let paint_id = builder.intern_paint(spec.clone());
                builder.push(DrawOp::DrawPath {
                    path: path_id,
                    paint: paint_id,
                });
            }
            if let Some(ref spec) = stroke_paint {
                let paint_id = builder.intern_paint(spec.clone());
                builder.push(DrawOp::DrawPath {
                    path: path_id,
                    paint: paint_id,
                });
            }
        }
    }

    builder.push(DrawOp::Restore);
    Ok(())
}

// ── Timeline ─────────────────────────────────────────────────────────

fn render_transition_overlay(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    transition: &TimelineTransitionDisplay,
) {
    let bounded_rect4 = rect_to_rect4(kurbo_rect(bounds));
    let p = transition.progress.clamp(0.0, 1.0);

    match &transition.kind {
        TransitionKind::Fade => {
            // Alpha blends into the SaveLayer created in render_timeline
        }
        TransitionKind::Slide(dir) => {
            let (dx, dy) = slide_offset(dir, bounds, 1.0 - p);
            builder.push(DrawOp::Translate { x: dx, y: dy });
        }
        TransitionKind::Wipe(dir) => {
            let clip_rect4 = wipe_clip_rect(dir, bounds, p);
            builder.push(DrawOp::BeginPath);
            builder.push(DrawOp::Path(PathOp::AddRect {
                x: clip_rect4.x,
                y: clip_rect4.y,
                width: clip_rect4.width,
                height: clip_rect4.height,
            }));
            builder.push(DrawOp::ClipPath { anti_alias: false });
        }
        TransitionKind::ClockWipe => {
            let sweep = (1.0 - p) * 360.0;
            if sweep > 0.0 {
                let overlay = PaintSpec {
                    fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
                    style: PaintStyle::Fill,
                    stroke: None,
                    anti_alias: true,
                    blend_mode: BlendMode::SrcOver,
                    image_filter: None,
                    color_filter: None,
                    mask_filter: None,
                    path_effect: None,
                };
                let paint_id = builder.intern_paint(overlay);
                builder.push(DrawOp::Save);
                builder.push(DrawOp::BeginPath);
                builder.push(DrawOp::Path(PathOp::AddRect {
                    x: bounded_rect4.x,
                    y: bounded_rect4.y,
                    width: bounded_rect4.width,
                    height: bounded_rect4.height,
                }));
                builder.push(DrawOp::ClipPath { anti_alias: false });
                builder.push(DrawOp::Arc {
                    rect: bounded_rect4,
                    start: -90.0,
                    sweep,
                    use_center: true,
                    paint: paint_id,
                });
                builder.push(DrawOp::Restore);
            }
        }
        TransitionKind::Iris => {
            let cx = bounded_rect4.x + bounded_rect4.width / 2.0;
            let cy = bounded_rect4.y + bounded_rect4.height / 2.0;
            let scale = p.max(0.001);
            builder.push(DrawOp::Translate { x: cx, y: cy });
            builder.push(DrawOp::Scale { x: scale, y: scale });
            builder.push(DrawOp::Translate { x: -cx, y: -cy });
        }
        TransitionKind::LightLeak(leak) => {
            let r = sinusoid_noise(p, leak.seed);
            let g = sinusoid_noise(p, leak.seed + 1.0);
            let b = sinusoid_noise(p, leak.seed + 2.0);
            let alpha = (1.0 - p) * 0.3 * leak.mask_scale;
            let paint = PaintSpec {
                fill: FillSpec::Solid([r, g, b, alpha]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: false,
                blend_mode: BlendMode::SrcOver,
                image_filter: None,
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Rect {
                rect: bounded_rect4,
                paint: paint_id,
            });
        }
        TransitionKind::Gl(gl) => {
            log::warn!("GL transition '{}' not supported in render layer", gl.name);
        }
    }
}

fn slide_offset(dir: &SlideDirection, bounds: DisplayRect, amount: f32) -> (f32, f32) {
    match dir {
        SlideDirection::FromLeft => (bounds.width * -amount, 0.0),
        SlideDirection::FromRight => (bounds.width * amount, 0.0),
        SlideDirection::FromTop => (0.0, bounds.height * -amount),
        SlideDirection::FromBottom => (0.0, bounds.height * amount),
    }
}

fn wipe_clip_rect(dir: &WipeDirection, bounds: DisplayRect, progress: f32) -> Rect4 {
    let p = progress;
    let x = bounds.x;
    let y = bounds.y;
    let w = bounds.width;
    let h = bounds.height;
    match dir {
        WipeDirection::FromLeft => Rect4 {
            x,
            y,
            width: w * p,
            height: h,
        },
        WipeDirection::FromRight => Rect4 {
            x: x + w * (1.0 - p),
            y,
            width: w * p,
            height: h,
        },
        WipeDirection::FromTop => Rect4 {
            x,
            y,
            width: w,
            height: h * p,
        },
        WipeDirection::FromBottom => Rect4 {
            x,
            y: y + h * (1.0 - p),
            width: w,
            height: h * p,
        },
        WipeDirection::FromTopLeft => Rect4 {
            x,
            y,
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromTopRight => Rect4 {
            x: x + w * (1.0 - p),
            y,
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromBottomLeft => Rect4 {
            x,
            y: y + h * (1.0 - p),
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromBottomRight => Rect4 {
            x: x + w * (1.0 - p),
            y: y + h * (1.0 - p),
            width: w * p,
            height: h * p,
        },
    }
}

fn sinusoid_noise(x: f32, seed: f32) -> f32 {
    ((x * std::f32::consts::TAU + seed * 12.9898).sin() * 43_758.547).fract()
}

pub fn render_timeline(ctx: &mut RenderCtx, item: &TimelineDisplayItem) -> Result<(), RenderError> {
    let rect_item = crate::display::list::RectDisplayItem {
        bounds: item.bounds,
        paint: item.paint.clone(),
    };

    if let Some(ref transition) = item.transition {
        let bounds = item.bounds;
        let rect4 = rect_to_rect4(kurbo_rect(bounds));

        // Solid white backdrop for transition compositing
        let paint = PaintSpec {
            fill: FillSpec::Solid([1.0; 4]),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };
        let paint_id = ctx.builder.intern_paint(paint);

        let layer_alpha = match &transition.kind {
            TransitionKind::Fade => transition.progress.clamp(0.0, 1.0),
            _ => 1.0,
        };

        ctx.builder.push(DrawOp::Save);
        ctx.builder.push(DrawOp::BeginPath);
        ctx.builder.push(DrawOp::Path(PathOp::AddRect {
            x: rect4.x,
            y: rect4.y,
            width: rect4.width,
            height: rect4.height,
        }));
        ctx.builder.push(DrawOp::ClipPath { anti_alias: false });

        ctx.builder.push(DrawOp::SaveLayer {
            bounds: Some(rect4),
            paint: Some(paint_id),
            alpha: layer_alpha,
        });

        render_transition_overlay(ctx.builder, bounds, transition);
        super::helpers::render_rect_with_shadows(ctx, &rect_item)?;

        ctx.builder.push(DrawOp::Restore);
        ctx.builder.push(DrawOp::Restore);
    } else {
        super::helpers::render_rect_with_shadows(ctx, &rect_item)?;
    }

    Ok(())
}

// ── Transition ───────────────────────────────────────────────────────

const LIGHT_LEAK_MASK_SKSL: &str = r#"
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
"#;

const LIGHT_LEAK_COMPOSITE_SKSL: &str = r#"
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
"#;

const MASK_EFFECT_KEY: u64 = 0xAA01_0001;
const COMPOSITE_EFFECT_KEY: u64 = 0xAA01_0002;

#[repr(C)]
#[derive(Clone, Copy)]
struct LightLeakMaskUniforms {
    evolve_progress: f32,
    retract_progress: f32,
    seed: f32,
    retract_seed: f32,
    hue_shift: f32,
    resolution: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LightLeakCompositeUniforms {
    progress: f32,
}

fn as_bytes<T: Copy>(val: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts(val as *const T as *const u8, std::mem::size_of::<T>()) }
}

pub(crate) fn render_light_leak_transition(
    ctx: &mut RenderCtx,
    from_range: DrawOpRange,
    to_range: DrawOpRange,
    progress: f32,
    params: &LightLeakTransition,
    bounds: DisplayRect,
) {
    let builder = &mut ctx.builder;
    let w = bounds.width.max(1.0).round() as u32;
    let h = bounds.height.max(1.0).round() as u32;

    let mask_effect_id = builder.intern_effect(MASK_EFFECT_KEY, LIGHT_LEAK_MASK_SKSL);
    let composite_effect_id =
        builder.intern_effect(COMPOSITE_EFFECT_KEY, LIGHT_LEAK_COMPOSITE_SKSL);

    let normalized = progress.clamp(0.0, 1.0);
    let mask_uniforms = LightLeakMaskUniforms {
        evolve_progress: (normalized * 2.0).min(1.0),
        retract_progress: (normalized * 2.0 - 1.0).max(0.0),
        seed: params.seed,
        retract_seed: params.seed + 42.0,
        hue_shift: params.hue_shift,
        resolution: [w as f32, h as f32],
    };
    let mask_uniforms_range = builder.intern_bytes(as_bytes(&mask_uniforms));

    let mask_dst = Rect4 {
        x: 0.0,
        y: 0.0,
        width: w as f32,
        height: h as f32,
    };

    {
        #[cfg(feature = "profile")]
        let _mask_span = span!(target: "render.backend", Level::TRACE, "light_leak_mask").entered();

        let mask_marker = builder.begin_range();
        builder.push(DrawOp::RuntimeEffect {
            effect: mask_effect_id,
            uniforms: mask_uniforms_range,
            children: ChildRange { start: 0, len: 0 },
            dst: mask_dst,
        });
        let mask_range = builder.end_range(mask_marker);

        let composite_uniforms = LightLeakCompositeUniforms {
            progress: normalized,
        };
        let composite_uniforms_range = builder.intern_bytes(as_bytes(&composite_uniforms));

        let child_start = builder.push_child(RuntimeEffectChildRef::Picture(from_range));
        builder.push_child(RuntimeEffectChildRef::Picture(to_range));
        builder.push_child(RuntimeEffectChildRef::Picture(mask_range));

        let dst_rect4 = Rect4 {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        };

        #[cfg(feature = "profile")]
        let _composite_span =
            span!(target: "render.backend", Level::TRACE, "light_leak_composite").entered();

        builder.push(DrawOp::RuntimeEffect {
            effect: composite_effect_id,
            uniforms: composite_uniforms_range,
            children: ChildRange {
                start: child_start,
                len: 3,
            },
            dst: dst_rect4,
        });
    }
}

// ── GL Transition ──────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct GlTransitionUniforms {
    progress: f32,
    resolution: [f32; 2],
}

pub(crate) fn render_gl_transition(
    ctx: &mut RenderCtx,
    from_range: DrawOpRange,
    to_range: DrawOpRange,
    progress: f32,
    effect: &GlTransition,
    bounds: DisplayRect,
) {
    let builder = &mut ctx.builder;
    let w = bounds.width.max(1.0).round() as u32;
    let h = bounds.height.max(1.0).round() as u32;

    let dst_rect4 = Rect4 {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    };

    let sksl = effect
        .sksl
        .as_deref()
        .map(String::from)
        .or_else(|| gl_transition::gl_transition_sksl(&effect.name).ok());
    let sksl = match sksl {
        Some(s) => s,
        None => {
            builder.push(DrawOp::ReplayRange { range: from_range });
            builder.push(DrawOp::SaveLayer {
                bounds: Some(dst_rect4),
                paint: None,
                alpha: progress,
            });
            builder.push(DrawOp::ReplayRange { range: to_range });
            builder.push(DrawOp::Restore);
            return;
        }
    };

    let cache_key = {
        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        effect.name.hash(&mut hasher);
        hasher.finish() | 0xBB00_0000_0000_0000
    };

    let effect_id = builder.intern_effect(cache_key, &sksl);

    let uniforms = GlTransitionUniforms {
        progress: progress.clamp(0.0, 1.0),
        resolution: [w as f32, h as f32],
    };
    let uniforms_range = builder.intern_bytes(as_bytes(&uniforms));

    let child_start = builder.push_child(RuntimeEffectChildRef::Picture(from_range));
    builder.push_child(RuntimeEffectChildRef::Picture(to_range));

    builder.push(DrawOp::RuntimeEffect {
        effect: effect_id,
        uniforms: uniforms_range,
        children: ChildRange {
            start: child_start,
            len: 2,
        },
        dst: dst_rect4,
    });
}

// ── Bitmap ───────────────────────────────────────────────────────────

pub(crate) fn fitted_rect(src_width: f32, src_height: f32, dst: &Rect, cover: bool) -> Rect {
    let iw = src_width as f64;
    let ih = src_height as f64;
    if iw <= 0.0 || ih <= 0.0 {
        return *dst;
    }
    let src_aspect = iw / ih;
    let dst_aspect = dst.width() / dst.height();

    let scale = if cover {
        if src_aspect > dst_aspect {
            dst.height() / ih
        } else {
            dst.width() / iw
        }
    } else if src_aspect > dst_aspect {
        dst.width() / iw
    } else {
        dst.height() / ih
    };

    let width = iw * scale;
    let height = ih * scale;
    let x = dst.x0 + (dst.width() - width) / 2.0;
    let y = dst.y0 + (dst.height() - height) / 2.0;
    Rect::new(x, y, x + width, y + height)
}

pub(crate) fn cover_src_rect(src_width: f32, src_height: f32, dst: &Rect) -> Rect {
    let fitted = fitted_rect(src_width, src_height, dst, true);
    let scale = fitted.width() / src_width as f64;
    let visible_width = dst.width() / scale;
    let visible_height = dst.height() / scale;
    let x = (src_width as f64 - visible_width) / 2.0;
    let y = (src_height as f64 - visible_height) / 2.0;
    Rect::new(x, y, x + visible_width, y + visible_height)
}

fn draw_bitmap_image(
    builder: &mut DrawOpBuilder,
    image_ref: ImageRef,
    item: &BitmapDisplayItem,
    dst: &Rect,
    src_width: f32,
    src_height: f32,
) {
    match item.object_fit {
        ObjectFit::Fill => {
            builder.push(DrawOp::ImageRect {
                image: image_ref.clone(),
                src: None,
                dst: rect_to_rect4(*dst),
                paint: None,
            });
        }
        ObjectFit::Contain => {
            let fitted = fitted_rect(src_width, src_height, dst, false);
            builder.push(DrawOp::ImageRect {
                image: image_ref.clone(),
                src: None,
                dst: rect_to_rect4(fitted),
                paint: None,
            });
        }
        ObjectFit::Cover => {
            let src = cover_src_rect(src_width, src_height, dst);
            builder.push(DrawOp::ImageRect {
                image: image_ref,
                src: Some(rect_to_rect4(src)),
                dst: rect_to_rect4(*dst),
                paint: None,
            });
        }
    }
}

pub fn render_bitmap(ctx: &mut RenderCtx, item: &BitmapDisplayItem) -> Result<(), RenderError> {
    #[cfg(feature = "profile")]
    event!(
        target: "render.draw",
        Level::TRACE,
        kind = "draw",
        name = "bitmap",
        result = "count",
        amount = 1_u64
    );

    let style = &item.paint;
    let dst = kurbo_rect(item.bounds);

    let asset_id = item.asset_id.key.clone();
    let image_ref = if let Some(timing) = item.video_timing {
        let info = ctx
            .catalog
            .video_info(&item.asset_id)
            .unwrap_or(VideoInfoMeta {
                width: item.width,
                height: item.height,
                duration_micros: None,
            });
        let request = VideoFrameRequest {
            composition_time_secs: ctx.frame_ctx.frame as f64 / ctx.frame_ctx.fps.max(1) as f64,
            timing,
        };
        if !request.is_visible() {
            return Ok(());
        }
        let time_micros = request.resolve_time_micros(&info).0;
        ImageRef::VideoFrame {
            asset_id,
            time_micros,
        }
    } else {
        ImageRef::Static { asset_id }
    };

    let src_width = item.width as f32;
    let src_height = item.height as f32;

    let builder = &mut ctx.builder;
    builder.push(DrawOp::Save);
    clip_bounds(builder, item.bounds, &style.border_radius);

    if !style.background.is_empty() {
        // 多层背景：按声明顺序从底到顶绘制。
        for bg in &style.background {
            let paint = background_fill_to_paint_spec(bg);
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Rect {
                rect: rect_to_rect4(dst),
                paint: paint_id,
            });
        }
    }

    draw_bitmap_image(builder, image_ref, item, &dst, src_width, src_height);

    for shadow in &style.inset_shadow {
        draw_inset_shadow(builder, item.bounds, &style.border_radius, shadow);
    }

    draw_node_border(
        builder,
        &dst,
        &style.border_radius,
        style.border_width,
        style.border_top_width,
        style.border_right_width,
        style.border_bottom_width,
        style.border_left_width,
        style.border_color,
        style.border_style,
        None,
    );

    builder.push(DrawOp::Restore);
    Ok(())
}

pub fn render_lottie(
    ctx: &mut RenderCtx,
    item: &crate::display::list::LottieDisplayItem,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let dst = kurbo_rect(item.bounds);

    let meta = crate::lottie::LottieMeta {
        width: item.width,
        height: item.height,
        fps: item.fps,
        in_frame: item.in_frame,
        out_frame: item.out_frame,
        dependencies: vec![],
    };
    let request = crate::media::VideoFrameRequest {
        composition_time_secs: ctx.frame_ctx.frame as f64 / ctx.frame_ctx.fps.max(1) as f64,
        timing: item.timing,
    };
    let Some(local_frame) = crate::lottie::resolve_lottie_frame(&request, &meta) else {
        return Ok(());
    };

    let src_width = item.width as f32;
    let src_height = item.height as f32;

    let builder = &mut ctx.builder;
    builder.push(DrawOp::Save);
    clip_bounds(builder, item.bounds, &style.border_radius);

    if !style.background.is_empty() {
        // 多层背景：按声明顺序从底到顶绘制。
        for bg in &style.background {
            let paint = background_fill_to_paint_spec(bg);
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Rect {
                rect: rect_to_rect4(dst),
                paint: paint_id,
            });
        }
    }

    let lottie_dst = match item.object_fit {
        ObjectFit::Fill => dst,
        ObjectFit::Contain => fitted_rect(src_width, src_height, &dst, false),
        ObjectFit::Cover => dst,
    };

    builder.push(DrawOp::LottieRect {
        bundle_id: item.bundle_id.key.clone(),
        frame: local_frame,
        dst: rect_to_rect4(lottie_dst),
    });

    for shadow in &style.inset_shadow {
        draw_inset_shadow(builder, item.bounds, &style.border_radius, shadow);
    }

    draw_node_border(
        builder,
        &lottie_dst,
        &style.border_radius,
        style.border_width,
        style.border_top_width,
        style.border_right_width,
        style.border_bottom_width,
        style.border_left_width,
        style.border_color,
        style.border_style,
        None,
    );

    builder.push(DrawOp::Restore);
    Ok(())
}

pub fn render_lottie_with_shadows(
    ctx: &mut RenderCtx,
    item: &crate::display::list::LottieDisplayItem,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let bounds = item.bounds;

    for shadow in &style.box_shadow {
        draw_box_shadow(ctx.builder, bounds, &style.border_radius, shadow);
    }

    for shadow in &style.drop_shadow {
        draw_item_drop_shadow(ctx, bounds, shadow, |ctx2| render_lottie(ctx2, item))?;
    }
    render_lottie(ctx, item)?;
    Ok(())
}

pub fn render_bitmap_with_shadows(
    ctx: &mut RenderCtx,
    item: &BitmapDisplayItem,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let bounds = item.bounds;

    for shadow in &style.box_shadow {
        draw_box_shadow(ctx.builder, bounds, &style.border_radius, shadow);
    }

    for shadow in &style.drop_shadow {
        draw_item_drop_shadow(ctx, bounds, shadow, |ctx2| render_bitmap(ctx2, item))?;
    }
    render_bitmap(ctx, item)?;

    Ok(())
}

#[cfg(test)]
mod script_runtime_effect_tests {
    // Direct unit tests for `execute_script_runtime_effect` would require a
    // fully-populated `RenderCtx` (which borrows the catalog, frame_ctx,
    // display tree, ordered scene, blob store, and builder). Coverage for
    // this path is provided by:
    //   - `parse_script_children` tests in `script::helpers`
    //   - `record_canvas_runtime_effect_pushes_script_effect` in `script::recorder::store`
    //   - the end-to-end render of `json/canvas-ripple-card.xml` and
    //     `json/profile-showcase.xml`.
}

#[cfg(test)]
mod radial_gradient_tests {
    use super::background_fill_to_fill_spec;
    use crate::canvas::paint::{FillSpec, ShaderSpec};
    use crate::style::{BackgroundFill, ColorToken};

    fn shader_for(fill: &BackgroundFill) -> ShaderSpec {
        match background_fill_to_fill_spec(fill) {
            FillSpec::Shader(s) => s,
            _ => panic!("expected shader fill"),
        }
    }

    #[test]
    fn default_center_radius_is_farthest_corner() {
        // 圆心位于中心时，farthest-corner = sqrt(0.5² + 0.5²)。
        let fill = BackgroundFill::radial_from_via_to(
            [0.5, 0.5],
            ColorToken::Red500,
            None,
            ColorToken::Blue500,
        );
        let ShaderSpec::RadialGradient {
            center,
            radius,
            stops,
            colors,
            ..
        } = shader_for(&fill)
        else {
            panic!("expected radial gradient");
        };
        assert_eq!(center, [0.5, 0.5]);
        assert!((radius - (0.5_f32 * 0.5 + 0.5 * 0.5).sqrt()).abs() < 1e-5);
        assert_eq!(stops, vec![0.0, 1.0]);
        assert_eq!(colors.len(), 2);
    }

    #[test]
    fn off_center_radius_uses_farthest_corner() {
        // 圆心 (0.2, 0.2)：最远角为 (1,1)，距离 = sqrt(0.8² + 0.8²)。
        let fill = BackgroundFill::radial_from_via_to(
            [0.2, 0.2],
            ColorToken::Red500,
            None,
            ColorToken::Blue500,
        );
        let ShaderSpec::RadialGradient { radius, .. } = shader_for(&fill) else {
            panic!("expected radial gradient");
        };
        let expected = (0.8_f32 * 0.8 + 0.8 * 0.8).sqrt();
        assert!((radius - expected).abs() < 1e-5);
    }

    #[test]
    fn via_stop_emits_three_color_stops() {
        let fill = BackgroundFill::radial_from_via_to(
            [0.5, 0.5],
            ColorToken::Red500,
            Some(ColorToken::Green500),
            ColorToken::Blue500,
        );
        let ShaderSpec::RadialGradient { stops, colors, .. } = shader_for(&fill) else {
            panic!("expected radial gradient");
        };
        assert_eq!(stops, vec![0.0, 0.5, 1.0]);
        assert_eq!(colors.len(), 3);
    }
}
