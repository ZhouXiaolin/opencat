use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    ops::Range,
    sync::{Arc, OnceLock},
};

use skia_safe::{
    Canvas, FontMgr, FontStyle, Paint, PathBuilder, Rect, Typeface,
    font_style::{Slant, Weight, Width},
    surfaces,
    textlayout::{
        FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, RectHeightStyle,
        RectWidthStyle, TextAlign as ParagraphAlign, TextBox, TextDecoration,
        TextStyle as ParagraphTextStyle, TypefaceFontProvider,
    },
};

use crate::{
    backend::skia::color::skia_color,
    runtime::text_engine::{SharedTextEngine, TextEngine, TextMeasureRequest, TextMeasurement},
    scene::script::{TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch},
    style::{ComputedTextStyle, FontWeight, TextAlign, TextTransform},
};
use unicode_segmentation::UnicodeSegmentation;

static EMOJI_FONT_DATA: &[u8] = include_bytes!("../../../assets/NotoColorEmoji.ttf");
const UNBOUNDED_LAYOUT_WIDTH: f32 = 100_000.0;

thread_local! {
    static TEXT_MEASURE_CACHE: RefCell<HashMap<u64, (f32, f32)>> = RefCell::new(HashMap::new());
    static SHARED_FONT_COLLECTION: RefCell<Option<FontCollection>> = const { RefCell::new(None) };
    static EMOJI_TYPEFACE: RefCell<Option<Option<Typeface>>> = const { RefCell::new(None) };
}

pub(crate) struct SkiaTextEngine;

pub(crate) fn shared_text_engine() -> SharedTextEngine {
    static TEXT_ENGINE: OnceLock<SharedTextEngine> = OnceLock::new();
    TEXT_ENGINE
        .get_or_init(|| Arc::new(SkiaTextEngine) as SharedTextEngine)
        .clone()
}

impl TextEngine for SkiaTextEngine {
    fn measure(&self, request: &TextMeasureRequest<'_>) -> TextMeasurement {
        let layout_width = if request.allow_wrap {
            request.max_width
        } else {
            f32::INFINITY
        };
        let normalized_width = normalize_width(layout_width);
        let cache_key = text_measure_cache_key(request.text, request.style, normalized_width);

        if let Some(measured) =
            TEXT_MEASURE_CACHE.with(|cache| cache.borrow().get(&cache_key).copied())
        {
            return TextMeasurement {
                width: measured.0,
                height: measured.1,
            };
        }

        let paragraph = make_paragraph(request.text, request.style, normalized_width);
        let measured = (
            paragraph.longest_line().max(1.0),
            paragraph.height().max(1.0),
        );
        TEXT_MEASURE_CACHE.with(|cache| {
            cache.borrow_mut().insert(cache_key, measured);
        });
        TextMeasurement {
            width: measured.0,
            height: measured.1,
        }
    }
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
    let rendered_text = apply_text_transform(text, style.text_transform);
    let layout_width = if truncate {
        if !width.is_finite() || width <= 0.0 {
            // truncate 模式下容器宽度无效（NaN/0/负值）：单行省略后不会有任何可见像素，
            // 也无法构造 clip_rect，直接 bail-out 比交给 Skia 处理更安全。
            return;
        }
        width
    } else if allow_wrap {
        width
    } else {
        f32::INFINITY
    };

    let paragraph = if truncate {
        make_truncated_paragraph(&rendered_text, style, layout_width)
    } else {
        make_paragraph_from_text(&rendered_text, style, layout_width)
    };

    if truncate {
        canvas.save();
        canvas.clip_rect(
            Rect::from_xywh(left, top, width, paragraph.height()),
            None,
            None,
        );
    }

    paragraph.paint(canvas, (left, top));

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
    let layout_width = if allow_wrap { width } else { f32::INFINITY };
    let rendered_text = apply_text_transform(text, style.text_transform);
    let paragraph = make_paragraph_from_text(&rendered_text, style, layout_width);
    // Use the original (pre-transform) text for unit segmentation so that
    // JS-side split indices align with the rendered units.
    let units = describe_text_unit_ranges(text, batch.granularity);

