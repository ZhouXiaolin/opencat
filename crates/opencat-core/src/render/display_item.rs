#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::canvas::{Canvas2D, Rect};
use crate::display::list::DisplayItem;
use crate::runtime::fingerprint::item_paint_fingerprint;

use super::{record_cache_pressure, RenderCache, RenderCtx, RenderError};

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
    #[cfg(feature = "profile")]
    let _cached_span = span!(target: "render.backend", Level::TRACE, "draw_item_cached").entered();

    let semantics = item.picture_semantics();

    if let Some(picture) = cache.item_pictures.borrow_mut().get_cloned(&cache_key) {
        #[cfg(feature = "profile")]
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "item_picture",
            result = "hit",
            amount = 1_u64
        );
        canvas.save();
        canvas.translate(semantics.draw_translation_x, semantics.draw_translation_y);
        canvas.draw_picture(&picture, None, None);
        canvas.restore();
        return Ok(());
    }

    #[cfg(feature = "profile")]
    event!(
        target: "render.cache",
        Level::TRACE,
        kind = "cache",
        name = "item_picture",
        result = "miss",
        amount = 1_u64
    );

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

    let report = cache.item_pictures.borrow_mut().insert(cache_key, picture.clone());
    record_cache_pressure("item_picture", &report);

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
        DisplayItem::Rect(rect) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_rect").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "rect", result = "count", amount = 1_u64);
            super::rect::render_rect_with_shadows(canvas, rect, ctx, cache)
        }
        DisplayItem::Timeline(timeline) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_timeline").entered();
            super::timeline::render_timeline(ctx, timeline)
        }
        DisplayItem::Text(text) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_text").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "text", result = "count", amount = 1_u64);
            super::text::render_text_with_shadows(canvas, text, ctx, cache)
        }
        DisplayItem::Bitmap(bitmap) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_bitmap").entered();
            super::bitmap::render_bitmap_with_shadows(canvas, bitmap, ctx, cache)
        }
        DisplayItem::DrawScript(script) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_script").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "script", result = "count", amount = 1_u64);
            super::draw_script::render_draw_script(canvas, script, ctx, cache)
        }
        DisplayItem::SvgPath(svg) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_svg").entered();
            super::svg_path::render_svg_path(canvas, svg, ctx, cache)
        }
    }
}
