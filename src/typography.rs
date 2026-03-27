use skia_safe::{Canvas, Font, FontMgr, FontStyle, Paint};

use crate::style::ComputedTextStyle;

fn make_font(style: &ComputedTextStyle) -> Font {
    let font_mgr = FontMgr::new();
    if let Some(typeface) = font_mgr.legacy_make_typeface(None, FontStyle::normal()) {
        Font::new(typeface, style.text_px)
    } else {
        let mut font = Font::default();
        font.set_size(style.text_px);
        font
    }
}

pub fn measure_text(text: &str, style: &ComputedTextStyle) -> (f32, f32) {
    let font = make_font(style);
    let (width, bounds) = font.measure_str(text, None);
    (width.max(1.0), bounds.height().max(1.0))
}

pub fn draw_text(canvas: &Canvas, text: &str, left: f32, top: f32, style: &ComputedTextStyle) {
    let mut paint = Paint::default();
    paint.set_color(style.color.to_skia());
    paint.set_anti_alias(true);

    let font = make_font(style);
    let (_, bounds) = font.measure_str(text, None);
    let baseline = top - bounds.top;
    canvas.draw_str(text, (left, baseline), &font, &paint);
}
