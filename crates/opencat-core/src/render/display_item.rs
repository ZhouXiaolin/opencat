use crate::canvas::Canvas2D;
use crate::display::list::DisplayItem;

use super::{RenderCache, RenderCtx, RenderError};

pub fn render_display_item<C: Canvas2D>(
    canvas: &mut C,
    item: &DisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    match item {
        DisplayItem::Rect(rect) => super::rect::render_rect_with_shadows(canvas, rect, ctx, cache),
        DisplayItem::Timeline(timeline) => super::timeline::render_timeline(canvas, timeline, ctx, cache),
        DisplayItem::Text(text) => super::text::render_text_with_shadows(canvas, text, ctx, cache),
        DisplayItem::Bitmap(bitmap) => super::bitmap::render_bitmap(canvas, bitmap, ctx, cache),
        DisplayItem::DrawScript(script) => super::draw_script::render_draw_script(canvas, script, ctx, cache),
        DisplayItem::SvgPath(svg) => super::svg_path::render_svg_path(canvas, svg, ctx, cache),
    }
}
