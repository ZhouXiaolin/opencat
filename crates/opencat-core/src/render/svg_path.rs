use crate::canvas::Rect;
use crate::canvas::paint::{
    BlendMode, FillSpec, PaintSpec, PaintStyle, PathEffectSpec, StrokeCap, StrokeJoin, StrokeSpec,
};
use crate::display::list::{DisplayRect, SvgPathDisplayItem};
use crate::draw::op::DrawOp;
use crate::draw::types::{EncodedPath, FillType, PathOp};

use kurbo::BezPath;

use super::paint_conv::{background_fill_to_paint_spec, color_token_to_rgba};
use super::{RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(
        r.x as f64,
        r.y as f64,
        (r.x + r.width) as f64,
        (r.y + r.height) as f64,
    )
}

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
            && dash_len > 0.0 {
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