    for (index, unit) in units.into_iter().enumerate() {
        let override_value = batch
            .overrides
            .get(index)
            .cloned()
            .unwrap_or_else(TextUnitOverride::default);
        let opacity = override_value.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        if opacity <= 0.0 {
            continue;
        }

        let boxes = paragraph.get_rects_for_range(
            unit.start..unit.end,
            RectHeightStyle::Max,
            RectWidthStyle::Tight,
        );
        let Some((clip_path, unit_bounds)) = build_text_unit_clip(&boxes, left, top) else {
            continue;
        };

        let Some(mut unit_surface) = surfaces::raster_n32_premul((
            unit_bounds.width().ceil().max(1.0) as i32,
            unit_bounds.height().ceil().max(1.0) as i32,
        )) else {
            continue;
        };

        let unit_canvas = unit_surface.canvas();
        unit_canvas.clear(skia_safe::Color::TRANSPARENT);
        unit_canvas.save();
        unit_canvas.translate((-unit_bounds.left, -unit_bounds.top));
        unit_canvas.clip_path(&clip_path, skia_safe::ClipOp::Intersect, true);
        if let Some(color) = override_value.color {
            let mut unit_style = *style;
            unit_style.color = color;
            let unit_paragraph =
                make_paragraph_from_text(&rendered_text, &unit_style, layout_width);
            unit_paragraph.paint(unit_canvas, (left, top));
        } else {
            paragraph.paint(unit_canvas, (left, top));
        }
        unit_canvas.restore();

        let image = unit_surface.image_snapshot();
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        if opacity < 1.0 {
            paint.set_alpha((opacity * 255.0).round() as u8);
        }

        let translate_x = override_value.translate_x.unwrap_or(0.0);
        let translate_y = override_value.translate_y.unwrap_or(0.0);
        let scale = override_value.scale.unwrap_or(1.0);
        let rotation_deg = override_value.rotation_deg.unwrap_or(0.0);
        let pivot_x = unit_bounds.center_x();
        let pivot_y = unit_bounds.center_y();

        canvas.save();
        canvas.translate((pivot_x + translate_x, pivot_y + translate_y));
        if rotation_deg != 0.0 {
            canvas.rotate(rotation_deg, None);
        }
        if scale != 1.0 {
            canvas.scale((scale, scale));
        }
        canvas.translate((-pivot_x, -pivot_y));
        canvas.draw_image(image, (unit_bounds.left, unit_bounds.top), Some(&paint));
        canvas.restore();
    }
}

fn get_emoji_typeface() -> Option<Typeface> {
    EMOJI_TYPEFACE.with(|cached| {
        let mut cached = cached.borrow_mut();
        if let Some(typeface) = cached.as_ref() {
            return typeface.clone();
        }

        let font_mgr = FontMgr::new();
        let typeface = font_mgr.new_from_data(EMOJI_FONT_DATA, 0);
        *cached = Some(typeface.clone());
        typeface
    })
}

fn shared_font_collection() -> FontCollection {
    SHARED_FONT_COLLECTION.with(|cached| {
        let mut cached = cached.borrow_mut();
        if let Some(collection) = cached.as_ref() {
            return collection.clone();
        }

        let mut font_collection = FontCollection::new();
        font_collection.set_default_font_manager(FontMgr::new(), None);

        if let Some(typeface) = get_emoji_typeface() {
            let mut emoji_provider = TypefaceFontProvider::new();
            emoji_provider.register_typeface(typeface, Some("Noto Color Emoji"));
            font_collection.set_asset_font_manager(Some(emoji_provider.into()));
        }

        *cached = Some(font_collection.clone());
        font_collection
    })
}

fn make_paragraph(text: &str, style: &ComputedTextStyle, max_width: f32) -> Paragraph {
    let text = apply_text_transform(text, style.text_transform);
    make_paragraph_from_text(&text, style, max_width)
}

fn make_paragraph_from_text(text: &str, style: &ComputedTextStyle, max_width: f32) -> Paragraph {
    let text_style = paragraph_text_style(style);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(paragraph_align(style.text_align));
    paragraph_style.set_text_style(&text_style);

    let mut builder = ParagraphBuilder::new(&paragraph_style, shared_font_collection());
    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(normalize_width(max_width));
    paragraph
}

