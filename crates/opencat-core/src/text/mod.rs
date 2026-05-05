//! 纯计算文本测量与字形光栅化。不依赖 Skia/平台字体管理，用 cosmic-text + fontdb。

use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
    ops::Range,
};

use cosmic_text::{
    Attrs, Buffer, Command, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
};
use rustc_hash::{FxHashMap, FxHasher};

use crate::style::{ComputedTextStyle, TextAlign, TextTransform};
use unicode_segmentation::UnicodeSegmentation;

const NOTO_SANS_SC: &[u8] = include_bytes!("../../../../assets/NotoSansSC-Regular.otf");
const NOTO_COLOR_EMOJI: &[u8] = include_bytes!("../../../../assets/NotoColorEmoji.ttf");

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMeasurement {
    pub width: f32,
    pub height: f32,
}

/// Backend-agnostic glyph data. Outline paths for regular text,
/// pre-rasterized RGBA bitmaps for color glyphs (e.g., emoji).
#[derive(Clone, Debug)]
pub enum GlyphData {
    /// Vector outline path commands
    Outline(Vec<Command>),
    /// Pre-rasterized color bitmap (e.g., emoji glyphs)
    ColorImage {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        placement_left: i32,
        placement_top: i32,
    },
}

/// A positioned glyph reference into the deduplicated glyph map.
#[derive(Clone, Debug)]
pub struct GlyphPosition {
    pub cache_key: u64,
    pub x: f32,
    pub y: f32,
    /// Byte range of the glyph within the original (transformed) text,
    /// used for text-unit override grouping.
    pub byte_range: Range<usize>,
}

/// Per-line layout metrics.
#[derive(Clone, Debug)]
pub struct TextLine {
    pub y: f32,
    pub width: f32,
    pub positions: Vec<GlyphPosition>,
}

/// Complete text rasterization result with deduplicated glyph data.
///
/// Same glyph occurring multiple times in the text is stored only once
/// in `glyphs`. `lines` references glyphs by `cache_key`.
#[derive(Clone, Debug)]
pub struct TextRasterization {
    /// Deduplicated glyph data keyed by cache_key
    pub glyphs: FxHashMap<u64, GlyphData>,
    /// Lines with positioned glyphs
    pub lines: Vec<TextLine>,
}

// ── Font database ──────────────────────────────────────────────────────────

pub fn default_font_db_with_embedded_only() -> fontdb::Database {
    let mut db = fontdb::Database::new();
    db.load_font_data(NOTO_SANS_SC.to_vec());
    db.load_font_data(NOTO_COLOR_EMOJI.to_vec());
    db
}

/// 创建带内嵌字体的 fontdb::Database。
/// `extra_font_dirs` 中每个目录会调用 `Database::load_fonts_dir`。
pub fn default_font_db(extra_font_dirs: &[&str]) -> fontdb::Database {
    let mut db = default_font_db_with_embedded_only();
    for dir in extra_font_dirs {
        db.load_fonts_dir(dir);
    }
    db
}

// ── Thread-local font system (shared by measure_text and rasterize_glyphs) ──

thread_local! {
    static FONT_SYSTEM: RefCell<Option<FontSystem>> = const { RefCell::new(None) };
    static SWASH_CACHE: RefCell<Option<SwashCache>> = const { RefCell::new(None) };
}

fn with_font_system<R>(f: impl FnOnce(&mut FontSystem) -> R) -> R {
    FONT_SYSTEM.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            let font_db = default_font_db_with_embedded_only();
            *opt = Some(FontSystem::new_with_locale_and_db(
                "en-US".to_string(),
                font_db,
            ));
        }
        f(opt.as_mut().unwrap())
    })
}

fn with_swash_cache<R>(f: impl FnOnce(&mut SwashCache) -> R) -> R {
    SWASH_CACHE.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(SwashCache::new());
        }
        f(opt.as_mut().unwrap())
    })
}

// ── Measurement ────────────────────────────────────────────────────────────

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

// ── Glyph rasterization ────────────────────────────────────────────────────

