use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle, StrokeSpec};
use crate::canvas::{Canvas2D, ClipOp, FillType, PathBuilder, Rect};
use crate::display::list::{DisplayRect, DrawScriptDisplayItem};

use super::bitmap::{cover_src_rect, fitted_rect};
use super::script_conv::{script_color_with_alpha, to_fill_spec, script_line_cap, script_line_join, script_point_mode};
use super::state::DrawScriptPaintState;
use super::{RenderCache, RenderCtx, RenderError};

use crate::scene::script::CanvasCommand;
use crate::style::ObjectFit;

/// 路径状态：直接持有平台 PathBuilder，在 draw/clip 时 finish 出 Path。
///
/// - `builder`：活跃的 PathBuilder，path 命令直接操作它。
/// - `cached_path`：finish 后缓存的 Path，供 FillPath/StrokePath/ClipPath 复用。
///   任何新的 path 命令会使缓存失效（置 None）。
pub struct PathState<C: Canvas2D> {
    builder: Option<C::PathBuilder>,
    cached_path: Option<C::Path>,
}

impl<C: Canvas2D> PathState<C> {
    fn new() -> Self {
        Self { builder: None, cached_path: None }
    }

    fn begin(&mut self, canvas: &C) {
        self.builder = Some(canvas.create_path_builder(FillType::Winding));
        self.cached_path = None;
    }

    fn builder(&mut self, canvas: &C) -> &mut C::PathBuilder {
        if self.builder.is_none() {
            self.builder = Some(canvas.create_path_builder(FillType::Winding));
        }
        self.cached_path = None;
        self.builder.as_mut().unwrap()
    }

    fn get_path(&mut self, canvas: &C) -> &C::Path {
        if self.cached_path.is_none() {
            if let Some(b) = self.builder.take() {
                self.cached_path = Some(b.finish());
            } else {
                self.cached_path = Some(canvas.create_path_builder(FillType::Winding).finish());
            }
        }
        self.cached_path.as_ref().unwrap()
    }
}

fn kurbo_rect_xywh(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x as f64, y as f64, (x + width) as f64, (y + height) as f64)
}

fn kurbo_rrect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> crate::canvas::RRect {
    let r = radius as f64;
    crate::canvas::RRect::new(
        x as f64, y as f64, (x + width) as f64, (y + height) as f64, r,
    )
}

fn display_rect_to_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

pub fn render_draw_script<C: Canvas2D>(
    canvas: &mut C,
    item: &DrawScriptDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let mut state = DrawScriptPaintState::default();
    let mut path_state = PathState::<C>::new();
    let clip_rect = display_rect_to_rect(item.bounds);

    canvas.save();
    canvas.clip_rect(&clip_rect, ClipOp::Intersect, true);

    for command in &item.commands {
        execute_canvas_command(canvas, command, &mut state, &mut path_state, ctx, cache)?;
    }

    canvas.restore();
    Ok(())
}

