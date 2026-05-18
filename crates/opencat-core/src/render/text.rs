#[cfg(feature = "profile")]
use tracing::{Level, event};

use cosmic_text::Command;

use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};
use crate::canvas::{Canvas2D, FillType, Rect};
use crate::display::list::{DisplayRect, TextDisplayItem};
use crate::scene::script::TextUnitOverride;
use crate::style::TextAlign;
use crate::text::{
    GlyphData, apply_text_transform, compute_line_x_shift, describe_text_unit_ranges,
    ranges_overlap, rasterize_glyphs,
};

use super::paint_conv::drop_shadow_to_image_filter;
use super::{record_cache_pressure, RenderCache, RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

pub fn commands_to_verbs_points(commands: &[Command], scale: f32) -> (Vec<u8>, Vec<f32>) {
    let mut verbs = Vec::with_capacity(commands.len());
    let mut points = Vec::new();
    for cmd in commands {
        match cmd {
            Command::MoveTo(p) => {
                verbs.push(0);
                points.push(p.x * scale);
                points.push(-p.y * scale);
            }
            Command::LineTo(p) => {
                verbs.push(1);
                points.push(p.x * scale);
                points.push(-p.y * scale);
            }
            Command::QuadTo(c, p) => {
                verbs.push(2);
                points.push(c.x * scale);
                points.push(-c.y * scale);
                points.push(p.x * scale);
                points.push(-p.y * scale);
            }
            Command::CurveTo(c1, c2, p) => {
                verbs.push(4);
                points.push(c1.x * scale);
                points.push(-c1.y * scale);
                points.push(c2.x * scale);
                points.push(-c2.y * scale);
                points.push(p.x * scale);
                points.push(-p.y * scale);
            }
            Command::Close => {
                verbs.push(5);
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
                GlyphData::Outline(commands, upem) => {
                    let draw_scale = item.style.text_px / *upem;
                    let unscale = *upem / item.style.text_px;
                    let path = {
                        let mut lru = cache.glyph_paths.borrow_mut();
                        if let Some(cached) = lru.get_cloned(&pos.outline_key) {
                            #[cfg(feature = "profile")]
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
                            drop(lru);
                            let (verbs, pts) = commands_to_verbs_points(commands, unscale);
                            let weight = verbs.len() + pts.len();
                            let p = canvas.make_path_from_verbs(&verbs, &pts, FillType::Winding);
                            let report = cache.glyph_paths.borrow_mut().insert_with_weight(pos.outline_key, p.clone(), weight.max(1));
                            record_cache_pressure("glyph_path", &report);
                            #[cfg(feature = "profile")]
                            event!(
                                target: "render.cache",
                                Level::TRACE,
                                kind = "cache",
                                name = "glyph_path",
                                result = "miss",
                                amount = 1_u64
                            );
                            p
                        }
                    };
                    canvas.save();
                    canvas.translate(abs_x, abs_y);
                    if (draw_scale - 1.0).abs() > f32::EPSILON {
                        canvas.scale(draw_scale, draw_scale);
                    }
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
                            #[cfg(feature = "profile")]
                            event!(
                                target: "render.cache",
                                Level::TRACE,
                                kind = "cache",
                                name = "glyph_image",
                                result = "hit",
                                amount = 1_u64
                            );
                            cached
                        } else {
                            drop(lru);
                            let weight = (*width * *height * 4) as usize;
                            let img = canvas.make_image_from_rgba(rgba, *width, *height);
                            let report = cache.glyph_images.borrow_mut().insert_with_weight(pos.cache_key, img.clone(), weight.max(1));
                            record_cache_pressure("glyph_image", &report);
                            #[cfg(feature = "profile")]
                            event!(
                                target: "render.cache",
                                Level::TRACE,
                                kind = "cache",
                                name = "glyph_image",
                                result = "miss",
                                amount = 1_u64
                            );
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

fn render_text_with_unit_overrides<C: Canvas2D>(
    canvas: &mut C,
    item: &TextDisplayItem,
    _ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let batch = item.text_unit_overrides.as_ref().unwrap();
    let rendered = apply_text_transform(&item.text, item.style.text_transform);
    let raster = rasterize_glyphs(
        &item.text,
        &item.style,
        item.bounds.width,
        item.allow_wrap,
        false,
    );

    let base_rgba = super::paint_conv::color_token_to_rgba(&item.style.color);
    let text_align = if item.allow_wrap {
        item.style.text_align
    } else {
        TextAlign::Left
    };
    let units = describe_text_unit_ranges(&rendered, batch.granularity);

    for (index, unit_range) in units.into_iter().enumerate() {
        let ov = batch
            .overrides
            .get(index)
            .cloned()
            .unwrap_or_else(TextUnitOverride::default);
        let opacity = ov.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        if opacity <= 0.0 {
            continue;
        }

        let unit_rgba = ov
            .color
            .as_ref()
            .map(super::paint_conv::color_token_to_rgba)
            .unwrap_or(base_rgba);
        let unit_paint = PaintSpec {
            fill: FillSpec::Solid(unit_rgba),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };

        let mut min_x: f32 = f32::INFINITY;
        let mut min_y: f32 = f32::INFINITY;
        let mut max_x: f32 = f32::NEG_INFINITY;
        let mut max_y: f32 = f32::NEG_INFINITY;

        struct GlyphEntry<P, I> {
            kind: GlyphEntryKind<P, I>,
            abs_x: f32,
            abs_y: f32,
            scale: f32,
        }
        enum GlyphEntryKind<P, I> {
            Path(P),
            Image { image: I, rect: (f32, f32, f32, f32) },
        }

        let mut entries: Vec<GlyphEntry<C::Path, C::Image>> = Vec::new();

        for line in &raster.lines {
            let line_x_shift = compute_line_x_shift(line.width, item.bounds.width, text_align);
            for pos in &line.positions {
                if !ranges_overlap(pos.byte_range.clone(), unit_range.clone()) {
                    continue;
                }
                let glyph_data = match raster.glyphs.get(&pos.cache_key) {
                    Some(d) => d,
                    None => continue,
                };
                let abs_x = item.bounds.x + line_x_shift + pos.x;
                let abs_y = item.bounds.y + pos.y;

                match glyph_data {
                    GlyphData::Outline(commands, upem) => {
                        let draw_scale = item.style.text_px / *upem;
                        let unscale = *upem / item.style.text_px;
                        let path = {
                            let mut lru = cache.glyph_paths.borrow_mut();
                            if let Some(cached) = lru.get_cloned(&pos.outline_key) {
                                #[cfg(feature = "profile")]
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
                                drop(lru);
                                let (verbs, pts) = commands_to_verbs_points(commands, unscale);
                                let weight = verbs.len() + pts.len();
                                let p =
                                    canvas.make_path_from_verbs(&verbs, &pts, FillType::Winding);
                                let report = cache
                                    .glyph_paths
                                    .borrow_mut()
                                    .insert_with_weight(pos.outline_key, p.clone(), weight.max(1));
                                record_cache_pressure("glyph_path", &report);
                                #[cfg(feature = "profile")]
                                event!(
                                    target: "render.cache",
                                    Level::TRACE,
                                    kind = "cache",
                                    name = "glyph_path",
                                    result = "miss",
                                    amount = 1_u64
                                );
                                p
                            }
                        };
                        let (bx, by, bw, bh) = outline_bounds(commands, unscale);
                        let gx = abs_x + bx * draw_scale;
                        let gy = abs_y + by * draw_scale;
                        if bw > 0.0 && bh > 0.0 {
                            min_x = min_x.min(gx);
                            min_y = min_y.min(gy);
                            max_x = max_x.max(gx + bw * draw_scale);
                            max_y = max_y.max(gy + bh * draw_scale);
                        }
                        entries.push(GlyphEntry {
                            kind: GlyphEntryKind::Path(path),
                            abs_x,
                            abs_y,
                            scale: draw_scale,
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
                        min_x = min_x.min(ix);
                        min_y = min_y.min(iy);
                        max_x = max_x.max(ix + iw);
                        max_y = max_y.max(iy + ih);

                        let image = {
                            let mut lru = cache.glyph_images.borrow_mut();
                            if let Some(cached) = lru.get_cloned(&pos.cache_key) {
                                #[cfg(feature = "profile")]
                                event!(
                                    target: "render.cache",
                                    Level::TRACE,
                                    kind = "cache",
                                    name = "glyph_image",
                                    result = "hit",
                                    amount = 1_u64
                                );
                                cached
                            } else {
                                drop(lru);
                                let weight = (*im_w * *im_h * 4) as usize;
                                let img = canvas.make_image_from_rgba(rgba, *im_w, *im_h);
                                let report = cache
                                    .glyph_images
                                    .borrow_mut()
                                    .insert_with_weight(pos.cache_key, img.clone(), weight.max(1));
                                record_cache_pressure("glyph_image", &report);
                                #[cfg(feature = "profile")]
                                event!(
                                    target: "render.cache",
                                    Level::TRACE,
                                    kind = "cache",
                                    name = "glyph_image",
                                    result = "miss",
                                    amount = 1_u64
                                );
                                img
                            }
                        };
                        entries.push(GlyphEntry {
                            kind: GlyphEntryKind::Image {
                                image,
                                rect: (ix, iy, iw, ih),
                            },
                            abs_x,
                            abs_y,
                            scale: 1.0,
                        });
                    }
                }
            }
        }

        if entries.is_empty() || min_x.is_infinite() || min_y.is_infinite() {
            continue;
        }

        let bw = (max_x - min_x).ceil().max(1.0) as u32;
        let bh = (max_y - min_y).ceil().max(1.0) as u32;

        let unit_image = canvas.render_to_image(bw, bh, |offscreen| {
            offscreen.translate(-min_x, -min_y);
            for entry in &entries {
                match &entry.kind {
                    GlyphEntryKind::Path(path) => {
                        offscreen.save();
                        offscreen.translate(entry.abs_x, entry.abs_y);
                        if (entry.scale - 1.0).abs() > f32::EPSILON {
                            offscreen.scale(entry.scale, entry.scale);
                        }
                        offscreen.draw_path(path, &unit_paint);
                        offscreen.restore();
                    }
                    GlyphEntryKind::Image { image, rect } => {
                        offscreen.draw_image(image, rect.0, rect.1, Some(&unit_paint));
                    }
                }
            }
        });

        let translate_x = ov.translate_x.unwrap_or(0.0);
        let translate_y = ov.translate_y.unwrap_or(0.0);
        let scale = ov.scale.unwrap_or(1.0);
        let rotation_deg = ov.rotation_deg.unwrap_or(0.0);
        let pivot_x = (min_x + max_x) * 0.5;
        let pivot_y = (min_y + max_y) * 0.5;

        let mut draw_paint = PaintSpec {
            fill: FillSpec::Solid([1.0, 1.0, 1.0, 1.0]),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };
        if opacity < 1.0 {
            draw_paint.fill = FillSpec::Solid([1.0, 1.0, 1.0, opacity]);
        }

        canvas.save();
        canvas.translate(pivot_x + translate_x, pivot_y + translate_y);
        if rotation_deg != 0.0 {
            canvas.rotate(rotation_deg, 0.0, 0.0);
        }
        if (scale - 1.0).abs() > f32::EPSILON {
            canvas.scale(scale, scale);
        }
        canvas.translate(-pivot_x, -pivot_y);
        canvas.draw_image(&unit_image, min_x, min_y, Some(&draw_paint));
        canvas.restore();
    }

    Ok(())
}

fn outline_bounds(commands: &[Command], scale: f32) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut track = |x: f32, y: f32| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    };
    for cmd in commands {
        match cmd {
            Command::MoveTo(p) | Command::LineTo(p) => track(p.x * scale, -p.y * scale),
            Command::QuadTo(c, p) => {
                track(c.x * scale, -c.y * scale);
                track(p.x * scale, -p.y * scale);
            }
            Command::CurveTo(c1, c2, p) => {
                track(c1.x * scale, -c1.y * scale);
                track(c2.x * scale, -c2.y * scale);
                track(p.x * scale, -p.y * scale);
            }
            Command::Close => {}
        }
    }
    if min_x.is_infinite() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

pub fn render_text_with_shadows<C: Canvas2D>(
    canvas: &mut C,
    item: &TextDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let has_overrides = item.text_unit_overrides.is_some();
    let render_fn = |c: &mut C, i: &TextDisplayItem, x: &RenderCtx<C>, ca: &mut RenderCache<C>| {
        if has_overrides {
            render_text_with_unit_overrides(c, i, x, ca)
        } else {
            render_text(c, i, x, ca)
        }
    };

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
        render_fn(canvas, item, ctx, cache)?;
        canvas.restore();
    }
    render_fn(canvas, item, ctx, cache)?;
    Ok(())
}