/// Rasterize text into backend-agnostic glyph data using the shared (embedded)
/// FontSystem. Returns deduplicated glyphs keyed by `cache_key` and positioned
/// per line.
pub fn rasterize_glyphs(
    text: &str,
    style: &ComputedTextStyle,
    max_width: f32,
    allow_wrap: bool,
    truncate: bool,
) -> TextRasterization {
    let rendered = apply_text_transform(text, style.text_transform);
    if truncate && (!max_width.is_finite() || max_width <= 0.0) {
        return TextRasterization {
            glyphs: FxHashMap::default(),
            lines: Vec::new(),
        };
    }

    let layout_width = if truncate || allow_wrap {
        if max_width.is_finite() && max_width > 0.0 {
            Some(max_width)
        } else {
            None
        }
    } else {
        None
    };

    let line_height = style.resolved_line_height_px();
    let metrics = Metrics::new(style.text_px, line_height);

    with_font_system(|font_system| {
        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_size(font_system, layout_width, None);

        let attrs = Attrs::new()
            .family(cosmic_text::Family::SansSerif)
            .weight(cosmic_text::Weight(style.font_weight.0));
        buffer.set_text(font_system, &rendered, attrs, Shaping::Advanced);

        let mut glyphs: FxHashMap<u64, GlyphData> = FxHashMap::default();
        let mut lines: Vec<TextLine> = Vec::new();

        with_swash_cache(|swash_cache| {
            for run in buffer.layout_runs() {
                let mut positions: Vec<GlyphPosition> = Vec::new();

                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let ck = glyph_cache_key(&physical.cache_key);

                    let x = physical.x as f32 + physical.cache_key.x_bin.as_float();
                    let y = run.line_y + physical.y as f32 + physical.cache_key.y_bin.as_float();

                    positions.push(GlyphPosition {
                        cache_key: ck,
                        x,
                        y,
                        byte_range: glyph.start..glyph.end,
                    });

                    if glyphs.contains_key(&ck) {
                        continue;
                    }

                    let swash_image = swash_cache.get_image(font_system, physical.cache_key);
                    if let Some(image) = swash_image
                        && image.content == SwashContent::Color
                    {
                        glyphs.insert(
                            ck,
                            GlyphData::ColorImage {
                                rgba: image.data.clone(),
                                width: image.placement.width as u32,
                                height: image.placement.height as u32,
                                placement_left: image.placement.left,
                                placement_top: image.placement.top,
                            },
                        );
                        continue;
                    }

                    if let Some(commands) =
                        swash_cache.get_outline_commands(font_system, physical.cache_key)
                    {
                        glyphs.insert(ck, GlyphData::Outline(commands.to_vec()));
                    }
                }

                lines.push(TextLine {
                    y: run.line_y,
                    width: run.line_w,
                    positions,
                });
            }
        });

        TextRasterization { glyphs, lines }
    })
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Compute a stable u64 key from a cosmic-text CacheKey.
pub fn glyph_cache_key(cache_key: &cosmic_text::CacheKey) -> u64 {
    let mut hasher = FxHasher::default();
    cache_key.hash(&mut hasher);
    hasher.finish()
}

pub fn apply_text_transform(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
    }
}

pub fn compute_line_x_shift(line_w: f32, container_w: f32, align: TextAlign) -> f32 {
    match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => (container_w - line_w) * 0.5,
        TextAlign::Right => container_w - line_w,
    }
}

/// Split text into grapheme-cluster byte ranges.
pub fn describe_grapheme_ranges(text: &str) -> Vec<Range<usize>> {
    UnicodeSegmentation::graphemes(text, true)
        .scan(0usize, |offset, grapheme| {
            let start = *offset;
            *offset += grapheme.len();
            Some(start..*offset)
        })
        .collect()
}

