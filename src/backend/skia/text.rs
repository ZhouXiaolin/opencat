use std::ops::Range;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use skia_safe::{
    AlphaType, Canvas, ColorType, ImageInfo, Paint, PathBuilder, Rect, surfaces,
};

use crate::{
    backend::skia::color::skia_color,
    scene::script::{TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch},
    style::{ComputedTextStyle, TextAlign, TextTransform},
};
use unicode_segmentation::UnicodeSegmentation;

thread_local! {
    static FONT_DB: fontdb::Database = crate::text::default_font_db(&[]);
}

fn get_font_db() -> fontdb::Database {
    FONT_DB.with(|db| db.clone())
}

pub(crate) fn draw_text(
    canvas: &Canvas,
    text: &str,
    left: f32,
    top: f32,
    width: f32,
    allow_wrap: bool,
    style: &ComputedTextStyle,
    truncate: bool,
) {
    let rendered = apply_text_transform(text, style.text_transform);
    if truncate && (!width.is_finite() || width <= 0.0) {
        return;
    }

    let layout_width = if truncate || allow_wrap {
        if width.is_finite() && width > 0.0 {
            Some(width)
        } else {
            None
        }
    } else {
        None
    };

    let font_db = get_font_db();
    let line_height = style.resolved_line_height_px();
    let metrics = Metrics::new(style.text_px, line_height);

    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".to_string(), font_db);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(&mut font_system, layout_width, None);

    let attrs = Attrs::new()
        .family(cosmic_text::Family::SansSerif)
        .weight(cosmic_text::Weight(style.font_weight.0));
    buffer.set_text(&mut font_system, &rendered, attrs, Shaping::Advanced);

    let mut swash_cache = SwashCache::new();
    let sk_color = skia_color(style.color);

    let text_align = style.text_align;

    if truncate {
        canvas.save();
        let clip_height = compute_text_height(&buffer);
        canvas.clip_rect(
            Rect::from_xywh(left, top, width, clip_height.max(line_height)),
            None,
            None,
        );
    }

    for run in buffer.layout_runs() {
        let line_x_shift = compute_line_x_shift(run.line_w, width, text_align);
        for glyph in run.glyphs {
            let physical = glyph.physical((0.0, 0.0), 1.0);

            let swash_image = swash_cache.get_image(&mut font_system, physical.cache_key);
            if let Some(image) = swash_image {
                if image.content == SwashContent::Color {
                    let skia_img = swash_image_to_skia(image);
                    if let Some(img) = skia_img {
                        let abs_x = left + line_x_shift + physical.x as f32;
                        let abs_y = top + run.line_y + physical.y as f32;
                        let x = abs_x + image.placement.left as f32;
                        let y = abs_y - image.placement.top as f32;
                        let mut paint = Paint::default();
                        paint.set_anti_alias(true);
                        canvas.draw_image(img, (x, y), Some(&paint));
                    }
                    continue;
                }
            }

            if let Some(commands) =
                swash_cache.get_outline_commands(&mut font_system, physical.cache_key)
            {
                let abs_x = left + line_x_shift + physical.x as f32
                    + physical.cache_key.x_bin.as_float();
                let abs_y = top + run.line_y + physical.y as f32
                    + physical.cache_key.y_bin.as_float();

                let path = build_skia_path(commands, abs_x, abs_y);
                let mut paint = Paint::default();
                paint.set_color(sk_color);
                paint.set_anti_alias(true);
                canvas.draw_path(&path, &paint);
            }
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
) {
    let rendered = apply_text_transform(text, style.text_transform);
    let layout_width = if allow_wrap && width.is_finite() && width > 0.0 {
        Some(width)
    } else {
        None
    };

    let font_db = get_font_db();
    let line_height = style.resolved_line_height_px();
    let metrics = Metrics::new(style.text_px, line_height);

    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".to_string(), font_db);
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(&mut font_system, layout_width, None);

    let attrs = Attrs::new()
        .family(cosmic_text::Family::SansSerif)
        .weight(cosmic_text::Weight(style.font_weight.0));
    buffer.set_text(&mut font_system, &rendered, attrs, Shaping::Advanced);

    let units = describe_text_unit_ranges(text, batch.granularity);

    let mut swash_cache = SwashCache::new();
    let sk_color = skia_color(style.color);

    let text_align = if allow_wrap { style.text_align } else { TextAlign::Left };

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
            color_image: Option<skia_safe::Image>,
            img_rect: Option<Rect>,
            color: skia_safe::Color,
        }

        let mut glyphs: Vec<GlyphRender> = Vec::new();

        for run in buffer.layout_runs() {
            let line_x_shift = compute_line_x_shift(run.line_w, width, text_align);
            for glyph in run.glyphs {
                if !ranges_overlap(glyph.start..glyph.end, unit_range.clone()) {
                    continue;
                }

                let physical = glyph.physical((0.0, 0.0), 1.0);
                let abs_x = left + line_x_shift + physical.x as f32
                    + physical.cache_key.x_bin.as_float();
                let abs_y = top + run.line_y + physical.y as f32
                    + physical.cache_key.y_bin.as_float();

                let swash_image =
                    swash_cache.get_image(&mut font_system, physical.cache_key);
                if let Some(image) = swash_image {
                    if image.content == SwashContent::Color {
                        if let Some(img) = swash_image_to_skia(image) {
                            let ix = abs_x + image.placement.left as f32;
                            let iy = abs_y - image.placement.top as f32;
                            let iw = image.placement.width as f32;
                            let ih = image.placement.height as f32;
                            let ir = Rect::from_xywh(ix, iy, iw, ih);
                            min_x = min_x.min(ix);
                            min_y = min_y.min(iy);
                            max_x = max_x.max(ix + iw);
                            max_y = max_y.max(iy + ih);
                            glyphs.push(GlyphRender {
                                path: None,
                                color_image: Some(img),
                                img_rect: Some(ir),
                                color: unit_color,
                            });
                        }
                        continue;
                    }
                }

                if let Some(commands) =
                    swash_cache.get_outline_commands(&mut font_system, physical.cache_key)
                {
                    let path = build_skia_path(commands, abs_x, abs_y);
                    let b = *path.bounds();
                    if b.width() > 0.0 && b.height() > 0.0 {
                        min_x = min_x.min(b.left());
                        min_y = min_y.min(b.top());
                        max_x = max_x.max(b.right());
                        max_y = max_y.max(b.bottom());
                    }
                    glyphs.push(GlyphRender {
                        path: Some(path),
                        color_image: None,
                        img_rect: None,
                        color: unit_color,
                    });
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
                unit_canvas.draw_path(path, &paint);
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

fn compute_line_x_shift(line_w: f32, container_w: f32, align: TextAlign) -> f32 {
    match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => (container_w - line_w) * 0.5,
        TextAlign::Right => container_w - line_w,
    }
}

fn compute_text_height(buffer: &Buffer) -> f32 {
    let mut height: f32 = 0.0;
    for run in buffer.layout_runs() {
        height = height.max(run.line_top + run.line_height);
    }
    height
}

fn swash_image_to_skia(image: &cosmic_text::SwashImage) -> Option<skia_safe::Image> {
    let w = image.placement.width as i32;
    let h = image.placement.height as i32;
    if w <= 0 || h <= 0 {
        return None;
    }
    let info = ImageInfo::new(
        (w, h),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    let data: Vec<u8> = match image.content {
        SwashContent::Color => image.data.clone(),
        SwashContent::Mask => {
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for &alpha in &image.data {
                rgba.extend_from_slice(&[255, 255, 255, alpha]);
            }
            rgba
        }
        SwashContent::SubpixelMask => {
            return None;
        }
    };
    skia_safe::images::raster_from_data(
        &info,
        skia_safe::Data::new_copy(&data),
        (w * 4) as usize,
    )
}

fn build_skia_path(
    commands: &[cosmic_text::Command],
    offset_x: f32,
    offset_y: f32,
) -> skia_safe::Path {
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

fn describe_text_unit_ranges(text: &str, granularity: TextUnitGranularity) -> Vec<Range<usize>> {
    match granularity {
        TextUnitGranularity::Grapheme => describe_grapheme_ranges(text),
        TextUnitGranularity::Word => {
            if contains_cjk(text) {
                return describe_grapheme_ranges(text);
            }
            UnicodeSegmentation::split_word_bounds(text)
                .filter(|segment| !segment.is_empty())
                .scan(0usize, |offset, segment| {
                    let start = *offset;
                    *offset += segment.len();
                    Some(start..*offset)
                })
                .collect()
        }
    }
}

fn describe_grapheme_ranges(text: &str) -> Vec<Range<usize>> {
    UnicodeSegmentation::graphemes(text, true)
        .scan(0usize, |offset, grapheme| {
            let start = *offset;
            *offset += grapheme.len();
            Some(start..*offset)
        })
        .collect()
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2A6DF
                | 0x2A700..=0x2B73F
                | 0x2B740..=0x2B81F
                | 0x2B820..=0x2CEAF
                | 0x3040..=0x309F
                | 0x30A0..=0x30FF
                | 0xAC00..=0xD7AF
        )
    })
}

fn apply_text_transform(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
    }
}

fn ranges_overlap(a: Range<usize>, b: Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

#[cfg(test)]
mod tests {
    use super::apply_text_transform;
    use crate::style::TextTransform;

    #[test]
    fn textlayout_applies_uppercase_transform() {
        assert_eq!(
            apply_text_transform("Physics Education Series", TextTransform::Uppercase),
            "PHYSICS EDUCATION SERIES"
        );
    }
}
