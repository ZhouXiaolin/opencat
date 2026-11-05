use crate::canvas::Rect;
use crate::canvas::paint::{
    BlendMode, BlurStyle, FillSpec, MaskFilterSpec, PaintSpec, PaintStyle,
    PathEffectSpec, StrokeCap, StrokeJoin, StrokeSpec, TileMode,
};
use crate::display::list::{
    BitmapDisplayItem, DisplayRect, RectDisplayItem,
    SvgDisplayItem, SvgPathDisplayItem,
};
use crate::ir::draw_op::{
    ColorU8, DRRectSpec, DrawOp, LineCap, LineJoin, Radii4, Rect4,
};
use crate::ir::draw_types::ImageRef;
use crate::ir::draw_types::{
    EncodedPath, FillType, PaintId, PathOp,
};
use crate::media::VideoFrameRequest;
use crate::probe::catalog::VideoInfoMeta;
use crate::render::builder::DrawOpBuilder;
use crate::style::{
    BackgroundFill, BorderRadius, BorderStyle, ColorToken,
    ObjectFit, RadialExtent, RadialShape,
};

use super::RenderError;
use super::ctx::RenderCtx;


use super::canvas_lower::svg_path_to_ops;
use super::gradient::{
    GradientBox, RadialParams, arbitrary_gradient_to_shader_spec, build_linear_gradient,
    layer_tile_rect,
    build_radial_gradient,
};
use super::shadow_filter::{draw_box_shadow, draw_inset_shadow, draw_item_drop_shadow};

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

/// Convert `BackgroundFill` to `PaintSpec` (fill-only, no stroke) for a node
/// whose painted box is `rect` (device coordinates). Gradients are mapped onto
/// `rect`, so the rect size/position participates in the (interned) paint spec.
pub fn background_fill_to_paint_spec(
    fill: &BackgroundFill,
    rect: Rect,
    blend_mode: BlendMode,
) -> PaintSpec {
    PaintSpec {
        fill: background_fill_to_fill_spec(fill, rect),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    }
}

/// Convert `BackgroundFill` to `FillSpec` for a node box `rect`.
pub fn background_fill_to_fill_spec(fill: &BackgroundFill, rect: Rect) -> FillSpec {
    match fill {
        BackgroundFill::Solid { color } => FillSpec::Solid(color_token_to_rgba(color)),
        BackgroundFill::LinearGradient { direction, stops } => {
            let box_ = GradientBox::from_rect(&rect);
            let shader = build_linear_gradient(None, Some(*direction), stops, false, TileMode::Clamp, box_);
            FillSpec::Shader(shader)
        }
        BackgroundFill::RadialGradient { center, stops } => {
            let box_ = GradientBox::from_rect(&rect);
            let params = RadialParams {
                center: *center,
                shape: RadialShape::Circle,
                radii: None,
                extent: RadialExtent::FarthestCorner,
            };
            let shader = build_radial_gradient(box_, &params, stops, false);
            FillSpec::Shader(shader)
        }
        BackgroundFill::ArbitraryGradient { gradient } => {
            let shader = arbitrary_gradient_to_shader_spec(gradient, rect);
            FillSpec::Shader(shader)
        }
    }
}

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

