//! 纯计算文本测量。不依赖 Skia/平台字体管理，用 cosmic-text + fontdb。

use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};

use crate::style::ComputedTextStyle;

const NOTO_SANS_SC: &[u8] = include_bytes!("../assets/NotoSansSC-Regular.otf");
const NOTO_COLOR_EMOJI: &[u8] = include_bytes!("../assets/NotoColorEmoji.ttf");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMeasurement {
    pub width: f32,
    pub height: f32,
}

/// 创建带内嵌字体的 fontdb::Database。
/// `extra_font_dirs` 中每个目录会调用 `Database::load_fonts_dir`。
pub fn default_font_db(extra_font_dirs: &[&str]) -> fontdb::Database {
    let mut db = fontdb::Database::new();
    db.load_font_data(NOTO_SANS_SC.to_vec());
    db.load_font_data(NOTO_COLOR_EMOJI.to_vec());
    for dir in extra_font_dirs {
        db.load_fonts_dir(dir);
    }
    db
}

/// 用 cosmic-text 测量文本占用的盒子尺寸。
pub fn measure_text(
    text: &str,
    style: &ComputedTextStyle,
    max_width: f32,
    allow_wrap: bool,
    font_db: &fontdb::Database,
) -> TextMeasurement {
    let layout_width = if allow_wrap && max_width.is_finite() && max_width > 0.0 {
        Some(max_width)
    } else {
        None
    };
    let line_height = style.resolved_line_height_px();
    let metrics = Metrics::new(style.text_px, line_height);

    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".to_string(), font_db.clone());
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(&mut font_system, layout_width, None);

    let attrs = Attrs::new()
        .family(cosmic_text::Family::SansSerif)
        .weight(cosmic_text::Weight(style.font_weight.0));
    let transformed = apply_text_transform(text, style.text_transform);
    buffer.set_text(&mut font_system, &transformed, attrs, Shaping::Advanced);

    let mut measured_width: f32 = 0.0;
    let mut measured_height: f32 = 0.0;
    for run in buffer.layout_runs() {
        measured_width = measured_width.max(run.line_w);
        measured_height = measured_height.max(run.line_top + run.line_height);
    }
    if measured_height < line_height {
        measured_height = line_height;
    }

    TextMeasurement {
        width: measured_width.max(1.0),
        height: measured_height.max(1.0),
    }
}

fn apply_text_transform(text: &str, transform: crate::style::TextTransform) -> String {
    match transform {
        crate::style::TextTransform::None => text.to_string(),
        crate::style::TextTransform::Uppercase => text.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::{default_font_db, measure_text};
    use crate::style::ComputedTextStyle;

    #[test]
    fn cosmic_text_measures_short_english_text() {
        let db = default_font_db(&[]);
        let style = ComputedTextStyle::default();
        let measured = measure_text("Hello", &style, f32::INFINITY, false, &db);
        assert!(
            measured.width > 10.0 && measured.width < 80.0,
            "short English text should measure to a small finite width, got {}",
            measured.width
        );
        assert!(
            measured.height >= style.resolved_line_height_px() - 0.5,
            "text height should be at least one line, got {}",
            measured.height
        );
    }
}

// 兼容旧调用点的 type alias，迁移完成后删除
pub type SharedTextMeasurer = Arc<dyn TextMeasurer>;

pub struct TextMeasureRequest<'a> {
    pub text: &'a str,
    pub style: &'a ComputedTextStyle,
    pub max_width: f32,
    pub allow_wrap: bool,
}

pub trait TextMeasurer: Send + Sync {
    fn measure(&self, request: &TextMeasureRequest<'_>) -> TextMeasurement;
}
