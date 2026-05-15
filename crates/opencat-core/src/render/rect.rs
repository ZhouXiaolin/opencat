use crate::canvas::paint::{
    BlendMode, BlurStyle, FillSpec, MaskFilterSpec, PaintSpec, PaintStyle,
    PathEffectSpec, StrokeCap, StrokeSpec,
};
use crate::canvas::{Canvas2D, ClipOp, Rect, RRect};
use crate::display::list::{DisplayRect, RectDisplayItem};
use crate::style::{BorderRadius, BorderStyle, BoxShadow, ColorToken, DropShadow, InsetShadow};

use super::paint_conv::{
    background_fill_to_paint_spec, box_shadow_to_mask_filter, color_token_to_rgba,
    drop_shadow_to_image_filter, inset_shadow_to_mask_filter,
};
use super::{RenderCache, RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

fn kurbo_rect_xywh(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x as f64, y as f64, (x + width) as f64, (y + height) as f64)
}

fn kurbo_rrect(rect: &Rect, radius: &BorderRadius) -> RRect {
    let radii = effective_corner_radius(rect, radius);
    let r: (f64, f64, f64, f64) = (radii[0] as f64, radii[1] as f64, radii[2] as f64, radii[3] as f64);
    RRect::new(rect.x0, rect.y0, rect.x1, rect.y1, r)
}

fn effective_corner_radius(rect: &Rect, radius: &BorderRadius) -> [f32; 4] {
    let w = rect.width() as f32;
    let h = rect.height() as f32;
    let clamp = |r: f32| {
        if r <= 0.0 { 0.0 }
        else { r.min(w / 2.0).min(h / 2.0) }
    };
    [clamp(radius.top_left), clamp(radius.top_right), clamp(radius.bottom_right), clamp(radius.bottom_left)]
}

fn spread_radius(radius: &BorderRadius, spread: f32) -> BorderRadius {
    BorderRadius {
        top_left: (radius.top_left + spread).max(0.0),
        top_right: (radius.top_right + spread).max(0.0),
        bottom_right: (radius.bottom_right + spread).max(0.0),
        bottom_left: (radius.bottom_left + spread).max(0.0),
    }
}

fn draw_box_shadow<C: Canvas2D>(
    canvas: &mut C,
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

    if radii.iter().any(|&r| r > 0.0) {
        let rrect = kurbo_rrect(&rect, &sr);
        canvas.draw_rrect(&rrect, &paint);
    } else {
        canvas.draw_rect(&rect, &paint);
    }
}

fn draw_inset_shadow<C: Canvas2D>(
    canvas: &mut C,
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

    canvas.save();
    clip_bounds(canvas, bounds, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        let rrect = kurbo_rrect(&rect, &sr);
        canvas.draw_rrect(&rrect, &paint);
    } else {
        canvas.draw_rect(&rect, &paint);
    }
    canvas.restore();
}

fn clip_bounds<C: Canvas2D>(
    canvas: &mut C,
    bounds: DisplayRect,
    border_radius: &BorderRadius,
) {
    let rect = kurbo_rect(bounds);
    let radii = effective_corner_radius(&rect, border_radius);
    if radii.iter().any(|&r| r > 0.0) {
        let rrect = kurbo_rrect(&rect, border_radius);
        canvas.clip_rrect(&rrect, ClipOp::Intersect, true);
    } else {
        canvas.clip_rect(&rect, ClipOp::Intersect, true);
    }
}

fn draw_item_drop_shadow<C: Canvas2D>(
    canvas: &mut C,
    bounds: DisplayRect,
    shadow: &DropShadow,
    draw: impl FnOnce(&mut C) -> Result<(), RenderError>,
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

    canvas.save_layer_with(Some(shadow_bounds), &paint);
    let result = draw(canvas);
    canvas.restore();
    result
}

fn apply_blur_effect(spec: &mut PaintSpec, blur_sigma: Option<f32>) {
    if let Some(sigma) = blur_sigma {
        if sigma > 0.0 {
            spec.mask_filter = Some(MaskFilterSpec::Blur {
                sigma,
                style: BlurStyle::Normal,
                respect_ctm: true,
            });
        }
    }
}

fn build_stroke_paint(color: &[f32; 4], width: f32, border_style: &BorderStyle, blur_sigma: Option<f32>) -> PaintSpec {
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

fn draw_node_border<C: Canvas2D>(
    canvas: &mut C,
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
    let Some(color) = border_color else { return; };
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
            draw_border_fill_ring(canvas, rect, radius, top_w, right_w, bottom_w, left_w, &rgba, blur_sigma);
        }
        BorderStyle::Dashed | BorderStyle::Dotted => {
            draw_per_side_borders(canvas, rect, radius, top_w, right_w, bottom_w, left_w, &rgba, &stroke_style, blur_sigma);
        }
    }
}

fn draw_border_fill_ring<C: Canvas2D>(
    canvas: &mut C,
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

    let outer_rrect = kurbo_rrect(outer_rect, outer_radius);

    if inner_right <= inner_left || inner_bottom <= inner_top {
        canvas.draw_rrect(&outer_rrect, &paint);
        return;
    }

    let inner_rect = Rect::new(inner_left, inner_top, inner_right, inner_bottom);
    let inner_radius = BorderRadius {
        top_left: (outer_radius.top_left - top_w.max(left_w)).max(0.0),
        top_right: (outer_radius.top_right - top_w.max(right_w)).max(0.0),
        bottom_right: (outer_radius.bottom_right - bottom_w.max(right_w)).max(0.0),
        bottom_left: (outer_radius.bottom_left - bottom_w.max(left_w)).max(0.0),
    };
    let inner_rrect = kurbo_rrect(&inner_rect, &inner_radius);

    canvas.draw_drrect(&outer_rrect, &inner_rrect, &paint);
}

