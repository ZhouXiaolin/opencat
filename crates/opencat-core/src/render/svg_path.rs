use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle, PathEffectSpec, StrokeCap, StrokeJoin, StrokeSpec};
use crate::canvas::{Canvas2D, Rect};
use crate::display::list::{DisplayRect, SvgPathDisplayItem};

use super::paint_conv::{background_fill_to_paint_spec, color_token_to_rgba};
use super::{RenderCache, RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

pub fn render_svg_path<C: Canvas2D>(
    canvas: &mut C,
    item: &SvgPathDisplayItem,
    _ctx: &RenderCtx<C>,
    _cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
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
        if width <= 0.0 { return None; }
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
        if let Some(dash_len) = item.paint.stroke_dasharray {
            if dash_len > 0.0 {
                let offset = item.paint.stroke_dashoffset.unwrap_or(0.0);
                spec.path_effect = Some(PathEffectSpec::Dash {
                    intervals: vec![dash_len, dash_len],
                    phase: offset,
                });
            }
        }
        Some(spec)
    });

    canvas.save();

    let scale_f32 = scale as f32;
    let draw_w = item.view_box[2] * scale_f32;
    let draw_h = item.view_box[3] * scale_f32;
    let offset_x = (dst.width() as f32 - draw_w) / 2.0;
    let offset_y = (dst.height() as f32 - draw_h) / 2.0;
    canvas.translate(dst.x0 as f32 + offset_x, dst.y0 as f32 + offset_y);
    canvas.scale(scale_f32, scale_f32);
    canvas.translate(-item.view_box[0], -item.view_box[1]);

    for path_data in &item.path_data {
        if let Some(path) = canvas.make_path_from_svg(path_data) {
            if let Some(ref paint) = fill_paint {
                canvas.draw_path(&path, paint);
            }
            if let Some(ref paint) = stroke_paint {
                canvas.draw_path(&path, paint);
            }
        }
    }

    canvas.restore();
    Ok(())
}
