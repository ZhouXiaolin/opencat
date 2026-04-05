use skia_safe::{
    Canvas, FontMgr, FontStyle, Typeface,
    font_style::{Slant, Weight, Width},
    textlayout::{
        FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextAlign as ParagraphAlign,
        TextStyle as ParagraphTextStyle, TypefaceFontProvider,
    },
};

use crate::style::{ComputedTextStyle, FontWeight, TextAlign};

static EMOJI_FONT_DATA: &[u8] = include_bytes!("../assets/NotoColorEmoji.ttf");
const UNBOUNDED_LAYOUT_WIDTH: f32 = 100_000.0;

fn get_emoji_typeface() -> Option<Typeface> {
    let font_mgr = FontMgr::new();
    font_mgr.new_from_data(EMOJI_FONT_DATA, 0)
}

fn make_paragraph(text: &str, style: &ComputedTextStyle, max_width: f32) -> Paragraph {
    let mut font_collection = FontCollection::new();
    font_collection.set_default_font_manager(FontMgr::new(), None);

    if let Some(typeface) = get_emoji_typeface() {
        let mut emoji_provider = TypefaceFontProvider::new();
        emoji_provider.register_typeface(typeface, Some("Noto Color Emoji"));
        font_collection.set_asset_font_manager(Some(emoji_provider.into()));
    }

    let mut text_style = ParagraphTextStyle::new();
    text_style.set_color(style.color.to_skia());
    text_style.set_font_size(style.text_px);
    text_style.set_font_style(font_style(style.font_weight));
    text_style.set_letter_spacing(style.letter_spacing);
    text_style.set_height(style.line_height);
    text_style.set_height_override(true);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(paragraph_align(style.text_align));
    paragraph_style.set_text_style(&text_style);

    let mut builder = ParagraphBuilder::new(&paragraph_style, font_collection);
    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(normalize_width(max_width));
    paragraph
}

fn font_style(weight: FontWeight) -> FontStyle {
    let weight = match weight {
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

pub fn measure_text(text: &str, style: &ComputedTextStyle) -> (f32, f32) {
    measure_text_in_width(text, style, f32::INFINITY)
}

pub fn measure_text_in_width(text: &str, style: &ComputedTextStyle, max_width: f32) -> (f32, f32) {
    let paragraph = make_paragraph(text, style, max_width);
    (
        paragraph.longest_line().max(1.0),
        paragraph.height().max(1.0),
    )
}

pub fn draw_text(
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

#[cfg(test)]
mod tests {
    use super::measure_text_in_width;
    use crate::style::ComputedTextStyle;

    #[test]
    fn textlayout_wraps_long_cjk_text_in_narrow_width() {
        let style = ComputedTextStyle::default();
        let single_line_height = style.text_px * style.line_height;
        let (_, measured_height) =
            measure_text_in_width("这是一个没有空格但应该自动换行的很长中文句子", &style, 80.0);

        assert!(
            measured_height > single_line_height,
            "expected narrow text layout to wrap into multiple lines, got height {measured_height}"
        );
    }
}