/// Split text into word byte ranges (graheme-based for CJK).
pub fn describe_text_unit_ranges(
    text: &str,
    granularity: crate::scene::script::TextUnitGranularity,
) -> Vec<Range<usize>> {
    match granularity {
        crate::scene::script::TextUnitGranularity::Grapheme => describe_grapheme_ranges(text),
        crate::scene::script::TextUnitGranularity::Word => {
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

pub fn contains_cjk(text: &str) -> bool {
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

pub fn ranges_overlap(a: Range<usize>, b: Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}

// ── Conversion to display-list glyph types ──────────────────────────────────

use crate::display::list::{
    DisplayGlyphCommand, DisplayGlyphData, DisplayGlyphEntry, DisplayGlyphLine,
    DisplayGlyphPosition, DisplayTextGlyphs,
};

/// Convert a `TextRasterization` into serializable display-list glyph data.
pub fn rasterization_to_display_glyphs(raster: &TextRasterization) -> DisplayTextGlyphs {
    let entries: Vec<DisplayGlyphEntry> = raster
        .glyphs
        .iter()
        .map(|(key, data)| DisplayGlyphEntry {
            cache_key: *key,
            data: match data {
                GlyphData::Outline(commands) => DisplayGlyphData::Outline {
                    commands: commands.iter().map(convert_command).collect(),
                },
                GlyphData::ColorImage {
                    rgba,
                    width,
                    height,
                    placement_left,
                    placement_top,
                } => DisplayGlyphData::ColorImage {
                    rgba: rgba.clone(),
                    width: *width,
                    height: *height,
                    placement_left: *placement_left,
                    placement_top: *placement_top,
                },
            },
        })
        .collect();

    let lines: Vec<DisplayGlyphLine> = raster
        .lines
        .iter()
        .map(|line| DisplayGlyphLine {
            y: line.y,
            width: line.width,
            positions: line
                .positions
                .iter()
                .map(|pos| DisplayGlyphPosition {
                    cache_key: pos.cache_key,
                    x: pos.x,
                    y: pos.y,
                    byte_start: pos.byte_range.start,
                    byte_end: pos.byte_range.end,
                })
                .collect(),
        })
        .collect();

    DisplayTextGlyphs { entries, lines }
}

fn convert_command(cmd: &cosmic_text::Command) -> DisplayGlyphCommand {
    match cmd {
        cosmic_text::Command::MoveTo(p) => DisplayGlyphCommand::MoveTo {
            x: p.x,
            y: p.y,
        },
        cosmic_text::Command::LineTo(p) => DisplayGlyphCommand::LineTo {
            x: p.x,
            y: p.y,
        },
        cosmic_text::Command::QuadTo(c, p) => DisplayGlyphCommand::QuadTo {
            cx: c.x,
            cy: c.y,
            x: p.x,
            y: p.y,
        },
        cosmic_text::Command::CurveTo(c1, c2, p) => DisplayGlyphCommand::CurveTo {
            c1x: c1.x,
            c1y: c1.y,
            c2x: c2.x,
            c2y: c2.y,
            x: p.x,
            y: p.y,
        },
        cosmic_text::Command::Close => DisplayGlyphCommand::Close,
    }
}

// ── FontProvider trait ─────────────────────────────────────────────────────

pub trait FontProvider {
    fn font_db(&self) -> &fontdb::Database;
}

pub struct DefaultFontProvider {
    db: std::sync::Arc<fontdb::Database>,
}

impl DefaultFontProvider {
    pub fn new() -> Self {
        Self {
            db: std::sync::Arc::new(default_font_db_with_embedded_only()),
        }
    }

    pub fn from_arc(db: std::sync::Arc<fontdb::Database>) -> Self {
        Self { db }
    }
}

impl Default for DefaultFontProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FontProvider for DefaultFontProvider {
    fn font_db(&self) -> &fontdb::Database {
        &self.db
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{
        DefaultFontProvider, FontProvider, apply_text_transform, default_font_db, measure_text,
        rasterize_glyphs,
    };
    use crate::style::{ComputedTextStyle, TextTransform};

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

    #[test]
    fn default_font_provider_exposes_loaded_db() {
        let p = DefaultFontProvider::new();
        let count = p.font_db().faces().count();
        assert!(
            count >= 2,
            "embedded NotoSansSC + NotoColorEmoji should be present, got {count}"
        );
    }

    #[test]
    fn textlayout_applies_uppercase_transform() {
        assert_eq!(
            apply_text_transform("Physics Education Series", TextTransform::Uppercase),
            "PHYSICS EDUCATION SERIES"
        );
    }

    #[test]
    fn rasterize_glyphs_produces_deduplicated_output() {
        let style = ComputedTextStyle::default();
        let result = rasterize_glyphs("Hello", &style, f32::INFINITY, false, false);

        // "Hello" has 5 glyphs, all unique
        assert_eq!(result.glyphs.len(), 5);
        let total_positions: usize = result.lines.iter().map(|l| l.positions.len()).sum();
        assert_eq!(total_positions, 5);
    }

    #[test]
    fn rasterize_glyphs_every_position_key_found_in_glyphs() {
        let style = ComputedTextStyle::default();
        let result = rasterize_glyphs("AAA", &style, f32::INFINITY, false, false);

        // CacheKey includes subpixel x_bin, so each "A" at a different
        // position may get a different key. Verify all positions resolve.
        let total_positions: usize = result.lines.iter().map(|l| l.positions.len()).sum();
        assert_eq!(total_positions, 3);
        for line in &result.lines {
            for pos in &line.positions {
                assert!(
                    result.glyphs.contains_key(&pos.cache_key),
                    "position cache_key not found in glyphs map"
                );
            }
        }
    }

    #[test]
    fn rasterize_glyphs_emoji_produces_color_image() {
        let style = ComputedTextStyle::default();
        let result = rasterize_glyphs("😀", &style, f32::INFINITY, false, false);

        assert!(!result.glyphs.is_empty());
        let has_color = result
            .glyphs
            .values()
            .any(|d| matches!(d, super::GlyphData::ColorImage { .. }));
        assert!(has_color, "emoji glyph should be rasterized as ColorImage");
    }
}
