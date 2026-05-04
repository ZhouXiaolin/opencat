//! 验证 cosmic-text 的测量结果与 Skia textlayout 偏差在容忍范围内。
//! 偏差超过阈值会破坏布局兼容性，必须在迁移阶段卡死。

use opencat::style::{ComputedTextStyle, FontWeight};
use opencat::text::TextMeasureRequest;

const TOLERANCE: f32 = 0.5;
/// Emoji fallback 在不同引擎间差异较大，使用宽松阈值。
const EMOJI_TOLERANCE: f32 = 20.0;

fn skia_engine() -> opencat::text::SharedTextMeasurer {
    opencat::backend::skia::text::shared_text_engine()
}

fn cosmic_db() -> fontdb::Database {
    opencat::text::default_font_db(&[])
}

fn assert_close(name: &str, a: f32, b: f32, tolerance: f32) {
    let diff = (a - b).abs();
    assert!(
        diff <= tolerance,
        "{name}: skia={a}, cosmic={b}, diff={diff} > tolerance={tolerance}"
    );
}

#[test]
fn parity_short_english_normal() {
    let style = ComputedTextStyle {
        text_px: 16.0,
        font_weight: FontWeight::NORMAL,
        ..ComputedTextStyle::default()
    };
    let req = TextMeasureRequest {
        text: "Hello",
        style: &style,
        max_width: f32::INFINITY,
        allow_wrap: false,
    };
    let skia = skia_engine().measure(&req);
    let cosmic =
        opencat::text::measure_text("Hello", &style, f32::INFINITY, false, &cosmic_db());
    assert_close("short_english_normal.width", skia.width, cosmic.width, TOLERANCE);
    assert_close("short_english_normal.height", skia.height, cosmic.height, TOLERANCE);
}

#[test]
fn parity_short_chinese_bold() {
    let style = ComputedTextStyle {
        text_px: 24.0,
        font_weight: FontWeight::BOLD,
        ..ComputedTextStyle::default()
    };
    let req = TextMeasureRequest {
        text: "你好世界",
        style: &style,
        max_width: f32::INFINITY,
        allow_wrap: false,
    };
    let skia = skia_engine().measure(&req);
    let cosmic =
        opencat::text::measure_text("你好世界", &style, f32::INFINITY, false, &cosmic_db());
    assert_close("short_chinese_bold.width", skia.width, cosmic.width, TOLERANCE);
    assert_close("short_chinese_bold.height", skia.height, cosmic.height, TOLERANCE);
}

#[test]
fn parity_multiline_cjk_wrap() {
    let style = ComputedTextStyle::default();
    let text = "这是一个没有空格但应该自动换行的很长中文句子";
    let req = TextMeasureRequest {
        text,
        style: &style,
        max_width: 200.0,
        allow_wrap: true,
    };
    let skia = skia_engine().measure(&req);
    let cosmic = opencat::text::measure_text(text, &style, 200.0, true, &cosmic_db());
    assert_close("multiline_cjk_wrap.width", skia.width, cosmic.width, TOLERANCE);
    assert_close("multiline_cjk_wrap.height", skia.height, cosmic.height, TOLERANCE);
}

#[test]
fn parity_text_with_emoji() {
    let style = ComputedTextStyle::default();
    let req = TextMeasureRequest {
        text: "Hello 🐱",
        style: &style,
        max_width: f32::INFINITY,
        allow_wrap: false,
    };
    let skia = skia_engine().measure(&req);
    let cosmic =
        opencat::text::measure_text("Hello 🐱", &style, f32::INFINITY, false, &cosmic_db());
    // Emoji fallback 在不同引擎间差异较大，使用宽松阈值
    assert_close("text_with_emoji.width", skia.width, cosmic.width, EMOJI_TOLERANCE);
    assert_close("text_with_emoji.height", skia.height, cosmic.height, EMOJI_TOLERANCE);
}
