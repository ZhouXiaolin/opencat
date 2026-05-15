use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};
use crate::canvas::{Canvas2D, Rect};
use crate::display::list::{BitmapDisplayItem, DisplayRect};
use crate::style::ObjectFit;

use super::paint_conv::background_fill_to_paint_spec;
use super::{RenderCache, RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

fn fitted_rect(src_width: f32, src_height: f32, dst: &Rect, cover: bool) -> Rect {
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

fn cover_src_rect(src_width: f32, src_height: f32, dst: &Rect) -> Rect {
    let fitted = fitted_rect(src_width, src_height, dst, true);
    let scale = fitted.width() / src_width as f64;
    let visible_width = dst.width() / scale;
    let visible_height = dst.height() / scale;
    let x = (src_width as f64 - visible_width) / 2.0;
    let y = (src_height as f64 - visible_height) / 2.0;
    Rect::new(x, y, x + visible_width, y + visible_height)
}

pub fn render_bitmap<C: Canvas2D>(
    canvas: &mut C,
    item: &BitmapDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let asset_key = item.asset_id.0.clone();
    let dst = kurbo_rect(item.bounds);

    let image = {
        let mut lru = cache.images.borrow_mut();
        if let Some(Some(img)) = lru.get_cloned(&asset_key) {
            img
        } else {
            drop(lru);
            let loaded: Option<C::Image> = 'load: {
                if let Some(path) = ctx.asset_paths.and_then(|store| store.path(&item.asset_id)) {
                    let encoded = std::fs::read(path).map_err(|e| {
                        RenderError::MissingResource(format!("failed to read image: {} ({})", path.display(), e))
                    })?;
                    if let Some(img) = canvas.make_image_from_encoded(&encoded) {
                        break 'load Some(img);
                    }
                }
                None
            };
            let mut lru = cache.images.borrow_mut();
            let report = lru.insert(asset_key, loaded.clone());
            drop(report);
            match loaded {
                Some(img) => img,
                None => return Err(RenderError::MissingResource(format!("cannot load image: {}", item.asset_id.0))),
            }
        }
    };

    if let Some(ref bg) = item.paint.background {
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

    let src_width = item.width as f32;
    let src_height = item.height as f32;

    match item.object_fit {
        ObjectFit::Fill => {
            canvas.draw_image_rect(&image, None, &dst, Some(&paint));
        }
        ObjectFit::Contain => {
            let fitted = fitted_rect(src_width, src_height, &dst, false);
            canvas.draw_image_rect(&image, None, &fitted, Some(&paint));
        }
        ObjectFit::Cover => {
            let src = cover_src_rect(src_width, src_height, &dst);
            canvas.draw_image_rect(&image, Some(&src), &dst, Some(&paint));
        }
    }

    Ok(())
}
