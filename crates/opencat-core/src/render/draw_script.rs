use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle, StrokeSpec};
use crate::canvas::StrokeCap;
use crate::display::list::DrawScriptDisplayItem;
use crate::draw::op::{
    ColorF32, ColorU8, DRRectSpec, DrawOp, LineCap, LineJoin, PointMode, Radii4, Rect4,
};
use crate::draw::types::{ImageRef, PathOp};
use crate::scene::script::CanvasCommand;
use crate::style::ObjectFit;

use super::ctx::RenderCtx;
use super::RenderError;

use crate::scene::script::{ScriptLineCap, ScriptLineJoin, ScriptLineJoin::*, ScriptPointMode};

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
            fill_color: ColorU8 { r: 0, g: 0, b: 0, a: 255 },
            stroke_color: ColorU8 { r: 0, g: 0, b: 0, a: 255 },
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

fn script_color_to_color_u8(c: crate::scene::script::ScriptColor) -> ColorU8 {
    ColorU8 { r: c.r, g: c.g, b: c.b, a: c.a }
}

fn script_color_to_color_f32(c: crate::scene::script::ScriptColor, global_alpha: f32) -> ColorF32 {
    ColorF32 {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: (c.a as f32 / 255.0) * global_alpha,
    }
}

fn script_line_cap_to_draw(c: ScriptLineCap) -> LineCap {
    match c {
        ScriptLineCap::Butt => LineCap::Butt,
        ScriptLineCap::Round => LineCap::Round,
        ScriptLineCap::Square => LineCap::Square,
    }
}

fn script_line_join_to_draw(j: ScriptLineJoin) -> LineJoin {
    match j {
        Miter => LineJoin::Miter,
        ScriptLineJoin::Round => LineJoin::Round,
        ScriptLineJoin::Bevel => LineJoin::Bevel,
    }
}

