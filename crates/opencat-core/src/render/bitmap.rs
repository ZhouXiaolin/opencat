#[cfg(feature = "profile")]
use tracing::{Level, event};

use crate::canvas::Rect;
use crate::display::list::BitmapDisplayItem;
use crate::ir::draw_op::DrawOp;
use crate::ir::draw_types::ImageRef;
use crate::render::builder::DrawOpBuilder;
use crate::style::ObjectFit;

use super::paint_conv::background_fill_to_paint_spec;
use super::rect::{
    clip_bounds, draw_box_shadow, draw_inset_shadow, draw_item_drop_shadow, draw_node_border,
    kurbo_rect, rect_to_rect4,
};
use super::{RenderCtx, RenderError};

pub(crate) fn fitted_rect(src_width: f32, src_height: f32, dst: &Rect, cover: bool) -> Rect {
    let iw = src_width as f64;
    let ih = src_height as f64;
    if iw <= 0.0 || ih <= 0.0 {
        return *dst;
    }
    let src_aspect = iw / ih;
    let dst_aspect = dst.width() / dst.height();

    let scale = if cover {
        if src_aspect > dst_aspect {
            dst.height() / ih
        } else {
            dst.width() / iw
        }
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

fn draw_bitmap_image(
    builder: &mut DrawOpBuilder,
    image_ref: ImageRef,
    item: &BitmapDisplayItem,
    dst: &Rect,
    src_width: f32,
    src_height: f32,
) {
    match item.object_fit {
        ObjectFit::Fill => {
            builder.push(DrawOp::ImageRect {
                image: image_ref.clone(),
                src: None,
                dst: rect_to_rect4(*dst),
                paint: None,
            });
        }
        ObjectFit::Contain => {
            let fitted = fitted_rect(src_width, src_height, dst, false);
            builder.push(DrawOp::ImageRect {
                image: image_ref.clone(),
                src: None,
                dst: rect_to_rect4(fitted),
                paint: None,
            });
        }
        ObjectFit::Cover => {
            let src = cover_src_rect(src_width, src_height, dst);
            builder.push(DrawOp::ImageRect {
                image: image_ref,
                src: Some(rect_to_rect4(src)),
                dst: rect_to_rect4(*dst),
                paint: None,
            });
        }
    }
}

pub fn render_bitmap(ctx: &mut RenderCtx, item: &BitmapDisplayItem) -> Result<(), RenderError> {
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

    let asset_id = item.asset_id.0.clone();
    let image_ref = if item.video_timing.is_some() {
        let frame_index = ctx.frame_ctx.frame;
        ImageRef::VideoFrame {
            asset_id,
            frame_index,
        }
    } else {
        ImageRef::Static { asset_id }
    };

    let src_width = item.width as f32;
    let src_height = item.height as f32;

    let builder = &mut ctx.builder;
    builder.push(DrawOp::Save);
    clip_bounds(builder, item.bounds, &style.border_radius);

    if let Some(ref bg) = style.background {
        let paint = background_fill_to_paint_spec(bg);
        let paint_id = builder.intern_paint(paint);
        builder.push(DrawOp::Rect {
            rect: rect_to_rect4(dst),
            paint: paint_id,
        });
    }

    draw_bitmap_image(builder, image_ref, item, &dst, src_width, src_height);

    if let Some(ref shadow) = style.inset_shadow {
        draw_inset_shadow(builder, item.bounds, &style.border_radius, shadow);
    }

    draw_node_border(
        builder,
        &dst,
        &style.border_radius,
        style.border_width,
        style.border_top_width,
        style.border_right_width,
        style.border_bottom_width,
        style.border_left_width,
        style.border_color,
        style.border_style,
        style.blur_sigma,
    );

    builder.push(DrawOp::Restore);
    Ok(())
}

pub fn render_bitmap_with_shadows(
    ctx: &mut RenderCtx,
    item: &BitmapDisplayItem,
) -> Result<(), RenderError> {
    let style = &item.paint;
    let bounds = item.bounds;

    if let Some(ref shadow) = style.box_shadow {
        draw_box_shadow(ctx.builder, bounds, &style.border_radius, shadow);
    }

    if let Some(ref shadow) = style.drop_shadow {
        draw_item_drop_shadow(ctx, bounds, shadow, |ctx2| render_bitmap(ctx2, item))?;
    }
    render_bitmap(ctx, item)?;

    Ok(())
}