fn make_truncated_paragraph(text: &str, style: &ComputedTextStyle, max_width: f32) -> Paragraph {
    let text_style = paragraph_text_style(style);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(paragraph_align(style.text_align));
    paragraph_style.set_text_style(&text_style);
    paragraph_style.set_ellipsis("…");
    paragraph_style.set_max_lines(1);

    let mut builder = ParagraphBuilder::new(&paragraph_style, shared_font_collection());
    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(normalize_width(max_width));
    paragraph
}

fn paragraph_text_style(style: &ComputedTextStyle) -> ParagraphTextStyle {
    let mut text_style = ParagraphTextStyle::new();
    text_style.set_color(skia_color(style.color));
    text_style.set_font_size(style.text_px);
    text_style.set_font_style(font_style(style.font_weight));
    text_style.set_letter_spacing(style.letter_spacing);
    text_style.set_height(style.resolved_line_height_px() / style.text_px);
    text_style.set_height_override(true);
    if style.line_through {
        text_style.set_decoration_type(TextDecoration::LINE_THROUGH);
        text_style.set_decoration_color(skia_color(style.color));
    }
    text_style
}

fn build_text_unit_clip(boxes: &[TextBox], left: f32, top: f32) -> Option<(skia_safe::Path, Rect)> {
    let mut bounds: Option<Rect> = None;
    let mut builder = PathBuilder::new();

    for text_box in boxes {
        let rect = text_box.rect.with_offset((left, top));
        builder.add_rect(rect, None::<skia_safe::PathDirection>, None::<usize>);
        match &mut bounds {
            Some(current) => current.join(rect),
            None => bounds = Some(rect),
        }
    }

    bounds.map(|bounds| (builder.snapshot(), bounds))
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

fn font_style(weight: FontWeight) -> FontStyle {
    let weight = match weight {
        FontWeight::Light => 300,
        FontWeight::Normal => 400,
        FontWeight::Medium => 500,
        FontWeight::SemiBold => 600,
        FontWeight::Bold => 700,
    };
    FontStyle::new(Weight::from(weight), Width::NORMAL, Slant::Upright)
}

fn paragraph_align(align: TextAlign) -> ParagraphAlign {
    match align {
        TextAlign::Left => ParagraphAlign::Left,
        TextAlign::Center => ParagraphAlign::Center,
        TextAlign::Right => ParagraphAlign::Right,
    }
}

fn normalize_width(width: f32) -> f32 {
    if width.is_finite() && width > 0.0 {
        width
    } else {
        UNBOUNDED_LAYOUT_WIDTH
    }
}

fn apply_text_transform(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
    }
}

fn text_measure_cache_key(text: &str, style: &ComputedTextStyle, width: f32) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    apply_text_transform(text, style.text_transform).hash(&mut hasher);
    style.color.hash(&mut hasher);
    style.font_weight.hash(&mut hasher);
    style.text_align.hash(&mut hasher);
    style.text_px.to_bits().hash(&mut hasher);
    style.letter_spacing.to_bits().hash(&mut hasher);
    style.line_height.to_bits().hash(&mut hasher);
    style.line_height_px.map(f32::to_bits).hash(&mut hasher);
    style.text_transform.hash(&mut hasher);
    width.to_bits().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::{apply_text_transform, shared_text_engine};
    use crate::{
        runtime::text_engine::TextMeasureRequest,
        style::{ComputedTextStyle, TextTransform},
    };

    #[test]
    fn textlayout_wraps_long_cjk_text_in_narrow_width() {
        let style = ComputedTextStyle::default();
        let single_line_height = style.resolved_line_height_px();
        let measured = shared_text_engine().measure(&TextMeasureRequest {
            text: "这是一个没有空格但应该自动换行的很长中文句子",
            style: &style,
            max_width: 80.0,
            allow_wrap: true,
        });

        assert!(
            measured.height > single_line_height,
            "expected narrow text layout to wrap into multiple lines, got height {}",
            measured.height
        );
    }

    #[test]
    fn textlayout_applies_uppercase_transform() {
        assert_eq!(
            apply_text_transform("Physics Education Series", TextTransform::Uppercase),
            "PHYSICS EDUCATION SERIES"
        );
    }
}