fn script_point_mode_to_draw(m: ScriptPointMode) -> PointMode {
    match m {
        ScriptPointMode::Points => PointMode::Points,
        ScriptPointMode::Lines => PointMode::Lines,
        ScriptPointMode::Polygon => PointMode::Polygon,
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
    Rect4 { x, y, width: w, height: h }
}

fn drrect_spec(
    x: f32, y: f32, w: f32, h: f32, r: f32,
) -> DRRectSpec {
    DRRectSpec {
        rect: rect4_xywh(x, y, w, h),
        radii: Radii4 {
            top_left: r, top_right: r, bottom_right: r, bottom_left: r,
        },
    }
}

pub fn render_draw_script(
    ctx: &mut RenderCtx,
    item: &DrawScriptDisplayItem,
) -> Result<(), RenderError> {
    let mut state = LocalPaintState::default();
    let b = &mut ctx.builder;
    let clip_rect = rect4_xywh(item.bounds.x, item.bounds.y, item.bounds.width, item.bounds.height);

    let needs_alpha_layer = item.commands.iter().any(|cmd| {
        matches!(cmd, CanvasCommand::Clear { color: None })
    });

    if needs_alpha_layer {
        b.push(DrawOp::SaveLayer {
            bounds: Some(clip_rect),
            paint: None,
            alpha: 1.0,
        });
    } else {
        b.push(DrawOp::Save);
        b.push(DrawOp::BeginPath);
        b.push(DrawOp::Path(PathOp::AddRect {
            x: item.bounds.x,
            y: item.bounds.y,
            width: item.bounds.width,
            height: item.bounds.height,
        }));
        b.push(DrawOp::ClipPath { anti_alias: true });
    }

    for command in &item.commands {
        execute_canvas_command(b, command, &mut state)?;
    }

    b.push(DrawOp::Restore);
    Ok(())
}

fn execute_canvas_command(
    b: &mut crate::draw::builder::DrawOpBuilder,
    cmd: &CanvasCommand,
    state: &mut LocalPaintState,
) -> Result<(), RenderError> {
    match cmd {
        CanvasCommand::Save => {
            b.push(DrawOp::Save);
        }
        CanvasCommand::SaveLayer { alpha, bounds } => {
            let layer_alpha = (state.global_alpha * *alpha).clamp(0.0, 1.0);
            let bounds_rect = bounds.map(|bds| rect4_xywh(bds[0], bds[1], bds[2], bds[3]));
            b.push(DrawOp::SaveLayer {
                bounds: bounds_rect,
                paint: None,
                alpha: layer_alpha,
            });
        }
        CanvasCommand::Restore => {
            b.push(DrawOp::Restore);
        }
        CanvasCommand::RestoreToCount { count } => {
            b.push(DrawOp::RestoreToCount { count: *count });
        }
        CanvasCommand::SetFillStyle { color } => {
            state.fill_color = script_color_to_color_u8(*color);
            b.push(DrawOp::SetFillStyle {
                color: state.fill_color,
            });
        }
        CanvasCommand::SetStrokeStyle { color } => {
            state.stroke_color = script_color_to_color_u8(*color);
            b.push(DrawOp::SetStrokeStyle {
                color: state.stroke_color,
            });
        }
        CanvasCommand::SetLineWidth { width } => {
            state.line_width = *width;
            b.push(DrawOp::SetLineWidth { width: *width });
        }
        CanvasCommand::SetLineCap { cap } => {
            state.line_cap = script_line_cap_to_draw(*cap);
            b.push(DrawOp::SetLineCap { cap: state.line_cap });
        }
        CanvasCommand::SetLineJoin { join } => {
            state.line_join = script_line_join_to_draw(*join);
            b.push(DrawOp::SetLineJoin { join: state.line_join });
        }
        CanvasCommand::SetLineDash { intervals, phase } => {
            state.line_dash = Some(intervals.clone());
            state.line_dash_phase = *phase;
            let range = b.intern_f32_range(intervals);
            b.push(DrawOp::SetLineDash {
                intervals: range,
                phase: *phase,
            });
        }
        CanvasCommand::ClearLineDash => {
            state.line_dash = None;
            state.line_dash_phase = 0.0;
            b.push(DrawOp::ClearLineDash);
        }
        CanvasCommand::SetGlobalAlpha { alpha } => {
            state.global_alpha = *alpha;
            b.push(DrawOp::SetGlobalAlpha { alpha: *alpha });
        }
        CanvasCommand::SetAntiAlias { enabled } => {
            state.anti_alias = *enabled;
            b.push(DrawOp::SetAntiAlias { enabled: *enabled });
        }
        CanvasCommand::Translate { x, y } => {
            b.push(DrawOp::Translate { x: *x, y: *y });
        }
        CanvasCommand::Scale { x, y } => {
            b.push(DrawOp::Scale { x: *x, y: *y });
        }
        CanvasCommand::Rotate { degrees } => {
            b.push(DrawOp::Rotate { degrees: *degrees, cx: 0.0, cy: 0.0 });
        }
        CanvasCommand::Skew { sx, sy } => {
            b.push(DrawOp::Skew { sx: *sx, sy: *sy });
        }
        CanvasCommand::Concat { matrix } => {
            b.push(DrawOp::Concat { matrix: *matrix });
        }

        // ── Clear / Paint ──────────────────────────────────────────────

        CanvasCommand::Clear { color } => {
            let color_f32 = match color {
                Some(c) => script_color_to_color_f32(*c, state.global_alpha),
                None => ColorF32 { r: 0.0, g: 0.0, b: 0.0, a: 0.0 },
            };
            b.push(DrawOp::Clear { color: color_f32 });
        }
        CanvasCommand::DrawPaint { color, .. } => {
            let paint_id = b.intern_paint(PaintSpec {
                fill: FillSpec::Solid([
                    color.r as f32 / 255.0,
                    color.g as f32 / 255.0,
                    color.b as f32 / 255.0,
                    (color.a as f32 / 255.0) * state.global_alpha,
                ]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: state.anti_alias,
                blend_mode: BlendMode::SrcOver,
                image_filter: None,
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            });
            b.push(DrawOp::Paint { paint: paint_id });
        }

        // ── Text ───────────────────────────────────────────────────────

        CanvasCommand::DrawText { text, x, y, color, stroke, stroke_width, font_size, .. } => {
            let _ = (text, x, y, color, stroke, stroke_width, font_size);
            // TODO: Phase 2 — decompose DrawText into glyph path DrawOps via font DB.
            // The original code called canvas.draw_simple_text(). For the DrawOp pipeline
            // this needs to be decomposed into individual glyph outlines.
        }

        // ── Immediate rect / rrect ─────────────────────────────────────

        CanvasCommand::FillRect { x, y, width, height, color } => {
            b.push(DrawOp::SetFillStyle {
                color: script_color_to_color_u8(*color),
            });
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddRect { x: *x, y: *y, width: *width, height: *height }));
            b.push(DrawOp::FillPath);
        }
        CanvasCommand::FillRRect { x, y, width, height, radius } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddRRect {
                x: *x, y: *y, width: *width, height: *height, radius: *radius,
            }));
            b.push(DrawOp::FillPath);
        }
        CanvasCommand::StrokeRect { x, y, width, height, color, stroke_width } => {
            b.push(DrawOp::SetStrokeStyle {
                color: script_color_to_color_u8(*color),
            });
            b.push(DrawOp::SetLineWidth { width: *stroke_width });
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddRect { x: *x, y: *y, width: *width, height: *height }));
            b.push(DrawOp::StrokePath);
        }
        CanvasCommand::StrokeRRect { x, y, width, height, radius } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddRRect {
                x: *x, y: *y, width: *width, height: *height, radius: *radius,
            }));
            b.push(DrawOp::StrokePath);
        }

        // ── Line / Circle / Oval / Arc ─────────────────────────────────

        CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::MoveTo { x: *x0, y: *y0 }));
            b.push(DrawOp::Path(PathOp::LineTo { x: *x1, y: *y1 }));
            b.push(DrawOp::StrokePath);
        }
        CanvasCommand::FillCircle { cx, cy, radius } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddOval {
                x: cx - radius, y: cy - radius,
                width: radius * 2.0, height: radius * 2.0,
            }));
            b.push(DrawOp::FillPath);
        }
        CanvasCommand::StrokeCircle { cx, cy, radius } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddOval {
                x: cx - radius, y: cy - radius,
                width: radius * 2.0, height: radius * 2.0,
            }));
            b.push(DrawOp::StrokePath);
        }
        CanvasCommand::FillOval { cx, cy, rx, ry } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddOval {
                x: cx - rx, y: cy - ry,
                width: rx * 2.0, height: ry * 2.0,
            }));
            b.push(DrawOp::FillPath);
        }
        CanvasCommand::StrokeOval { cx, cy, rx, ry } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddOval {
                x: cx - rx, y: cy - ry,
                width: rx * 2.0, height: ry * 2.0,
            }));
            b.push(DrawOp::StrokePath);
        }
        CanvasCommand::DrawArc { cx, cy, rx, ry, start_angle, sweep_angle, use_center } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddArc {
                x: cx - rx, y: cy - ry,
                width: rx * 2.0, height: ry * 2.0,
                start_angle: *start_angle, sweep_angle: *sweep_angle,
            }));
            let _ = use_center;
            b.push(DrawOp::FillPath);
        }
        CanvasCommand::StrokeArc { cx, cy, rx, ry, start_angle, sweep_angle } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddArc {
                x: cx - rx, y: cy - ry,
                width: rx * 2.0, height: ry * 2.0,
                start_angle: *start_angle, sweep_angle: *sweep_angle,
            }));
            b.push(DrawOp::StrokePath);
        }

        // ── Path construction (stateful) ────────────────────────────────

        CanvasCommand::BeginPath => {
            b.push(DrawOp::BeginPath);
        }
        CanvasCommand::MoveTo { x, y } => {
            b.push(DrawOp::Path(PathOp::MoveTo { x: *x, y: *y }));
        }
        CanvasCommand::LineTo { x, y } => {
            b.push(DrawOp::Path(PathOp::LineTo { x: *x, y: *y }));
        }
        CanvasCommand::QuadTo { cx, cy, x, y } => {
            b.push(DrawOp::Path(PathOp::QuadTo { cx: *cx, cy: *cy, x: *x, y: *y }));
        }
        CanvasCommand::CubicTo { c1x, c1y, c2x, c2y, x, y } => {
            b.push(DrawOp::Path(PathOp::CubicTo {
                c1x: *c1x, c1y: *c1y, c2x: *c2x, c2y: *c2y, x: *x, y: *y,
            }));
        }
        CanvasCommand::ClosePath => {
            b.push(DrawOp::Path(PathOp::Close));
        }
        CanvasCommand::AddRectPath { x, y, width, height } => {
            b.push(DrawOp::Path(PathOp::AddRect { x: *x, y: *y, width: *width, height: *height }));
        }
        CanvasCommand::AddRRectPath { x, y, width, height, radius } => {
            b.push(DrawOp::Path(PathOp::AddRRect {
                x: *x, y: *y, width: *width, height: *height, radius: *radius,
            }));
        }
        CanvasCommand::AddOvalPath { x, y, width, height } => {
            b.push(DrawOp::Path(PathOp::AddOval { x: *x, y: *y, width: *width, height: *height }));
        }
        CanvasCommand::AddArcPath { x, y, width, height, start_angle, sweep_angle } => {
            b.push(DrawOp::Path(PathOp::AddArc {
                x: *x, y: *y, width: *width, height: *height,
                start_angle: *start_angle, sweep_angle: *sweep_angle,
            }));
        }
        CanvasCommand::FillPath => {
            b.push(DrawOp::FillPath);
        }
        CanvasCommand::StrokePath => {
            b.push(DrawOp::StrokePath);
        }

        // ── Clip ───────────────────────────────────────────────────────

        CanvasCommand::ClipRect { x, y, width, height, anti_alias } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddRect { x: *x, y: *y, width: *width, height: *height }));
            b.push(DrawOp::ClipPath { anti_alias: *anti_alias });
        }
        CanvasCommand::ClipPath { anti_alias } => {
            b.push(DrawOp::ClipPath { anti_alias: *anti_alias });
        }
        CanvasCommand::ClipRRect { x, y, width, height, radius, anti_alias } => {
            b.push(DrawOp::BeginPath);
            b.push(DrawOp::Path(PathOp::AddRRect {
                x: *x, y: *y, width: *width, height: *height, radius: *radius,
            }));
            b.push(DrawOp::ClipPath { anti_alias: *anti_alias });
        }

        // ── Points ─────────────────────────────────────────────────────

        CanvasCommand::DrawPoints { mode, points } => {
            let paint_id = b.intern_paint(state.stroke_paint_spec());
            let range = b.intern_f32_range(points);
            b.push(DrawOp::Points {
                mode: script_point_mode_to_draw(*mode),
                points: range,
                paint: paint_id,
            });
        }

        // ── DRRect ─────────────────────────────────────────────────────

        CanvasCommand::FillDRRect {
            outer_x, outer_y, outer_width, outer_height, outer_radius,
            inner_x, inner_y, inner_width, inner_height, inner_radius,
        } => {
            let paint_id = b.intern_paint(state.fill_paint_spec());
            b.push(DrawOp::DRRect {
                outer: drrect_spec(*outer_x, *outer_y, *outer_width, *outer_height, *outer_radius),
                inner: drrect_spec(*inner_x, *inner_y, *inner_width, *inner_height, *inner_radius),
                paint: paint_id,
            });
        }
        CanvasCommand::StrokeDRRect {
            outer_x, outer_y, outer_width, outer_height, outer_radius,
            inner_x, inner_y, inner_width, inner_height, inner_radius,
        } => {
            let paint_id = b.intern_paint(state.stroke_paint_spec());
            b.push(DrawOp::DRRect {
                outer: drrect_spec(*outer_x, *outer_y, *outer_width, *outer_height, *outer_radius),
                inner: drrect_spec(*inner_x, *inner_y, *inner_width, *inner_height, *inner_radius),
                paint: paint_id,
            });
        }

        // ── Images ─────────────────────────────────────────────────────

        CanvasCommand::DrawImage { asset_id, x, y, width, height, src_rect, object_fit, .. } => {
            let img_ref = ImageRef::Static { asset_id: asset_id.clone() };
            let dst = rect4_xywh(*x, *y, *width, *height);
            let src = src_rect.map(|s| rect4_xywh(s[0], s[1], s[2], s[3]));
            match object_fit {
                ObjectFit::Fill => {
                    b.push(DrawOp::ImageRect {
                        image: img_ref,
                        src,
                        dst,
                        paint: None,
                    });
                }
                ObjectFit::Contain | ObjectFit::Cover => {
                    // TODO: object-fit calculation needs intrinsic image dimensions,
                    // which are not available at DrawOp recording time. For now,
                    // emit ImageRect with Fill semantics. The media preparation
                    // pass or executor can handle object-fit later.
                    b.push(DrawOp::ImageRect {
                        image: img_ref,
                        src,
                        dst,
                        paint: None,
                    });
                }
            }
        }
        CanvasCommand::DrawImageSimple { asset_id, x, y, .. } => {
            let img_ref = ImageRef::Static { asset_id: asset_id.clone() };
            b.push(DrawOp::Image {
                image: img_ref,
                x: *x,
                y: *y,
                paint: None,
            });
        }
    }
    Ok(())
}