pub fn execute_canvas_command<C: Canvas2D>(
    canvas: &mut C,
    cmd: &CanvasCommand,
    state: &mut DrawScriptPaintState,
    path_state: &mut PathState<C>,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    match cmd {
        CanvasCommand::Save => { canvas.save(); }
        CanvasCommand::SaveLayer { alpha, bounds } => {
            let layer_alpha = (state.global_alpha * *alpha).clamp(0.0, 1.0);
            let paint = PaintSpec {
                fill: FillSpec::Solid([1.0, 1.0, 1.0, layer_alpha]),
                style: PaintStyle::Fill, stroke: None, anti_alias: true,
                blend_mode: BlendMode::SrcOver, image_filter: None,
                color_filter: None, mask_filter: None, path_effect: None,
            };
            let bounds_rect: Option<Rect> = bounds.map(|b| kurbo_rect_xywh(b[0], b[1], b[2], b[3]));
            canvas.save_layer_with(bounds_rect, &paint);
        }
        CanvasCommand::Restore => { canvas.restore(); }
        CanvasCommand::RestoreToCount { count } => { canvas.restore_to_count(*count); }
        CanvasCommand::SetFillStyle { color } => {
            state.fill_style.fill = to_fill_spec(*color, state.global_alpha);
        }
        CanvasCommand::SetStrokeStyle { color } => {
            state.stroke_style.fill = to_fill_spec(*color, state.global_alpha);
            state.stroke_style.style = PaintStyle::Stroke;
        }
        CanvasCommand::SetLineWidth { width } => { state.line_width = *width; }
        CanvasCommand::SetLineCap { cap } => { state.line_cap = script_line_cap(*cap); }
        CanvasCommand::SetLineJoin { join } => { state.line_join = script_line_join(*join); }
        CanvasCommand::SetLineDash { intervals, phase } => {
            state.line_dash = Some(intervals.clone());
            state.line_dash_phase = *phase;
        }
        CanvasCommand::ClearLineDash => {
            state.line_dash = None;
            state.line_dash_phase = 0.0;
        }
        CanvasCommand::SetGlobalAlpha { alpha } => { state.global_alpha = *alpha; }
        CanvasCommand::SetAntiAlias { enabled } => { state.anti_alias = *enabled; }
        CanvasCommand::Translate { x, y } => { canvas.translate(*x, *y); }
        CanvasCommand::Scale { x, y } => { canvas.scale(*x, *y); }
        CanvasCommand::Rotate { degrees } => { canvas.rotate(*degrees, 0.0, 0.0); }
        CanvasCommand::ClipRect { x, y, width, height, anti_alias } => {
            let r = kurbo_rect_xywh(*x, *y, *width, *height);
            canvas.clip_rect(&r, ClipOp::Intersect, *anti_alias);
        }
        CanvasCommand::Clear { color } => {
            let rgba = match color {
                Some(c) => script_color_with_alpha(*c, state.global_alpha),
                None => [0.0, 0.0, 0.0, 0.0],
            };
            let paint = PaintSpec {
                fill: FillSpec::Solid(rgba),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: false,
                blend_mode: BlendMode::Src,
                image_filter: None,
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            canvas.draw_paint(&paint);
        }
        CanvasCommand::DrawPaint { color, anti_alias } => {
            let mut paint = state.fill_paint_spec();
            paint.fill = FillSpec::Solid(script_color_with_alpha(*color, state.global_alpha));
            paint.anti_alias = *anti_alias;
            canvas.draw_paint(&paint);
        }
        CanvasCommand::DrawText { text, x, y, color, anti_alias, stroke, stroke_width, font_size, .. } => {
            let mut paint = if *stroke {
                let mut p = state.stroke_paint_spec();
                p.stroke = Some(StrokeSpec { width: (*stroke_width).max(0.0), ..p.stroke.clone().unwrap_or_default() });
                p
            } else {
                state.fill_paint_spec()
            };
            paint.fill = FillSpec::Solid(script_color_with_alpha(*color, state.global_alpha));
            paint.anti_alias = *anti_alias;
            canvas.draw_simple_text(text, *x, *y, *font_size, &paint);
        }
        CanvasCommand::FillRect { x, y, width, height, color } => {
            let mut paint = state.fill_paint_spec();
            paint.fill = FillSpec::Solid(script_color_with_alpha(*color, state.global_alpha));
            canvas.draw_rect(&kurbo_rect_xywh(*x, *y, *width, *height), &paint);
        }
        CanvasCommand::FillRRect { x, y, width, height, radius } => {
            let paint = state.fill_paint_spec();
            canvas.draw_rrect(&kurbo_rrect(*x, *y, *width, *height, *radius), &paint);
        }
        CanvasCommand::StrokeRect { x, y, width, height, color, stroke_width } => {
            let mut paint = state.stroke_paint_spec();
            paint.fill = FillSpec::Solid(script_color_with_alpha(*color, state.global_alpha));
            paint.stroke = Some(StrokeSpec { width: *stroke_width, ..paint.stroke.clone().unwrap_or_default() });
            canvas.draw_rect(&kurbo_rect_xywh(*x, *y, *width, *height), &paint);
        }
        CanvasCommand::StrokeRRect { x, y, width, height, radius } => {
            let paint = state.stroke_paint_spec();
            canvas.draw_rrect(&kurbo_rrect(*x, *y, *width, *height, *radius), &paint);
        }
        CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
            let paint = state.stroke_paint_spec();
            canvas.draw_line(*x0, *y0, *x1, *y1, &paint);
        }
        CanvasCommand::FillCircle { cx, cy, radius } => {
            let paint = state.fill_paint_spec();
            canvas.draw_circle(*cx, *cy, *radius, &paint);
        }
        CanvasCommand::StrokeCircle { cx, cy, radius } => {
            let paint = state.stroke_paint_spec();
            canvas.draw_circle(*cx, *cy, *radius, &paint);
        }
        CanvasCommand::BeginPath => { path_state.begin(canvas); }
        CanvasCommand::MoveTo { x, y } => { path_state.builder(canvas).move_to(*x, *y); }
        CanvasCommand::LineTo { x, y } => { path_state.builder(canvas).line_to(*x, *y); }
        CanvasCommand::QuadTo { cx, cy, x, y } => { path_state.builder(canvas).quad_to(*cx, *cy, *x, *y); }
        CanvasCommand::CubicTo { c1x, c1y, c2x, c2y, x, y } => {
            path_state.builder(canvas).cubic_to(*c1x, *c1y, *c2x, *c2y, *x, *y);
        }
        CanvasCommand::ClosePath => { path_state.builder(canvas).close(); }
        CanvasCommand::AddRectPath { x, y, width, height } => { path_state.builder(canvas).add_rect(*x, *y, *width, *height); }
        CanvasCommand::AddRRectPath { x, y, width, height, radius } => {
            path_state.builder(canvas).add_rrect(*x, *y, *width, *height, *radius);
        }
        CanvasCommand::AddOvalPath { x, y, width, height } => { path_state.builder(canvas).add_oval(*x, *y, *width, *height); }
        CanvasCommand::AddArcPath { x, y, width, height, start_angle, sweep_angle } => {
            path_state.builder(canvas).add_arc(*x, *y, *width, *height, *start_angle, *sweep_angle);
        }
        CanvasCommand::FillPath => {
            let paint = state.fill_paint_spec();
            let path = path_state.get_path(canvas);
            canvas.draw_path(path, &paint);
        }
        CanvasCommand::StrokePath => {
            let paint = state.stroke_paint_spec();
            let path = path_state.get_path(canvas);
            canvas.draw_path(path, &paint);
        }
        CanvasCommand::DrawImage { asset_id, x, y, width, height, src_rect, anti_alias, object_fit, .. } => {
            let (image, img_w, img_h) = load_image_for_script(canvas, asset_id, ctx, cache)?;
            let dst = kurbo_rect_xywh(*x, *y, *width, *height);
            let paint = PaintSpec {
                fill: FillSpec::Solid([1.0; 4]), style: PaintStyle::Fill, stroke: None,
                anti_alias: *anti_alias, blend_mode: BlendMode::SrcOver,
                image_filter: None, color_filter: None, mask_filter: None, path_effect: None,
            };
            if let Some(src) = src_rect {
                let src_r = kurbo_rect_xywh(src[0], src[1], src[2], src[3]);
                canvas.draw_image_rect(&image, Some(&src_r), &dst, Some(&paint));
            } else {
                match object_fit {
                    ObjectFit::Fill => { canvas.draw_image_rect(&image, None, &dst, Some(&paint)); }
                    ObjectFit::Contain => {
                        let fitted = fitted_rect(img_w as f32, img_h as f32, &dst, false);
                        canvas.draw_image_rect(&image, None, &fitted, Some(&paint));
                    }
                    ObjectFit::Cover => {
                        let src = cover_src_rect(img_w as f32, img_h as f32, &dst);
                        canvas.draw_image_rect(&image, Some(&src), &dst, Some(&paint));
                    }
                }
            }
        }
        CanvasCommand::DrawArc { cx, cy, rx, ry, start_angle, sweep_angle, use_center } => {
            let paint = state.fill_paint_spec();
            let oval = kurbo_rect_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0);
            canvas.draw_arc(&oval, *start_angle, *sweep_angle, *use_center, &paint);
        }
        CanvasCommand::StrokeArc { cx, cy, rx, ry, start_angle, sweep_angle } => {
            let paint = state.stroke_paint_spec();
            let oval = kurbo_rect_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0);
            canvas.draw_arc(&oval, *start_angle, *sweep_angle, false, &paint);
        }
        CanvasCommand::FillOval { cx, cy, rx, ry } => {
            let paint = state.fill_paint_spec();
            canvas.draw_oval(&kurbo_rect_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0), &paint);
        }
        CanvasCommand::StrokeOval { cx, cy, rx, ry } => {
            let paint = state.stroke_paint_spec();
            canvas.draw_oval(&kurbo_rect_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0), &paint);
        }
        CanvasCommand::ClipPath { anti_alias } => {
            let path = path_state.get_path(canvas).clone();
            canvas.clip_path(&path, ClipOp::Intersect, *anti_alias);
            path_state.begin(canvas);
        }
        CanvasCommand::ClipRRect { x, y, width, height, radius, anti_alias } => {
            let rrect = kurbo_rrect(*x, *y, *width, *height, *radius);
            canvas.clip_rrect(&rrect, ClipOp::Intersect, *anti_alias);
        }
        CanvasCommand::DrawPoints { mode, points } => {
            let paint = state.stroke_paint_spec();
            canvas.draw_points(script_point_mode(*mode), points, &paint);
        }
        CanvasCommand::FillDRRect { outer_x, outer_y, outer_width, outer_height, outer_radius,
                                     inner_x, inner_y, inner_width, inner_height, inner_radius } => {
            let paint = state.fill_paint_spec();
            let outer = kurbo_rrect(*outer_x, *outer_y, *outer_width, *outer_height, *outer_radius);
            let inner = kurbo_rrect(*inner_x, *inner_y, *inner_width, *inner_height, *inner_radius);
            canvas.draw_drrect(&outer, &inner, &paint);
        }
        CanvasCommand::StrokeDRRect { outer_x, outer_y, outer_width, outer_height, outer_radius,
                                       inner_x, inner_y, inner_width, inner_height, inner_radius } => {
            let paint = state.stroke_paint_spec();
            let outer = kurbo_rrect(*outer_x, *outer_y, *outer_width, *outer_height, *outer_radius);
            let inner = kurbo_rrect(*inner_x, *inner_y, *inner_width, *inner_height, *inner_radius);
            canvas.draw_drrect(&outer, &inner, &paint);
        }
        CanvasCommand::Skew { sx, sy } => { canvas.skew(*sx, *sy); }
        CanvasCommand::DrawImageSimple { asset_id, x, y, anti_alias, .. } => {
            let (image, _, _) = load_image_for_script(canvas, asset_id, ctx, cache)?;
            let paint = PaintSpec {
                fill: FillSpec::Solid([1.0; 4]), style: PaintStyle::Fill, stroke: None,
                anti_alias: *anti_alias, blend_mode: BlendMode::SrcOver,
                image_filter: None, color_filter: None, mask_filter: None, path_effect: None,
            };
            canvas.draw_image(&image, *x, *y, Some(&paint));
        }
        CanvasCommand::Concat { matrix } => { canvas.concat(matrix); }
    }
    Ok(())
}

