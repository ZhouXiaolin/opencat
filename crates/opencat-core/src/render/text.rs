use cosmic_text::Command;

use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};
use crate::canvas::{Canvas2D, FillType, Rect};
use crate::display::list::{DisplayRect, TextDisplayItem};
use crate::text::{GlyphData, compute_line_x_shift, rasterize_glyphs};

use super::paint_conv::drop_shadow_to_image_filter;
use super::{RenderCache, RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

fn commands_to_verbs_points(commands: &[Command]) -> (Vec<u8>, Vec<f32>) {
    let mut verbs = Vec::with_capacity(commands.len());
    let mut points = Vec::new();
    for cmd in commands {
        match cmd {
            Command::MoveTo(p) => {
                verbs.push(0);
                points.push(p.x);
                points.push(-p.y);
            }
            Command::LineTo(p) => {
                verbs.push(1);
                points.push(p.x);
                points.push(-p.y);
            }
            Command::QuadTo(c, p) => {
                verbs.push(2);
                points.push(c.x);
                points.push(-c.y);
                points.push(p.x);
                points.push(-p.y);
            }
            Command::CurveTo(c1, c2, p) => {
                verbs.push(3);
                points.push(c1.x);
                points.push(-c1.y);
                points.push(c2.x);
                points.push(-c2.y);
                points.push(p.x);
                points.push(-p.y);
            }
            Command::Close => {
                verbs.push(4);
            }
        }
    }
    (verbs, points)
}

pub fn render_text<C: Canvas2D>(
    canvas: &mut C,
    item: &TextDisplayItem,
    _ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let raster = rasterize_glyphs(
        &item.text,
        &item.style,
        item.bounds.width,
        item.allow_wrap,
        item.truncate,
    );

    let rgba = super::paint_conv::color_token_to_rgba(&item.style.color);
    let paint = PaintSpec {
        fill: FillSpec::Solid(rgba),
        style: PaintStyle::Fill,
        stroke: None,
        anti_alias: true,
        blend_mode: BlendMode::SrcOver,
        image_filter: None,
        color_filter: None,
        mask_filter: None,
        path_effect: None,
    };

    for line in &raster.lines {
        let line_x_shift = compute_line_x_shift(line.width, item.bounds.width, item.style.text_align);
        for pos in &line.positions {
            let glyph_data = match raster.glyphs.get(&pos.cache_key) {
                Some(data) => data,
                None => continue,
            };
            let abs_x = item.bounds.x + line_x_shift + pos.x;
            let abs_y = item.bounds.y + pos.y;

            match glyph_data {
                GlyphData::Outline(commands) => {
                    let path = {
                        let mut lru = cache.glyph_paths.borrow_mut();
                        if let Some(cached) = lru.get_cloned(&pos.cache_key) {
                            cached
                        } else {
                            drop(lru);
                            let (verbs, pts) = commands_to_verbs_points(commands);
                            let p = canvas.make_path_from_verbs(&verbs, &pts, FillType::Winding);
                            cache.glyph_paths.borrow_mut().insert(pos.cache_key, p.clone());
                            p
                        }
                    };
                    canvas.save();
                    canvas.translate(abs_x, abs_y);
                    canvas.draw_path(&path, &paint);
                    canvas.restore();
                }
                GlyphData::ColorImage {
                    rgba,
                    width,
                    height,
                    placement_left,
                    placement_top,
                } => {
                    let image = {
                        let mut lru = cache.glyph_images.borrow_mut();
                        if let Some(cached) = lru.get_cloned(&pos.cache_key) {
                            cached
                        } else {
                            drop(lru);
                            let img = canvas.make_image_from_rgba(rgba, *width, *height);
                            cache.glyph_images.borrow_mut().insert(pos.cache_key, img.clone());
                            img
                        }
                    };
                    let ix = abs_x + *placement_left as f32;
                    let iy = abs_y - *placement_top as f32;
                    canvas.draw_image(&image, ix, iy, Some(&paint));
                }
            }
        }
    }
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