fn draw_per_side_borders<C: Canvas2D>(
    canvas: &mut C,
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
        let x0 = if top_w == left_w && r_tl > 0.0 { left + r_tl }
            else if left_w > 0.0 { left + left_w } else { left };
        let x1 = if top_w == right_w && r_tr > 0.0 { right - r_tr }
            else if right_w > 0.0 { right - right_w } else { right };
        if x1 > x0 {
            let paint = build_stroke_paint(color, top_w, border_style, blur_sigma);
            canvas.draw_line(x0, y, x1, y, &paint);
        }
    }

    if right_w > 0.0 {
        let x = right - right_w / 2.0;
        let y0 = if right_w == top_w && r_tr > 0.0 { top + r_tr }
            else if top_w > 0.0 { top + top_w } else { top };
        let y1 = if right_w == bottom_w && r_br > 0.0 { bottom - r_br }
            else if bottom_w > 0.0 { bottom - bottom_w } else { bottom };
        if y1 > y0 {
            let paint = build_stroke_paint(color, right_w, border_style, blur_sigma);
            canvas.draw_line(x, y0, x, y1, &paint);
        }
    }

    if bottom_w > 0.0 {
        let y = bottom - bottom_w / 2.0;
        let x0 = if bottom_w == left_w && r_bl > 0.0 { left + r_bl }
            else if left_w > 0.0 { left + left_w } else { left };
        let x1 = if bottom_w == right_w && r_br > 0.0 { right - r_br }
            else if right_w > 0.0 { right - right_w } else { right };
        if x1 > x0 {
            let paint = build_stroke_paint(color, bottom_w, border_style, blur_sigma);
            canvas.draw_line(x0, y, x1, y, &paint);
        }
    }

    if left_w > 0.0 {
        let x = left + left_w / 2.0;
        let y0 = if left_w == top_w && r_tl > 0.0 { top + r_tl }
            else if top_w > 0.0 { top + top_w } else { top };
        let y1 = if left_w == bottom_w && r_bl > 0.0 { bottom - r_bl }
            else if bottom_w > 0.0 { bottom - bottom_w } else { bottom };
        if y1 > y0 {
            let paint = build_stroke_paint(color, left_w, border_style, blur_sigma);
            canvas.draw_line(x, y0, x, y1, &paint);
        }
    }

    let draw_corner_arc = |canvas: &mut C, cx: f32, cy: f32, corner_r: f32, width: f32, start_deg: f32| {
        let arc_r = (corner_r - width / 2.0).max(0.0);
        if arc_r <= 0.0 { return; }
        let oval = kurbo_rect_xywh(cx - arc_r, cy - arc_r, 2.0 * arc_r, 2.0 * arc_r);
        let paint = build_stroke_paint(color, width, border_style, blur_sigma);
        canvas.draw_arc(&oval, start_deg, 90.0, false, &paint);
    };

    if r_tl > 0.0 && top_w > 0.0 && top_w == left_w {
        draw_corner_arc(canvas, left + r_tl, top + r_tl, r_tl, top_w, 180.0);
    }
    if r_tr > 0.0 && top_w > 0.0 && top_w == right_w {
        draw_corner_arc(canvas, right - r_tr, top + r_tr, r_tr, top_w, 270.0);
    }
    if r_br > 0.0 && bottom_w > 0.0 && bottom_w == right_w {
        draw_corner_arc(canvas, right - r_br, bottom - r_br, r_br, bottom_w, 0.0);
    }
    if r_bl > 0.0 && bottom_w > 0.0 && bottom_w == left_w {
        draw_corner_arc(canvas, left + r_bl, bottom - r_bl, r_bl, bottom_w, 90.0);
    }
}

pub fn render_rect<C: Canvas2D>(
    canvas: &mut C,
    item: &RectDisplayItem,
    _ctx: &RenderCtx<C>,
    _cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let has_any_border = style.border_width.is_some()
        || style.border_top_width.is_some()
        || style.border_right_width.is_some()
        || style.border_bottom_width.is_some()
        || style.border_left_width.is_some();
    if style.background.is_none() && !has_any_border && style.inset_shadow.is_none() {
        return Ok(());
    }

    let bounds = item.bounds;
    let rect = kurbo_rect(bounds);
    let radii = effective_corner_radius(&rect, &style.border_radius);
    let has_radius = radii.iter().any(|&r| r > 0.0);

    canvas.save();

    if let Some(ref background) = style.background {
        let paint_spec = background_fill_to_paint_spec(background);
        if has_radius {
            let rrect = kurbo_rrect(&rect, &style.border_radius);
            canvas.draw_rrect(&rrect, &paint_spec);
        } else {
            canvas.draw_rect(&rect, &paint_spec);
        }
    }

    if let Some(ref shadow) = style.inset_shadow {
        draw_inset_shadow(canvas, bounds, &style.border_radius, shadow);
    }

    draw_node_border(
        canvas, &rect, &style.border_radius,
        style.border_width, style.border_top_width, style.border_right_width,
        style.border_bottom_width, style.border_left_width,
        style.border_color, style.border_style, style.blur_sigma,
    );

    canvas.restore();
    Ok(())
}

pub fn render_rect_with_shadows<C: Canvas2D>(
    canvas: &mut C,
    item: &RectDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let bounds = item.bounds;

    if let Some(ref shadow) = style.box_shadow {
        draw_box_shadow(canvas, bounds, &style.border_radius, shadow);
    }

    if let Some(ref shadow) = style.drop_shadow {
        draw_item_drop_shadow(canvas, bounds, shadow, |c| {
            render_rect(c, item, ctx, cache)
        })?;
    }
    render_rect(canvas, item, ctx, cache)?;

    Ok(())
}