fn load_image_for_script<C: Canvas2D>(
    canvas: &C,
    asset_id: &str,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(C::Image, u32, u32), RenderError> {
    let key = asset_id.to_string();
    {
        let mut lru = cache.images.borrow_mut();
        if let Some(Some(img)) = lru.get_cloned(&key) {
            let sizes = cache.image_sizes.borrow();
            if let Some(&(w, h)) = sizes.get(&key) {
                return Ok((img, w, h));
            }
        }
    }
    let asset_id_obj = crate::resource::asset_id::AssetId(asset_id.to_string());
    let encoded = ctx.blob_store.and_then(|store| store.read(&asset_id_obj))
        .ok_or_else(|| RenderError::MissingResource(format!("missing asset blob for {}", asset_id)))?;
    let image = canvas.make_image_from_encoded(&encoded)
        .ok_or_else(|| RenderError::MissingResource(format!("failed to decode image: {}", asset_id)))?;
    let dims = read_image_dimensions_from_encoded(&encoded).unwrap_or((0, 0));
    {
        let mut lru = cache.images.borrow_mut();
        let report = lru.insert(key.clone(), Some(image.clone()));
        drop(report);
    }
    cache.image_sizes.borrow_mut().insert(key, dims);
    Ok((image, dims.0, dims.1))
}

fn read_image_dimensions_from_encoded(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 {
        return None;
    }
    if data.len() >= 24 && &data[1..4] == b"PNG" {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((w, h));
    }
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        let mut pos = 2usize;
        while pos + 4 <= data.len() {
            if data[pos] != 0xFF {
                return None;
            }
            let marker = data[pos + 1];
            if marker == 0xC0 || marker == 0xC2 {
                if pos + 9 > data.len() {
                    return None;
                }
                let h = u16::from_be_bytes([data[pos + 5], data[pos + 6]]) as u32;
                let w = u16::from_be_bytes([data[pos + 7], data[pos + 8]]) as u32;
                return Some((w, h));
            }
            if pos + 4 > data.len() {
                return None;
            }
            let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            if seg_len < 2 {
                return None;
            }
            pos += seg_len as usize;
        }
    }
    None
}
