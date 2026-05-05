use skia_safe::{
    AlphaType, Canvas, ColorType, ImageInfo, Paint, PathBuilder, Rect, surfaces,
};
use tracing::{Level, event};

use opencat_core::scene::script::{TextUnitOverride, TextUnitOverrideBatch};
use opencat_core::style::{ComputedTextStyle, TextAlign};
use opencat_core::text::{
    GlyphData, apply_text_transform, compute_line_x_shift, describe_text_unit_ranges,
    ranges_overlap, rasterize_glyphs,
};

use crate::{
    backend::skia::color::skia_color,
    runtime::cache::{GlyphImageCache, GlyphPathCache},
};

pub(crate) fn draw_text(
    canvas: &Canvas,
    text: &str,
    left: f32,
    top: f32,
    width: f32,
    allow_wrap: bool,
    style: &ComputedTextStyle,
    truncate: bool,
    glyph_path_cache: &GlyphPathCache,
    glyph_image_cache: &GlyphImageCache,
) {
    let raster = rasterize_glyphs(text, style, width, allow_wrap, truncate);

    let sk_color = skia_color(style.color);
    let mut color_paint = Paint::default();
    color_paint.set_color(sk_color);
    color_paint.set_anti_alias(true);

    if truncate {
        canvas.save();
        let clip_height = raster
            .lines
            .iter()
            .map(|l| l.y + style.resolved_line_height_px())
            .fold(0.0f32, f32::max);
        canvas.clip_rect(
            Rect::from_xywh(left, top, width, clip_height.max(style.resolved_line_height_px())),
            None,
            None,
        );
    }

    for line in &raster.lines {
        let line_x_shift = compute_line_x_shift(line.width, width, style.text_align);
        for pos in &line.positions {
            let glyph_data = match raster.glyphs.get(&pos.cache_key) {
                Some(data) => data,
                None => continue,
            };
            let abs_x = left + line_x_shift + pos.x;
            let abs_y = top + pos.y;

            draw_single_glyph(
                canvas,
                glyph_data,
                abs_x,
                abs_y,
                pos.cache_key,
                &color_paint,
                glyph_path_cache,
                glyph_image_cache,
            );
        }
    }

    if truncate {
        canvas.restore();
    }
}

