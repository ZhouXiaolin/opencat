//! 纯计算文本测量与字形光栅化。不依赖 Skia/平台字体管理，用 cosmic-text + fontdb。

use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
    ops::Range,
};

use cosmic_text::{
    Attrs, Buffer, Command, Fallback, FontSystem, Metrics, PlatformFallback, Shaping, SwashCache,
    SwashContent,
};
use hashbrown::HashMap;

use crate::style::{ComputedTextStyle, GenericFamily, TextAlign, TextTransform};
use unicode_segmentation::UnicodeSegmentation;

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
    /// Vector outline path commands in design units (unscaled).
    /// `upem` is the font's units-per-em for computing scale at draw time.
    Outline(Vec<Command>, f32),
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
///
/// `cache_key` includes font_size + subpixel bins (used for glyph data lookup).
/// `outline_key` uses only (font_id, glyph_id) for shared shape caching.
#[derive(Clone, Debug)]
pub struct GlyphPosition {
    pub cache_key: u64,
    pub outline_key: u64,
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
    pub glyphs: HashMap<u64, GlyphData>,
    /// Lines with positioned glyphs
    pub lines: Vec<TextLine>,
}

// ── Font database ──────────────────────────────────────────────────────────

/// Empty database — default for core and web until the host loads fonts.
pub fn empty_font_db() -> fontdb::Database {
    fontdb::Database::new()
}

/// Build a database from raw font bytes and map generic `sans-serif` to `sans_serif_family`.
pub fn font_db_from_bytes(faces: &[Vec<u8>], sans_serif_family: &str) -> fontdb::Database {
    let mut db = empty_font_db();
    for data in faces {
        db.load_font_data(data.clone());
    }
    db.set_sans_serif_family(sans_serif_family);
    db
}

/// Append font bytes to a copy of `db`.
pub fn extend_font_db(db: &fontdb::Database, extra_faces: &[Vec<u8>]) -> fontdb::Database {
    let mut next = db.clone();
    for data in extra_faces {
        next.load_font_data(data.clone());
    }
    next
}

/// Load extra directories into a copy of `db`.
pub fn extend_font_db_with_dirs(db: &fontdb::Database, dirs: &[&str]) -> fontdb::Database {
    let mut next = db.clone();
    for dir in dirs {
        next.load_fonts_dir(dir);
    }
    next
}

#[cfg(any(test, feature = "test-support"))]
pub fn test_default_font_db() -> fontdb::Database {
    font_db_from_bytes(
        &[
            include_bytes!("../../../../assets/NotoSansSC-Regular.otf").to_vec(),
            include_bytes!("../../../../assets/NotoColorEmoji.ttf").to_vec(),
        ],
        "Noto Sans SC",
    )
}

// ── Per-render font db scope (script canvas text measure) ───────────────────

thread_local! {
    static SCOPED_FONT_DB: RefCell<Option<fontdb::Database>> = const { RefCell::new(None) };
}

/// Run `f` while script bindings use this font database (e.g. `canvas.measureText`).
pub fn scope_font_db<R>(db: &fontdb::Database, f: impl FnOnce() -> R) -> R {
    SCOPED_FONT_DB.with(|cell| {
        let prev = cell.borrow_mut().replace(db.clone());
        let out = f();
        *cell.borrow_mut() = prev;
        out
    })
}

fn font_db_for_script() -> fontdb::Database {
    SCOPED_FONT_DB
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(empty_font_db)
}

// ── Fallback policy ────────────────────────────────────────────────────────

