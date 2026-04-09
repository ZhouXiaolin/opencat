use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, OnceLock},
};

use skia_safe::{
    Canvas, FontMgr, FontStyle, Typeface,
    font_style::{Slant, Weight, Width},
    textlayout::{
        FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextAlign as ParagraphAlign,
        TextStyle as ParagraphTextStyle, TypefaceFontProvider,
    },
};

use crate::{
    backend::skia::color::skia_color,
    runtime::text_engine::{SharedTextEngine, TextEngine, TextMeasureRequest, TextMeasurement},
    style::{ComputedTextStyle, FontWeight, TextAlign, TextTransform},
};

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
) {
    let layout_width = if allow_wrap { width } else { f32::INFINITY };
    let paragraph = make_paragraph(text, style, layout_width);
    paragraph.paint(canvas, (left, top));
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
    let mut text_style = ParagraphTextStyle::new();
    text_style.set_color(skia_color(style.color));
    text_style.set_font_size(style.text_px);
    text_style.set_font_style(font_style(style.font_weight));
    text_style.set_letter_spacing(style.letter_spacing);
    text_style.set_height(style.line_height);
    text_style.set_height_override(true);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(paragraph_align(style.text_align));
    paragraph_style.set_text_style(&text_style);

    let mut builder = ParagraphBuilder::new(&paragraph_style, shared_font_collection());
    builder.push_style(&text_style);
    builder.add_text(&text);

    let mut paragraph = builder.build();
    paragraph.layout(normalize_width(max_width));
    paragraph
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
        let single_line_height = style.text_px * style.line_height;
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
