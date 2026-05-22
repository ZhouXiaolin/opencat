#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::display::list::DisplayItem;
use crate::draw::cache::{self as draw_cache, CachedDrawRange};
use crate::draw::op::DrawOp;
use crate::runtime::fingerprint::item_paint_fingerprint;

use super::{RenderCtx, RenderError};

fn should_cache_item_picture(item: &DisplayItem) -> bool {
    matches!(
        item,
        DisplayItem::Bitmap(_) | DisplayItem::DrawScript(_) | DisplayItem::SvgPath(_)
    )
}

pub fn render_display_item(
    ctx: &mut RenderCtx,
    item: &DisplayItem,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    if should_cache_item_picture(item)
        && let Some(cache_key) = item_paint_fingerprint(item) {
            return render_display_item_cached(ctx, item, cache_key, cache);
        }
    render_display_item_direct(ctx, item, cache)
}

fn render_display_item_cached(
    ctx: &mut RenderCtx,
    item: &DisplayItem,
    cache_key: u64,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    #[cfg(feature = "profile")]
    let _cached_span = span!(target: "render.backend", Level::TRACE, "draw_item_cached").entered();

    let semantics = item.picture_semantics();

    // Cache hit: import segment and replay with draw translation
    if let Some(cached_range) = cache.item_ranges.get_cloned(&cache_key)
        && let Some(segment) = cache.segments.get_cloned(&cached_range.segment_key) {
            #[cfg(feature = "profile")]
            event!(
                target: "render.cache",
                Level::TRACE,
                kind = "cache",
                name = "item_picture",
                result = "hit",
                amount = 1_u64
            );

            ctx.builder.push(DrawOp::Save);
            ctx.builder.push(DrawOp::Translate {
                x: semantics.draw_translation_x,
                y: semantics.draw_translation_y,
            });
            ctx.builder.import_segment(&segment);
            ctx.builder.push(DrawOp::Restore);
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

    // Cache miss: render directly with draw_translation, snapshot, store
    ctx.builder.push(DrawOp::Save);
    ctx.builder.push(DrawOp::Translate {
        x: semantics.draw_translation_x,
        y: semantics.draw_translation_y,
    });

    let marker = ctx.builder.begin_range();
    render_display_item_direct(ctx, item, cache)?;
    let range = ctx.builder.end_range(marker);

    ctx.builder.push(DrawOp::Restore);

    // Snapshot and store in cache
    let segment = ctx.builder.snapshot_range(range);
    let segment_key = cache_key;

    cache.segments.insert(segment_key, segment);
    cache.item_ranges.insert(
        cache_key,
        CachedDrawRange {
            segment_range: range,
            fingerprint: cache_key,
            bounds: semantics.record_bounds,
            segment_key,
        },
    );

    Ok(())
}

fn render_display_item_direct(
    ctx: &mut RenderCtx,
    item: &DisplayItem,
    _cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    match item {
        DisplayItem::Rect(rect) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_rect").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "rect", result = "count", amount = 1_u64);
            super::rect::render_rect_with_shadows(ctx, rect)
        }
        DisplayItem::Timeline(timeline) => {
            #[cfg(feature = "profile")]
            let _span =
                span!(target: "render.backend", Level::TRACE, "draw_item_timeline").entered();
            super::timeline::render_timeline(ctx, timeline)
        }
        DisplayItem::Text(text) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_text").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "text", result = "count", amount = 1_u64);
            super::text::render_text_with_shadows(ctx, text)
        }
        DisplayItem::Bitmap(bitmap) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_bitmap").entered();
            super::bitmap::render_bitmap_with_shadows(ctx, bitmap)
        }
        DisplayItem::DrawScript(script) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_script").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "script", result = "count", amount = 1_u64);
            super::draw_script::render_draw_script(ctx, script)
        }
        DisplayItem::SvgPath(svg) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_svg").entered();
            super::svg_path::render_svg_path(ctx, svg)
        }
    }
}