pub(crate) fn draw_text_with_unit_overrides(
    canvas: &Canvas,
    text: &str,
    left: f32,
    top: f32,
    width: f32,
    allow_wrap: bool,
    style: &ComputedTextStyle,
    batch: &TextUnitOverrideBatch,
    glyph_path_cache: &GlyphPathCache,
    glyph_image_cache: &GlyphImageCache,
) {
    let rendered = apply_text_transform(text, style.text_transform);
    let raster = rasterize_glyphs(text, style, width, allow_wrap, false);

    let units = describe_text_unit_ranges(&rendered, batch.granularity);
    let sk_color = skia_color(style.color);
    let text_align = if allow_wrap {
        style.text_align
    } else {
        TextAlign::Left
    };

    for (index, unit_range) in units.into_iter().enumerate() {
        let override_value = batch
            .overrides
            .get(index)
            .cloned()
            .unwrap_or_else(TextUnitOverride::default);
        let opacity = override_value.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        if opacity <= 0.0 {
            continue;
        }

        let unit_color = override_value
            .color
            .map(skia_color)
            .unwrap_or(sk_color);

        let mut min_x: f32 = f32::INFINITY;
        let mut min_y: f32 = f32::INFINITY;
        let mut max_x: f32 = f32::NEG_INFINITY;
        let mut max_y: f32 = f32::NEG_INFINITY;

        struct GlyphRender {
            path: Option<skia_safe::Path>,
            path_abs_x: f32,
            path_abs_y: f32,
            color_image: Option<skia_safe::Image>,
            img_rect: Option<Rect>,
            color: skia_safe::Color,
        }

        let mut glyphs: Vec<GlyphRender> = Vec::new();

        for line in &raster.lines {
            let line_x_shift = compute_line_x_shift(line.width, width, text_align);
            for pos in &line.positions {
                if !ranges_overlap(pos.byte_range.clone(), unit_range.clone()) {
                    continue;
                }

                let glyph_data = match raster.glyphs.get(&pos.cache_key) {
                    Some(data) => data,
                    None => continue,
                };
                let abs_x = left + line_x_shift + pos.x;
                let abs_y = top + pos.y;

                match glyph_data {
                    GlyphData::Outline(commands) => {
                        let path = if let Some(cached) =
                            glyph_path_cache.borrow_mut().get_cloned(&pos.cache_key)
                        {
                            cached
                        } else {
                            let p = build_skia_path(commands, 0.0, 0.0);
                            event!(
                                target: "render.cache",
                                Level::TRACE,
                                kind = "cache",
                                name = "glyph_path",
                                result = "miss",
                                amount = 1_u64
                            );
                            glyph_path_cache.borrow_mut().insert(pos.cache_key, p.clone());
                            p
                        };
                        let b = *path.bounds();
                        let gx = abs_x + b.left();
                        let gy = abs_y + b.top();
                        let gw = b.width();
                        let gh = b.height();
                        if gw > 0.0 && gh > 0.0 {
                            min_x = min_x.min(gx);
                            min_y = min_y.min(gy);
                            max_x = max_x.max(gx + gw);
                            max_y = max_y.max(gy + gh);
                        }
                        glyphs.push(GlyphRender {
                            path: Some(path),
                            path_abs_x: abs_x,
                            path_abs_y: abs_y,
                            color_image: None,
                            img_rect: None,
                            color: unit_color,
                        });
                    }
                    GlyphData::ColorImage {
                        rgba,
                        width: im_w,
                        height: im_h,
                        placement_left,
                        placement_top,
                    } => {
                        let ix = abs_x + *placement_left as f32;
                        let iy = abs_y - *placement_top as f32;
                        let iw = *im_w as f32;
                        let ih = *im_h as f32;
                        let ir = Rect::from_xywh(ix, iy, iw, ih);
                        min_x = min_x.min(ix);
                        min_y = min_y.min(iy);
                        max_x = max_x.max(ix + iw);
                        max_y = max_y.max(iy + ih);

                        let skia_img = if let Some(cached) =
                            glyph_image_cache.borrow_mut().get_cloned(&pos.cache_key)
                        {
                            cached
                        } else if let Some(img) = rgba_to_skia_image(rgba, *im_w, *im_h) {
                            event!(
                                target: "render.cache",
                                Level::TRACE,
                                kind = "cache",
                                name = "glyph_image",
                                result = "miss",
                                amount = 1_u64
                            );
                            glyph_image_cache.borrow_mut().insert(pos.cache_key, img.clone());
                            img
                        } else {
                            continue;
                        };

                        glyphs.push(GlyphRender {
                            path: None,
                            path_abs_x: 0.0,
                            path_abs_y: 0.0,
                            color_image: Some(skia_img),
                            img_rect: Some(ir),
                            color: unit_color,
                        });
                    }
                }
            }
        }

        if glyphs.is_empty() || min_x.is_infinite() || min_y.is_infinite() {
            continue;
        }

        let bounds = Rect::from_ltrb(min_x, min_y, max_x, max_y);
        if bounds.is_empty() {
            continue;
        }

        let surf_w = (bounds.width().ceil() as i32).max(1);
        let surf_h = (bounds.height().ceil() as i32).max(1);
        let Some(mut surface) = surfaces::raster_n32_premul((surf_w, surf_h)) else {
            continue;
        };
        let unit_canvas = surface.canvas();
        unit_canvas.clear(skia_safe::Color::TRANSPARENT);
        unit_canvas.save();
        unit_canvas.translate((-bounds.left(), -bounds.top()));

        for g in &glyphs {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);
            paint.set_color(g.color);
            if let Some(ref path) = g.path {
                unit_canvas.save();
                unit_canvas.translate((g.path_abs_x, g.path_abs_y));
                unit_canvas.draw_path(path, &paint);
                unit_canvas.restore();
            }
            if let Some(ref img) = g.color_image {
                if let Some(ref ir) = g.img_rect {
                    unit_canvas.draw_image(img, (ir.left(), ir.top()), Some(&paint));
                }
            }
        }

        unit_canvas.restore();

        let image = surface.image_snapshot();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        if opacity < 1.0 {
            paint.set_alpha((opacity * 255.0).round() as u8);
        }

        let translate_x = override_value.translate_x.unwrap_or(0.0);
        let translate_y = override_value.translate_y.unwrap_or(0.0);
        let scale = override_value.scale.unwrap_or(1.0);
        let rotation_deg = override_value.rotation_deg.unwrap_or(0.0);
        let pivot_x = (bounds.left() + bounds.right()) * 0.5;
        let pivot_y = (bounds.top() + bounds.bottom()) * 0.5;

        canvas.save();
        canvas.translate((pivot_x + translate_x, pivot_y + translate_y));
        if rotation_deg != 0.0 {
            canvas.rotate(rotation_deg, None);
        }
        if (scale - 1.0).abs() > f32::EPSILON {
            canvas.scale((scale, scale));
        }
        canvas.translate((-pivot_x, -pivot_y));
        canvas.draw_image(image, (bounds.left(), bounds.top()), Some(&paint));
        canvas.restore();
    }
}

