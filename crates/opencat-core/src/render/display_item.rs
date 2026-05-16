use crate::canvas::{Canvas2D, Rect};
use crate::display::list::DisplayItem;
use crate::runtime::fingerprint::item_paint_fingerprint;

use super::{RenderCache, RenderCtx, RenderError};

fn should_cache_item_picture(item: &DisplayItem) -> bool {
    matches!(
        item,
        DisplayItem::Bitmap(_) | DisplayItem::DrawScript(_) | DisplayItem::SvgPath(_)
    )
}

pub fn render_display_item<C: Canvas2D>(
    canvas: &mut C,
    item: &DisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    if should_cache_item_picture(item) {
        if let Some(cache_key) = item_paint_fingerprint(item) {
            return render_display_item_cached(canvas, item, cache_key, ctx, cache);
        }
    }
    render_display_item_direct(canvas, item, ctx, cache)
}

fn render_display_item_cached<C: Canvas2D>(
    canvas: &mut C,
    item: &DisplayItem,
    cache_key: u64,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let semantics = item.picture_semantics();

    if let Some(picture) = cache.item_pictures.borrow_mut().get_cloned(&cache_key) {
        canvas.save();
        canvas.translate(semantics.draw_translation_x, semantics.draw_translation_y);
        canvas.draw_picture(&picture, None, None);
        canvas.restore();
        return Ok(());
    }

    let bounds = Rect::new(
        0.0,
        0.0,
        semantics.record_bounds.width as f64,
        semantics.record_bounds.height as f64,
    );
    let picture = canvas.make_picture(&bounds, |rec_canvas| {
        rec_canvas.translate(semantics.record_translation_x, semantics.record_translation_y);
        let _ = render_display_item_direct(rec_canvas, item, ctx, cache);
    });

    cache.item_pictures.borrow_mut().insert(cache_key, picture.clone());

    canvas.save();
    canvas.translate(semantics.draw_translation_x, semantics.draw_translation_y);
    canvas.draw_picture(&picture, None, None);
    canvas.restore();
    Ok(())
}

fn render_display_item_direct<C: Canvas2D>(
    canvas: &mut C,
    item: &DisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    match item {
        DisplayItem::Rect(rect) => super::rect::render_rect_with_shadows(canvas, rect, ctx, cache),
        DisplayItem::Timeline(timeline) => super::timeline::render_timeline(canvas, timeline, ctx, cache),
        DisplayItem::Text(text) => super::text::render_text_with_shadows(canvas, text, ctx, cache),
        DisplayItem::Bitmap(bitmap) => super::bitmap::render_bitmap_with_shadows(canvas, bitmap, ctx, cache),
        DisplayItem::DrawScript(script) => super::draw_script::render_draw_script(canvas, script, ctx, cache),
        DisplayItem::SvgPath(svg) => super::svg_path::render_svg_path(canvas, svg, ctx, cache),
    }
}