/// Codepoints with `Emoji_Presentation=Yes` (Unicode 15.1 `emoji-data.txt`, the
/// same property Chrome's ICU uses). These render as color emoji by default; a
/// text-presentation glyph requires an explicit `U+FE0F`… the inverse of what a
/// naive "emoji face first" fallback does to text-default symbols (`✳` U+2733,
/// `→` U+2192, `▏` U+258F are NOT in this set). Chromium source of truth:
/// unicode `Emoji` property data; extracted via `\p{Emoji_Presentation}`.
const EMOJI_PRESENTATION_RANGES: &[(u32, u32)] = &[
    (0x231A, 0x231B),
    (0x23E9, 0x23EC),
    (0x23F0, 0x23F0),
    (0x23F3, 0x23F3),
    (0x25FD, 0x25FE),
    (0x2614, 0x2615),
    (0x2648, 0x2653),
    (0x267F, 0x267F),
    (0x2693, 0x2693),
    (0x26A1, 0x26A1),
    (0x26AA, 0x26AB),
    (0x26BD, 0x26BE),
    (0x26C4, 0x26C5),
    (0x26CE, 0x26CE),
    (0x26D4, 0x26D4),
    (0x26EA, 0x26EA),
    (0x26F2, 0x26F3),
    (0x26F5, 0x26F5),
    (0x26FA, 0x26FA),
    (0x26FD, 0x26FD),
    (0x2705, 0x2705),
    (0x270A, 0x270B),
    (0x2728, 0x2728),
    (0x274C, 0x274C),
    (0x274E, 0x274E),
    (0x2753, 0x2755),
    (0x2757, 0x2757),
    (0x2795, 0x2797),
    (0x27B0, 0x27B0),
    (0x27BF, 0x27BF),
    (0x2B1B, 0x2B1C),
    (0x2B50, 0x2B50),
    (0x2B55, 0x2B55),
    (0x1F004, 0x1F004),
    (0x1F0CF, 0x1F0CF),
    (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A),
    (0x1F1E6, 0x1F1FF),
    (0x1F201, 0x1F201),
    (0x1F21A, 0x1F21A),
    (0x1F22F, 0x1F22F),
    (0x1F232, 0x1F236),
    (0x1F238, 0x1F23A),
    (0x1F250, 0x1F251),
    (0x1F300, 0x1F320),
    (0x1F32D, 0x1F335),
    (0x1F337, 0x1F37C),
    (0x1F37E, 0x1F393),
    (0x1F3A0, 0x1F3CA),
    (0x1F3CF, 0x1F3D3),
    (0x1F3E0, 0x1F3F0),
    (0x1F3F4, 0x1F3F4),
    (0x1F3F8, 0x1F43E),
    (0x1F440, 0x1F440),
    (0x1F442, 0x1F4FC),
    (0x1F4FF, 0x1F53D),
    (0x1F54B, 0x1F54E),
    (0x1F550, 0x1F567),
    (0x1F57A, 0x1F57A),
    (0x1F595, 0x1F596),
    (0x1F5A4, 0x1F5A4),
    (0x1F5FB, 0x1F64F),
    (0x1F680, 0x1F6C5),
    (0x1F6CC, 0x1F6CC),
    (0x1F6D0, 0x1F6D2),
    (0x1F6D5, 0x1F6D8),
    (0x1F6DC, 0x1F6DF),
    (0x1F6EB, 0x1F6EC),
    (0x1F6F4, 0x1F6FC),
    (0x1F7E0, 0x1F7EB),
    (0x1F7F0, 0x1F7F0),
    (0x1F90C, 0x1F93A),
    (0x1F93C, 0x1F945),
    (0x1F947, 0x1F9FF),
    (0x1FA70, 0x1FA7C),
    (0x1FA80, 0x1FA8A),
    (0x1FA8E, 0x1FAC6),
    (0x1FAC8, 0x1FAC8),
    (0x1FACD, 0x1FADC),
    (0x1FADF, 0x1FAEA),
    (0x1FAEF, 0x1FAF8),
];

/// VARIATION SELECTOR-16 — appended to a text-presentation codepoint to force
/// emoji presentation (`✳︎` → colored emoji), exactly as Chrome treats it.
const VARIATION_SELECTOR_16: char = '\u{FE0F}';

