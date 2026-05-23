use crate::canvas::StrokeCap;
use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle, StrokeSpec};
use crate::display::list::DrawScriptDisplayItem;
use crate::ir::draw_op::{ColorU8, DrawOp, LineCap, LineJoin, Rect4};
use crate::ir::draw_types::PathOp;

use super::RenderError;
use super::ctx::RenderCtx;

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
    let b = &mut ctx.builder;

    // Heuristic: Clear with fully transparent (a == 0.0) acts like "clear all"
    // In practice we check if any Clear exists
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
        execute_draw_op(b, command, &mut state)?;
    }

    b.push(DrawOp::Restore);
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
    }
    Ok(())
}