// ── Skia-specific conversion helpers ──────────────────────────────────────

fn draw_single_glyph(
    canvas: &Canvas,
    glyph_data: &GlyphData,
    x: f32,
    y: f32,
    cache_key: u64,
    color_paint: &Paint,
    glyph_path_cache: &GlyphPathCache,
    glyph_image_cache: &GlyphImageCache,
) {
    match glyph_data {
        GlyphData::Outline(commands) => {
            let path = if let Some(cached) =
                glyph_path_cache.borrow_mut().get_cloned(&cache_key)
            {
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "cache",
                    name = "glyph_path",
                    result = "hit",
                    amount = 1_u64
                );
                cached
            } else {
                let p = build_skia_path(commands, 0.0, 0.0);
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "cache",
                    name = "glyph_path",
                    result = "miss",
                    amount = 1_u64
                );
                glyph_path_cache.borrow_mut().insert(cache_key, p.clone());
                p
            };
            canvas.save();
            canvas.translate((x, y));
            canvas.draw_path(&path, color_paint);
            canvas.restore();
        }
        GlyphData::ColorImage {
            rgba,
            width: im_w,
            height: im_h,
            placement_left,
            placement_top,
        } => {
            let ix = x + *placement_left as f32;
            let iy = y - *placement_top as f32;

            let skia_img = if let Some(cached) =
                glyph_image_cache.borrow_mut().get_cloned(&cache_key)
            {
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "cache",
                    name = "glyph_image",
                    result = "hit",
                    amount = 1_u64
                );
                cached
            } else if let Some(img) = rgba_to_skia_image(rgba, *im_w, *im_h) {
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "cache",
                    name = "glyph_image",
                    result = "miss",
                    amount = 1_u64
                );
                glyph_image_cache.borrow_mut().insert(cache_key, img.clone());
                img
            } else {
                return;
            };

            canvas.draw_image(skia_img, (ix, iy), Some(color_paint));
        }
    }
}

fn build_skia_path(commands: &[cosmic_text::Command], offset_x: f32, offset_y: f32) -> skia_safe::Path {
    use cosmic_text::Command;

    let mut pb = PathBuilder::new();
    for cmd in commands {
        match cmd {
            Command::MoveTo(p) => {
                pb.move_to((p.x + offset_x, -p.y + offset_y));
            }
            Command::LineTo(p) => {
                pb.line_to((p.x + offset_x, -p.y + offset_y));
            }
            Command::QuadTo(c, p) => {
                pb.quad_to(
                    (c.x + offset_x, -c.y + offset_y),
                    (p.x + offset_x, -p.y + offset_y),
                );
            }
            Command::CurveTo(c1, c2, p) => {
                pb.cubic_to(
                    (c1.x + offset_x, -c1.y + offset_y),
                    (c2.x + offset_x, -c2.y + offset_y),
                    (p.x + offset_x, -p.y + offset_y),
                );
            }
            Command::Close => {
                pb.close();
            }
        }
    }
    pb.snapshot()
}

fn rgba_to_skia_image(rgba: &[u8], width: u32, height: u32) -> Option<skia_safe::Image> {
    let w = width as i32;
    let h = height as i32;
    if w <= 0 || h <= 0 {
        return None;
    }
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Unpremul, None);
    skia_safe::images::raster_from_data(&info, skia_safe::Data::new_copy(rgba), (w * 4) as usize)
}
