use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};
use crate::canvas::{Canvas2D, Rect};
use crate::display::list::{DisplayRect, TextDisplayItem};

use super::paint_conv::drop_shadow_to_image_filter;
use super::{RenderCache, RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

pub fn render_text<C: Canvas2D>(
    canvas: &mut C,
    item: &TextDisplayItem,
    _ctx: &RenderCtx<C>,
    _cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let paint = PaintSpec {
        fill: FillSpec::Solid(super::paint_conv::color_token_to_rgba(&item.style.color)),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };

    let font_size = item.style.text_px;
    canvas.draw_simple_text(&item.text, item.bounds.x, item.bounds.y + font_size, font_size, &paint);
    Ok(())
}

pub fn render_text_with_shadows<C: Canvas2D>(
    canvas: &mut C,
    item: &TextDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    if let Some(ref shadow) = item.drop_shadow {
        let (left, top, right, bottom) = shadow.outsets();
        let shadow_bounds = kurbo_rect(item.bounds.outset(left, top, right, bottom));
        let (image_filter, _color) = drop_shadow_to_image_filter(shadow);
        let layer_paint = PaintSpec {
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
        canvas.save_layer_with(Some(shadow_bounds), &layer_paint);
        render_text(canvas, item, ctx, cache)?;
        canvas.restore();
    }
    render_text(canvas, item, ctx, cache)?;
    Ok(())
}
