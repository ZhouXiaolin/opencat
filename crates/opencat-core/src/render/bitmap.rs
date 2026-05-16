#[cfg(feature = "profile")]
use tracing::{Level, event};

use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};
use crate::canvas::{Canvas2D, Rect};
use crate::display::list::BitmapDisplayItem;
use crate::style::ObjectFit;

use super::paint_conv::background_fill_to_paint_spec;
use super::rect::{clip_bounds, draw_box_shadow, draw_inset_shadow, draw_item_drop_shadow, draw_node_border, kurbo_rect};
use super::{record_cache_pressure, RenderCache, RenderCtx, RenderError};

pub(crate) fn fitted_rect(src_width: f32, src_height: f32, dst: &Rect, cover: bool) -> Rect {
    let iw = src_width as f64;
    let ih = src_height as f64;
    if iw <= 0.0 || ih <= 0.0 {
        return *dst;
    }
    let src_aspect = iw / ih;
    let dst_aspect = dst.width() / dst.height();

    let scale = if cover {
        if src_aspect > dst_aspect { dst.height() / ih } else { dst.width() / iw }
    } else if src_aspect > dst_aspect {
        dst.width() / iw
    } else {
        dst.height() / ih
    };

    let width = iw * scale;
    let height = ih * scale;
    let x = dst.x0 + (dst.width() - width) / 2.0;
    let y = dst.y0 + (dst.height() - height) / 2.0;
    Rect::new(x, y, x + width, y + height)
}

pub(crate) fn cover_src_rect(src_width: f32, src_height: f32, dst: &Rect) -> Rect {
    let fitted = fitted_rect(src_width, src_height, dst, true);
    let scale = fitted.width() / src_width as f64;
    let visible_width = dst.width() / scale;
    let visible_height = dst.height() / scale;
    let x = (src_width as f64 - visible_width) / 2.0;
    let y = (src_height as f64 - visible_height) / 2.0;
    Rect::new(x, y, x + visible_width, y + visible_height)
}

fn draw_bitmap_image<C: Canvas2D>(
    canvas: &mut C,
    item: &BitmapDisplayItem,
    image: &C::Image,
    dst: &Rect,
    paint: &PaintSpec,
    src_width: f32,
    src_height: f32,
) {
    match item.object_fit {
        ObjectFit::Fill => {
            canvas.draw_image_rect(image, None, dst, Some(paint));
        }
        ObjectFit::Contain => {
            let fitted = fitted_rect(src_width, src_height, dst, false);
            canvas.draw_image_rect(image, None, &fitted, Some(paint));
        }
        ObjectFit::Cover => {
            let src = cover_src_rect(src_width, src_height, dst);
            canvas.draw_image_rect(image, Some(&src), dst, Some(paint));
        }
    }
}

pub fn render_bitmap<C: Canvas2D>(
    canvas: &mut C,
    item: &BitmapDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    #[cfg(feature = "profile")]
    event!(
        target: "render.draw",
        Level::TRACE,
        kind = "draw",
        name = "bitmap",
        result = "count",
        amount = 1_u64
    );

    let style = &item.paint;
    let dst = kurbo_rect(item.bounds);

    let (image, src_width, src_height) = if item.video_timing.is_some() {
        let frame_index = ctx.frame_ctx.frame as u32;
        let resolved_id = ctx.asset_paths
            .and_then(|store| store.path(&item.asset_id))
            .map(|p| crate::resource::asset_id::AssetId(p.to_string_lossy().to_string()));
        let asset_id = resolved_id.as_ref().unwrap_or(&item.asset_id);
        let frame = ctx.video.borrow_mut().frame_rgba(asset_id, frame_index)
            .map_err(|e| RenderError::MissingResource(format!("video frame: {}", e)))?;
        let w = frame.width;
        let h = frame.height;
        #[cfg(feature = "profile")]
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "video_frame",
            result = "decode",
            amount = 1_u64
        );
        (canvas.make_image_from_rgba(&frame.data, w, h), w, h)
    } else {
        let asset_key = item.asset_id.0.clone();
        let mut lru = cache.images.borrow_mut();
        if let Some(Some(img)) = lru.get_cloned(&asset_key) {
            #[cfg(feature = "profile")]
            event!(
                target: "render.cache",
                Level::TRACE,
                kind = "cache",
                name = "image",
                result = "hit",
                amount = 1_u64
            );
            (img, item.width as u32, item.height as u32)
        } else {
            drop(lru);
            let loaded: Option<C::Image> = 'load: {
                if let Some(path) = ctx.asset_paths.and_then(|store| store.path(&item.asset_id)) {
                    let encoded = std::fs::read(path).map_err(|e| {
                        RenderError::MissingResource(format!("failed to read image: {} ({})", path.display(), e))
                    })?;
                    if let Some(img) = canvas.make_image_from_encoded(&encoded) {
                        #[cfg(feature = "profile")]
                        event!(
                            target: "render.cache",
                            Level::TRACE,
                            kind = "cache",
                            name = "image",
                            result = "miss",
                            amount = 1_u64
                        );
                        break 'load Some(img);
                    }
                }
                None
            };
            let mut lru = cache.images.borrow_mut();
            let report = lru.insert(asset_key, loaded.clone());
            record_cache_pressure("image", &report);
            drop(report);
            match loaded {
                Some(img) => (img, item.width as u32, item.height as u32),
                None => return Err(RenderError::MissingResource(format!("cannot load image: {}", item.asset_id.0))),
            }
        }
    };

    canvas.save();
    clip_bounds(canvas, item.bounds, &style.border_radius);

    if let Some(ref bg) = style.background {
        let paint = background_fill_to_paint_spec(bg);
        canvas.draw_rect(&dst, &paint);
    }

    let paint = PaintSpec {
        fill: FillSpec::Solid([1.0; 4]),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };

    draw_bitmap_image(canvas, item, &image, &dst, &paint, src_width as f32, src_height as f32);

    if let Some(ref shadow) = style.inset_shadow {
        draw_inset_shadow(canvas, item.bounds, &style.border_radius, shadow);
    }

    draw_node_border(
        canvas, &dst, &style.border_radius,
        style.border_width, style.border_top_width, style.border_right_width,
        style.border_bottom_width, style.border_left_width,
        style.border_color, style.border_style, style.blur_sigma,
    );

    canvas.restore();
    Ok(())
}

pub fn render_bitmap_with_shadows<C: Canvas2D>(
    canvas: &mut C,
    item: &BitmapDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let bounds = item.bounds;

    if let Some(ref shadow) = style.box_shadow {
        draw_box_shadow(canvas, bounds, &style.border_radius, shadow);
    }

    if let Some(ref shadow) = style.drop_shadow {
        draw_item_drop_shadow(canvas, bounds, shadow, |c| {
            render_bitmap(c, item, ctx, cache)
        })?;
    }
    render_bitmap(canvas, item, ctx, cache)?;

    Ok(())
}
