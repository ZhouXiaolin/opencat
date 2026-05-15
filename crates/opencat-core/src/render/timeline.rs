use crate::canvas::Canvas2D;
use crate::display::list::TimelineDisplayItem;

use super::{RenderCache, RenderCtx, RenderError};

pub fn render_timeline<C: Canvas2D>(
    canvas: &mut C,
    item: &TimelineDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let rect_item = crate::display::list::RectDisplayItem {
        bounds: item.bounds,
        paint: item.paint.clone(),
    };
    super::rect::render_rect_with_shadows(canvas, &rect_item, ctx, cache)
}