pub(crate) fn effective_corner_radius(rect: &Rect, radius: &BorderRadius) -> [f32; 4] {
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

pub(crate) fn spread_radius(radius: &BorderRadius, spread: f32) -> BorderRadius {
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

pub(crate) fn push_draw_rrect(builder: &mut DrawOpBuilder, rect: Rect, radii: [f32; 4], paint_id: PaintId) {
    builder.push(DrawOp::RRect {
        rect: rect_to_rect4(rect),
        radii: radii_to_radii4(radii),
        paint: paint_id,
    });
}

pub fn clip_bounds(builder: &mut DrawOpBuilder, bounds: DisplayRect, border_radius: &BorderRadius) {
    let rect = kurbo_rect(bounds);
    let radii = effective_corner_radius(&rect, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        push_rrect_path(builder, rect_to_rect4(rect), radii_to_radii4(radii));
        builder.push(DrawOp::ClipPath { anti_alias: true, fill_type: Default::default() });
    } else {
        let r4 = rect_to_rect4(rect);
        builder.push(DrawOp::BeginPath);
        builder.push(DrawOp::Path(PathOp::AddRect {
            x: r4.x,
            y: r4.y,
            width: r4.width,
            height: r4.height,
        }));
        builder.push(DrawOp::ClipPath { anti_alias: true, fill_type: Default::default() });
    }
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

/// Paint a node's background layers onto its box (CSS order: the first layer
/// is closest to the viewer, so paint it last), honouring the corner radius.
/// Shared by `render_rect` and the inline-box background painted behind text
/// glyphs (`TextDisplayItem::background`).
pub(crate) fn paint_background_box(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    background: &[BackgroundFill],
    border_radius: &BorderRadius,
    blend_mode: BlendMode,
) {
    if background.is_empty() {
        return;
    }
    let rect = kurbo_rect(bounds);
    let radii = effective_corner_radius(&rect, border_radius);
    let has_radius = radii.iter().any(|&r| r > 0.0);
    for background in background.iter().rev() {
        // 带 `background-position`/`size` 的层只画被定位出来的那一块。
        let tile = layer_tile_rect(background, rect).unwrap_or(rect);
        let paint_spec = background_fill_to_paint_spec(background, tile, blend_mode);
        let paint_id = builder.intern_paint(paint_spec);
        if has_radius && tile == rect {
            push_draw_rrect(builder, tile, radii, paint_id);
        } else {
            builder.push(DrawOp::Rect {
                rect: rect_to_rect4(tile),
                paint: paint_id,
            });
        }
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

    // `backdrop-blur` is emitted once, at the composite/apply level
    // (`dispatch::save_composite_layer`, driven by `DisplayNode::backdrop_blur_sigma`).
    // Re-emitting it here would blur the backdrop twice for every element that
    // also paints a background, so this item-level pass deliberately does not.

    if !style.background.is_empty() {
        // CSS 多层背景：第一层最靠近观察者（最上），从最后一层画到第一层。
        for background in style.background.iter().rev() {
            let tile = layer_tile_rect(background, rect).unwrap_or(rect);
            let paint_spec = background_fill_to_paint_spec(background, tile, style.blend_mode);
            let paint_id = builder.intern_paint(paint_spec);
            if has_radius && tile == rect {
                push_draw_rrect(builder, tile, radii, paint_id);
            } else {
                builder.push(DrawOp::Rect {
                    rect: rect_to_rect4(tile),
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
pub(crate) const SENTINEL_FILL: u32 = u32::MAX;
/// Sentinel: the stored DrawOp carries PaintId(u32::MAX - 1) meaning "resolve to stroke paint".
pub(crate) const SENTINEL_STROKE: u32 = u32::MAX - 1;

pub(crate) struct LocalPaintState {
    pub(crate) fill_color: ColorU8,
    pub(crate) stroke_color: ColorU8,
    pub(crate) line_width: f32,
    pub(crate) line_cap: LineCap,
    pub(crate) line_join: LineJoin,
    pub(crate) line_dash: Option<Vec<f32>>,
    pub(crate) line_dash_phase: f32,
    pub(crate) global_alpha: f32,
    pub(crate) anti_alias: bool,
    /// Canvas 2D shadow state (sticky until overwritten). Translated here,
    /// in core, into a `SaveLayer` + `DropShadow` wrapper — never emitted.
    pub(crate) shadow: Option<CanvasShadow>,
}

/// Canvas 2D shadow (`shadowColor` / `shadowBlur` / `shadowOffsetX/Y`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct CanvasShadow {
    pub(crate) color: ColorU8,
    pub(crate) blur: f32,
    pub(crate) dx: f32,
    pub(crate) dy: f32,
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
            shadow: None,
            anti_alias: true,
        }
    }
}

impl LocalPaintState {
    pub(crate) fn fill_paint_spec(&self) -> PaintSpec {
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

    pub(crate) fn stroke_paint_spec(&self) -> PaintSpec {
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

pub(crate) fn rect4_xywh(x: f32, y: f32, w: f32, h: f32) -> Rect4 {
    Rect4 {
        x,
        y,
        width: w,
        height: h,
    }
}


/// `<svg src>` / `<svg inline>`：加载（缓存）树并展开为 DrawOp 段（PLAN §4）。
pub fn render_svg(ctx: &mut RenderCtx, item: &SvgDisplayItem) -> Result<(), RenderError> {
    let tree = match &item.inline {
        Some(markup) => crate::render::svg::load_tree_markup(markup),
        None => crate::render::svg::load_tree(&item.src),
    };
    let Some(tree) = tree else {
        return Err(RenderError::MissingResource(match &item.inline {
            Some(_) => "inline svg markup is invalid".to_string(),
            None => format!("svg file not found or invalid: {}", item.src.display()),
        }));
    };
    let b = kurbo_rect(item.bounds);
    crate::render::svg::emit_svg(
        ctx.builder,
        &tree,
        Rect4 {
            x: b.x0 as f32,
            y: b.y0 as f32,
            width: b.width() as f32,
            height: b.height() as f32,
        },
    );
    Ok(())
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
        let mut spec = background_fill_to_paint_spec(fill, dst, BlendMode::SrcOver);
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
        // Dash 是「实线段 + 空白段」两值（CSS 语义）：单值写法两边相等，
        // 画成等距虚线；两段式是 DrawSVG 的「描边逐渐出现」——实线段是已
        // 画出的长度。dash=0 时什么都不画，正是描边动画的起点。
        if let Some(dash) = item.paint.stroke_dasharray
            && dash.gap > 0.0
        {
            let offset = item.paint.stroke_dashoffset.unwrap_or(0.0);
            spec.path_effect = Some(PathEffectSpec::Dash {
                intervals: vec![dash.dash.max(0.0), dash.gap],
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
                color_matrix: None,
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
            let paint = background_fill_to_paint_spec(bg, dst, BlendMode::SrcOver);
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
            let paint = background_fill_to_paint_spec(bg, dst, BlendMode::SrcOver);
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