fn is_emoji_presentation(cp: char) -> bool {
    let c = cp as u32;
    EMOJI_PRESENTATION_RANGES
        .binary_search_by(|(lo, hi)| {
            if c < *lo {
                std::cmp::Ordering::Greater
            } else if c > *hi {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// Does this text contain clusters a browser renders as color emoji (default
/// emoji presentation, or an explicit VS16)? Granularity is per text, matching
/// the shaper's per-run fallback list; mixed presentation+text-emoji strings
/// are a known approximation (both get the emoji-first list).
fn text_prefers_emoji(text: &str) -> bool {
    text.chars()
        .any(|ch| is_emoji_presentation(ch) || ch == VARIATION_SELECTOR_16)
}

/// cosmic-text fallback lists ordered for browser parity.
///
/// Chrome resolves a missing glyph by walking the CSS font stack, then its
/// generic tail (`monospace`/`sans-serif`/`serif`), with fontconfig's
/// preferences: a `'Courier New', monospace` tail metric-aliases to Liberation
/// Mono (the effective monospace), per-charset sorts put Liberation Sans ahead
/// of DejaVu Sans for `→`, and glyphs the aliased faces lack (`▾`/`✳`/`↵`/`◆`)
/// fall through to DejaVu Sans, fontconfig's default-family answer.
/// cosmic-text's fallback iterator tests glyph coverage as it walks the name
/// list, so encoding that preference as name order reproduces the browser.
///
/// The color-emoji face is hoisted to the front ONLY for texts that contain
/// default-emoji-presentation clusters or an explicit VS16 — hoisting it
/// unconditionally would capture text-presentation symbols (`✳`) as color
/// bitmaps, which is not what a browser does.
struct ShapingFallback {
    generic: Option<GenericFamily>,
    emoji_first: bool,
}

/// Common-list order segments; relative order inside a segment follows how
/// Chromium resolves uncovered glyphs on Linux. Sans: the requested stack's
/// generic tail hits fontconfig's sans preference (Noto ahead of
/// Liberation/DejaVu). Mono: a `'X', 'Courier New', monospace` stack never
/// reaches a real Courier — fontconfig metric-aliases it to Liberation Mono,
/// which is the effective monospace; glyphs it lacks (▾ ◆ ↵ ✳) go through the
/// per-character fallback, which fontconfig answers with the default family
/// (DejaVu Sans — `fc-match sans-serif:charset=<cp>`) before the leftover
/// mono faces. The six concrete lists below are spelled out literally: no
/// allocation, and unit tests assert exact browser parity.
const EMOJI_FALLBACK: &str = "Noto Color Emoji";
const SANS_FIRST: &[&str] = &[
    "Noto Sans",
    "Liberation Sans",
    "DejaVu Sans",
    "FreeSans",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "FreeMono",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
];
const SANS_FIRST_EMOJI: &[&str] = &[
    EMOJI_FALLBACK,
    "Noto Sans",
    "Liberation Sans",
    "DejaVu Sans",
    "FreeSans",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "FreeMono",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
];
const MONO_FIRST: &[&str] = &[
    "Liberation Mono",
    "DejaVu Sans",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
    "FreeMono",
    "Liberation Sans",
    "Noto Sans",
    "FreeSans",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
];
const MONO_FIRST_EMOJI: &[&str] = &[
    EMOJI_FALLBACK,
    "Liberation Mono",
    "DejaVu Sans",
    "DejaVu Sans Mono",
    "Noto Sans Mono",
    "FreeMono",
    "Liberation Sans",
    "Noto Sans",
    "FreeSans",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
];
const SERIF_FIRST: &[&str] = &[
    "Noto Serif",
    "DejaVu Serif",
    "Liberation Serif",
    "Nimbus Roman",
    "FreeSerif",
    "Noto Sans",
    "Liberation Sans",
    "DejaVu Sans",
    "FreeSans",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "FreeMono",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
];
const SERIF_FIRST_EMOJI: &[&str] = &[
    EMOJI_FALLBACK,
    "Noto Serif",
    "DejaVu Serif",
    "Liberation Serif",
    "Nimbus Roman",
    "FreeSerif",
    "Noto Sans",
    "Liberation Sans",
    "DejaVu Sans",
    "FreeSans",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Noto Sans Mono",
    "FreeMono",
    "Noto Sans Symbols",
    "Noto Sans Symbols2",
];

impl Fallback for ShapingFallback {
    fn common_fallback(&self) -> &'static [&'static str] {
        match (self.generic, self.emoji_first) {
            (Some(GenericFamily::Monospace), true) => MONO_FIRST_EMOJI,
            (Some(GenericFamily::Monospace), false) => MONO_FIRST,
            (Some(GenericFamily::Serif), true) => SERIF_FIRST_EMOJI,
            (Some(GenericFamily::Serif), false) => SERIF_FIRST,
            (_, true) => SANS_FIRST_EMOJI,
            (_, false) => SANS_FIRST,
        }
    }

    fn forbidden_fallback(&self) -> &'static [&'static str] {
        PlatformFallback.forbidden_fallback()
    }

    fn script_fallback(
        &self,
        script: unicode_script::Script,
        locale: &str,
    ) -> &'static [&'static str] {
        PlatformFallback.script_fallback(script, locale)
    }
}

fn shape_font_system(
    db: &fontdb::Database,
    generic: Option<GenericFamily>,
    emoji_first: bool,
) -> FontSystem {
    FontSystem::new_with_locale_and_db_and_fallback(
        "en-US".to_string(),
        db.clone(),
        ShapingFallback { generic, emoji_first },
    )
}

// ── Thread-local swash cache (rasterize_glyphs) ─────────────────────────────

thread_local! {
    static SWASH_CACHE: RefCell<Option<SwashCache>> = const { RefCell::new(None) };
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
        shape_font_system(&font_db, style.font_generic, text_prefers_emoji(text));
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(layout_width, None);

    let attrs = text_attrs(style);
    let transformed = apply_text_transform(text, style.text_transform);
    buffer.set_text(&transformed, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, false);

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

/// Rasterize text into backend-agnostic glyph data.
/// Returns deduplicated glyphs keyed by `cache_key` and positioned per line.
pub fn rasterize_glyphs(
    text: &str,
    style: &ComputedTextStyle,
    max_width: f32,
    allow_wrap: bool,
    truncate: bool,
    font_db: &fontdb::Database,
) -> TextRasterization {
    let rendered = apply_text_transform(text, style.text_transform);
    if truncate && (!max_width.is_finite() || max_width <= 0.0) {
        return TextRasterization {
            glyphs: HashMap::default(),
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

    let mut font_system = shape_font_system(
        &font_db,
        style.font_generic,
        text_prefers_emoji(&rendered),
    );
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(layout_width, None);

    let attrs = text_attrs(style);
    buffer.set_text(&rendered, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, false);

    let mut glyphs: HashMap<u64, GlyphData> = HashMap::default();
    let mut lines: Vec<TextLine> = Vec::new();

    with_swash_cache(|swash_cache| {
        for run in buffer.layout_runs() {
            let mut positions: Vec<GlyphPosition> = Vec::new();

            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let ck = glyph_cache_key(&physical.cache_key);
                let ok = glyph_outline_key(&physical.cache_key);

                let x = physical.x as f32 + physical.cache_key.x_bin.as_float();
                let y = run.line_y + physical.y as f32 + physical.cache_key.y_bin.as_float();

                positions.push(GlyphPosition {
                    cache_key: ck,
                    outline_key: ok,
                    x,
                    y,
                    byte_range: glyph.start..glyph.end,
                });

                if glyphs.contains_key(&ck) {
                    continue;
                }

                let swash_image = swash_cache.get_image(&mut font_system, physical.cache_key);
                if let Some(image) = swash_image
                    && image.content == SwashContent::Color
                {
                    glyphs.insert(
                        ck,
                        GlyphData::ColorImage {
                            rgba: image.data.clone(),
                            width: image.placement.width,
                            height: image.placement.height,
                            placement_left: image.placement.left,
                            placement_top: image.placement.top,
                        },
                    );
                    continue;
                }

                if let Some(commands) =
                    swash_cache.get_outline_commands(&mut font_system, physical.cache_key)
                {
                    let upem = font_system
                        .get_font(physical.cache_key.font_id, physical.cache_key.font_weight)
                        .map(|f| f.as_swash().metrics(&[]).units_per_em as f32)
                        .unwrap_or(1000.0);
                    glyphs.insert(ck, GlyphData::Outline(commands.to_vec(), upem));
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
}

// ── Script offscreen surface (canvas drawText / getImageData capability) ───

pub mod surface;

// ── Script text measurement ────────────────────────────────────────────────

/// Measure single-line text width for the script engine canvas API.
/// Uses the font database from [`scope_font_db`] when inside a render pass.
/// Returns 0.0 for empty strings.
pub fn measure_script_text_width(text: &str, font_size: f32, scale_x: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let line_height = font_size * 1.2;
    let metrics = Metrics::new(font_size, line_height);
    let font_db = font_db_for_script();
    // Canvas API text carries no style object: default sans class, but still
    // keep emoji presentation parity for emoji-heavy canvas strings.
    let mut font_system = shape_font_system(&font_db, None, text_prefers_emoji(text));
    let mut buffer = Buffer::new(&mut font_system, metrics);
    buffer.set_size(None, None);
    let attrs = Attrs::new().family(cosmic_text::Family::SansSerif);
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut font_system, false);
    let mut width: f32 = 0.0;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
    }
    (width * scale_x).max(0.0)
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn text_attrs(style: &ComputedTextStyle) -> Attrs<'_> {
    let family = style
        .font_family
        .as_deref()
        .map(cosmic_text::Family::Name)
        .unwrap_or(cosmic_text::Family::SansSerif);
    let letter_spacing_em = if style.text_px > 0.0 {
        style.letter_spacing / style.text_px
    } else {
        0.0
    };
    Attrs::new()
        .family(family)
        .weight(cosmic_text::Weight(style.font_weight.0))
        .letter_spacing(letter_spacing_em)
}

/// Compute a stable u64 key from a cosmic-text CacheKey.
pub fn glyph_cache_key(cache_key: &cosmic_text::CacheKey) -> u64 {
    let mut hasher = ahash::AHasher::default();
    cache_key.hash(&mut hasher);
    hasher.finish()
}

/// Key for the outline path cache — only `(font_id, glyph_id)` matters for shape.
pub fn glyph_outline_key(cache_key: &cosmic_text::CacheKey) -> u64 {
    let mut hasher = ahash::AHasher::default();
    cache_key.font_id.hash(&mut hasher);
    cache_key.glyph_id.hash(&mut hasher);
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
    granularity: crate::script::TextUnitGranularity,
) -> Vec<Range<usize>> {
    match granularity {
        crate::script::TextUnitGranularity::Grapheme => describe_grapheme_ranges(text),
        crate::script::TextUnitGranularity::Word => {
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
                GlyphData::Outline(commands, _upem) => DisplayGlyphData::Outline {
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
                    outline_key: pos.outline_key,
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
        cosmic_text::Command::MoveTo(p) => DisplayGlyphCommand::MoveTo { x: p.x, y: p.y },
        cosmic_text::Command::LineTo(p) => DisplayGlyphCommand::LineTo { x: p.x, y: p.y },
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
            db: std::sync::Arc::new(empty_font_db()),
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
        DefaultFontProvider, FontProvider, ShapingFallback, apply_text_transform, measure_text,
        rasterize_glyphs, test_default_font_db, text_prefers_emoji,
    };
    use cosmic_text::Fallback as _;
    use crate::style::{ComputedTextStyle, FontWeight, GenericFamily, TextTransform};

    #[test]
    fn diag_galaxie_space_advance() {
        use crate::text::font_db_from_bytes;
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/pr-to-video-launch/assets/");
        let mut faces = vec![];
        for f in [
            "HankenGrotesk-400.ttf",
            "HankenGrotesk-500.ttf",
            "HankenGrotesk-600.ttf",
            "HankenGrotesk-700.ttf",
            "GalaxieCopernicus-Book.ttf",
            "SplineSansMono-400.ttf",
            "SplineSansMono-500.ttf",
            "ArchivoBlack.ttf",
        ] {
            faces.push(std::fs::read(format!("{dir}{f}")).unwrap());
        }
        let db = font_db_from_bytes(&faces, "Hanken Grotesk");
        // also try WITHOUT the Noto fallbacks to compare
        {
            let style = ComputedTextStyle {
                text_px: 100.0,
                font_family: Some("Galaxie Copernicus".to_string()),
                ..ComputedTextStyle::default()
            };
            let m = measure_text("A A", &style, f32::INFINITY, false, &db);
            eprintln!("WITHOUT_NOTO A A width={:.2}", m.width);
        }
        let emoji = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/NotoColorEmoji.ttf")).unwrap();
        let sanssc = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/NotoSansSC-Regular.otf")).unwrap();
        for (label, extra) in [("EMOJI_ONLY", vec![emoji.clone()]), ("SANSSC_ONLY", vec![sanssc.clone()]), ("BOTH", vec![emoji.clone(), sanssc.clone()])] {
            let mut d = db.clone();
            for b in extra { d.load_font_data(b); }
            let style = ComputedTextStyle {
                text_px: 100.0,
                font_family: Some("Galaxie Copernicus".to_string()),
                ..ComputedTextStyle::default()
            };
            let m = measure_text("A A", &style, f32::INFINITY, false, &d);
            eprintln!("{label} A A width={:.2}", m.width);
            // try weight 500 too
            let style5 = ComputedTextStyle { font_weight: crate::style::FontWeight(500), ..style.clone() };
            let m5 = measure_text("A A", &style5, f32::INFINITY, false, &d);
            eprintln!("{label} A A @w500 width={:.2}", m5.width);
            // which font shapes the space? inspect via cosmic directly
            {
                use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
                let mut fs = FontSystem::new_with_locale_and_db("en-US".to_string(), d.clone());
                let mut buf = Buffer::new(&mut fs, Metrics::new(100.0, 120.0));
                let a = Attrs::new().family(cosmic_text::Family::Name("Galaxie Copernicus"));
                buf.set_text("A A", &a, Shaping::Advanced, None);
                buf.shape_until_scroll(&mut fs, false);
                for run in buf.layout_runs() {
                    for g in run.glyphs {
                        let fid = g.font_id;
                        let fam = fs.db().face(fid).map(|f| f.families.first().map(|x| x.0.clone())).flatten();
                        eprintln!("   glyph id={} font={:?} w={}", g.glyph_id, fam, g.w);
                    }
                }
            }
        }
        for face in db.faces() {
            if let fontdb::Source::Binary(_) = &face.source {
                eprintln!("FACE {:?} families={:?} weight={:?}", face.id, face.families, face.weight);
            }
        }
        let style = ComputedTextStyle {
            text_px: 100.0,
            font_family: Some("Galaxie Copernicus".to_string()),
            ..ComputedTextStyle::default()
        };
        for t in ["A", "AA", "A A"] {
            let m = measure_text(t, &style, f32::INFINITY, false, &db);
            let r = rasterize_glyphs(t, &style, f32::INFINITY, false, false, &db);
            let xs: Vec<f32> = r.lines[0].positions.iter().map(|p| p.x).collect();
            eprintln!("TEXT {:?} width={:.2} glyph_x={:?}", t, m.width, xs);
        }
    }

    #[test]
    fn cosmic_text_measures_short_english_text() {
        let db = test_default_font_db();
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
        let p = DefaultFontProvider::from_arc(std::sync::Arc::new(test_default_font_db()));
        let count = p.font_db().faces().count();
        assert!(
            count >= 2,
            "NotoSansSC + NotoColorEmoji should be present, got {count}"
        );
    }

    #[test]
    fn test_font_db_maps_sans_serif_to_noto_sans_sc() {
        let db = test_default_font_db();

        assert_eq!(db.family_name(&fontdb::Family::SansSerif), "Noto Sans SC");
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
        let db = test_default_font_db();
        let result = rasterize_glyphs("Hello", &style, f32::INFINITY, false, false, &db);

        // "Hello" has 5 glyphs, all unique
        assert_eq!(result.glyphs.len(), 5);
        let total_positions: usize = result.lines.iter().map(|l| l.positions.len()).sum();
        assert_eq!(total_positions, 5);
    }

    #[test]
    fn rasterize_glyphs_every_position_key_found_in_glyphs() {
        let style = ComputedTextStyle::default();
        let db = test_default_font_db();
        let result = rasterize_glyphs("AAA", &style, f32::INFINITY, false, false, &db);

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
    fn rasterized_line_width_matches_measured_width_for_spaced_text() {
        let style = ComputedTextStyle {
            text_px: 20.0,
            ..ComputedTextStyle::default()
        };
        let db = test_default_font_db();
        let text = "Content goes here - stats, code, images...";
        let measured = measure_text(text, &style, f32::INFINITY, false, &db);
        let result = rasterize_glyphs(text, &style, f32::INFINITY, false, false, &db);

        assert_eq!(result.lines.len(), 1);
        assert!(
            (result.lines[0].width - measured.width).abs() < 0.5,
            "rasterized width {} should match measured width {}",
            result.lines[0].width,
            measured.width
        );
    }

    #[test]
    fn letter_spacing_is_interpreted_as_pixels_not_em() {
        let db = test_default_font_db();
        let text = "ABC";
        let base = ComputedTextStyle {
            text_px: 18.0,
            ..ComputedTextStyle::default()
        };
        let tracked = ComputedTextStyle {
            letter_spacing: 1.0,
            ..base.clone()
        };

        let base_width = measure_text(text, &base, f32::INFINITY, false, &db).width;
        let tracked_width = measure_text(text, &tracked, f32::INFINITY, false, &db).width;
        let delta = tracked_width - base_width;

        assert!(
            (2.0..=4.0).contains(&delta),
            "1px tracking over 3 glyphs should add about 3px, got {delta}"
        );
    }

    #[test]
    fn rasterize_glyphs_bold_amount_every_position_key_found_in_glyphs() {
        let style = ComputedTextStyle {
            text_px: 22.0,
            font_weight: FontWeight::BOLD,
            ..ComputedTextStyle::default()
        };
        let db = test_default_font_db();
        let result = rasterize_glyphs("¥12,846.53", &style, f32::INFINITY, false, false, &db);

        let total_positions: usize = result.lines.iter().map(|l| l.positions.len()).sum();
        assert_eq!(total_positions, 10);
        for line in &result.lines {
            for pos in &line.positions {
                assert!(
                    result.glyphs.contains_key(&pos.cache_key),
                    "position cache_key {:?} not found in glyphs map for bold amount",
                    pos.cache_key
                );
                assert!(
                    matches!(
                        result.glyphs.get(&pos.cache_key),
                        Some(super::GlyphData::Outline(_, _))
                    ),
                    "bold amount glyph should use outline data, got {:?}",
                    result.glyphs.get(&pos.cache_key)
                );
            }
        }
    }

    #[test]
    fn rasterize_glyphs_emoji_produces_color_image() {
        let style = ComputedTextStyle::default();
        let db = test_default_font_db();
        let result = rasterize_glyphs("😀", &style, f32::INFINITY, false, false, &db);

        assert!(!result.glyphs.is_empty());
        let has_color = result
            .glyphs
            .values()
            .any(|d| matches!(d, super::GlyphData::ColorImage { .. }));
        assert!(has_color, "emoji glyph should be rasterized as ColorImage");
    }

    // ── Fallback policy: emoji presentation + generic-family ordering ─────────

    #[test]
    fn emoji_presentation_detection_matches_unicode_emoji_data() {
        // Text-presentation symbols: NOT hoisted to the color emoji face.
        for t in ["✳", "→", "▾", "↵", "▏", "☀", "©", "ℹ"] {
            assert!(
                !super::text_prefers_emoji(t),
                "{t} must keep text presentation"
            );
        }
        // Default-emoji-presentation codepoints and explicit VS16: hoisted.
        for t in ["😀", "🚀", "☔", "✳\u{FE0F}", "©\u{FE0F}"] {
            assert!(super::text_prefers_emoji(t), "{t} must prefer emoji");
        }
    }

    #[test]
    fn common_fallback_prefers_generic_class_faces() {
        let mono = super::ShapingFallback {
            generic: Some(GenericFamily::Monospace),
            emoji_first: false,
        }
        .common_fallback();
        assert_eq!(
            mono.first().copied(),
            Some("Liberation Mono"),
            "a 'Courier New', monospace tail metric-aliases to Liberation Mono — the \
             effective monospace a browser shapes covered symbols (→ −) with"
        );
        assert_eq!(
            mono.get(1).copied(),
            Some("DejaVu Sans"),
            "glyphs the aliased mono lacks (▾ ◆ ↵ ✳) hit fontconfig's default family \
             before the leftover mono faces"
        );
        let dvm = mono
            .iter()
            .position(|f| *f == "DejaVu Sans Mono")
            .expect("DejaVu Sans Mono present");
        let ns = mono
            .iter()
            .position(|f| *f == "Noto Sans")
            .expect("sans segment present after mono segment");
        assert!(
            dvm < ns,
            "remaining mono faces still precede sans last-resort: {mono:?}"
        );

        let sans = super::ShapingFallback {
            generic: Some(GenericFamily::SansSerif),
            emoji_first: false,
        }
        .common_fallback();
        assert_eq!(sans.first().copied(), Some("Noto Sans"));
        let (lib, dejavu) = (
            sans.iter().position(|f| *f == "Liberation Sans").unwrap(),
            sans.iter().position(|f| *f == "DejaVu Sans").unwrap(),
        );
        assert!(
            lib < dejavu,
            "Chrome shapes sans-stack '→' with Liberation Sans before DejaVu Sans"
        );
        assert!(
            !sans.contains(&"Noto Color Emoji"),
            "text-presentation-only text must not hoist the color emoji face"
        );

        let serif = super::ShapingFallback {
            generic: Some(GenericFamily::Serif),
            emoji_first: false,
        }
        .common_fallback();
        assert_eq!(serif.first().copied(), Some("Noto Serif"));
    }

    #[test]
    fn emoji_text_hoists_color_face_but_generic_order_holds() {
        let sans_emoji = super::ShapingFallback {
            generic: Some(GenericFamily::SansSerif),
            emoji_first: true,
        }
        .common_fallback();
        assert_eq!(sans_emoji.first().copied(), Some("Noto Color Emoji"));

        let mono_emoji = super::ShapingFallback {
            generic: Some(GenericFamily::Monospace),
            emoji_first: true,
        }
        .common_fallback();
        assert_eq!(mono_emoji.first().copied(), Some("Noto Color Emoji"));
        assert_eq!(
            mono_emoji.get(1).copied(),
            Some("Liberation Mono"),
            "generic preference applies right behind the emoji face"
        );
    }

    #[test]
    fn missing_glyph_run_falls_back_by_generic_class() {
        // Behavior: a mono-classed run whose primary face lacks a symbol shapes
        // it with Liberation Mono (the 'Courier New' metric alias — 0.6em
        // advance), while the same run without the class keeps the sans-first
        // order (Liberation Sans — 1.0em advance; Chrome CDP parity for sans
        // '→'). Needs the system Liberation + DejaVu faces; skipped where they
        // are not installed (portable CI).
        let liberation_dir = "/usr/share/fonts/truetype/liberation";
        let dejavu_dir = "/usr/share/fonts/truetype/dejavu";
        for face in [
            format!("{liberation_dir}/LiberationMono-Regular.ttf"),
            format!("{liberation_dir}/LiberationSans-Regular.ttf"),
            format!("{dejavu_dir}/DejaVuSans.ttf"),
        ] {
            if !std::path::Path::new(&face).exists() {
                eprintln!("skipping: {face} not installed");
                return;
            }
        }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/pr-to-video-launch/assets/");
        let mut faces = vec![std::fs::read(format!("{dir}SplineSansMono-400.ttf")).unwrap()];
        faces.push(std::fs::read(format!("{liberation_dir}/LiberationMono-Regular.ttf")).unwrap());
        faces.push(std::fs::read(format!("{liberation_dir}/LiberationSans-Regular.ttf")).unwrap());
        faces.push(std::fs::read(format!("{dejavu_dir}/DejaVuSans.ttf")).unwrap());
        let db = super::font_db_from_bytes(&faces, "Spline Sans Mono");

        let mono_style = ComputedTextStyle {
            text_px: 19.2,
            font_family: Some("Spline Sans Mono".to_string()),
            font_generic: Some(GenericFamily::Monospace),
            ..ComputedTextStyle::default()
        };
        let plain_style = ComputedTextStyle {
            text_px: 19.2,
            font_family: Some("Spline Sans Mono".to_string()),
            ..ComputedTextStyle::default()
        };

        // '→' advance: Liberation Mono 0.600em (11.52px @19.2px) on the mono
        // path vs Liberation Sans 1.000em (19.2px) on the sans-first path.
        let mono_width = measure_text("→", &mono_style, f32::INFINITY, false, &db).width;
        let plain_width = measure_text("→", &plain_style, f32::INFINITY, false, &db).width;
        assert!(
            mono_width < plain_width,
            "mono-generic fallback should pick the narrower aliased mono advance \
             (mono_width={mono_width}, plain_width={plain_width})"
        );
    }


}
