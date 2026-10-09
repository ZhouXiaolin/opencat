//! Engine-only default fonts (Noto Sans SC + Noto Color Emoji).
//!
//! Core stays font-free for defaults; the desktop engine embeds these so
//! examples and CLI work without an explicit `<fonts>` block. Document font
//! merge (precedence, family index, fallback) is owned by core prepare (#19);
//! the engine only supplies this base database plus raw document face bytes.

use std::path::PathBuf;
use std::sync::Arc;

const NOTO_SANS_SC: &[u8] = include_bytes!("../../../assets/NotoSansSC-Regular.otf");
const NOTO_COLOR_EMOJI: &[u8] = include_bytes!("../../../assets/NotoColorEmoji.ttf");
const NOTO_SANS_SC_FAMILY: &str = "Noto Sans SC";

/// Default font bytes for native rendering: CJK sans + color emoji fallback.
pub fn engine_default_font_faces() -> Vec<Vec<u8>> {
    vec![NOTO_SANS_SC.to_vec(), NOTO_COLOR_EMOJI.to_vec()]
}

/// Standard fontconfig font directories, used as a last-resort glyph-fallback
/// pool (browser parity: Chrome resolves codepoints a declared face lacks
/// through fontconfig over these same directories — e.g. `▾`/`↵` in the
/// Spline Sans Mono latin subset fall back to DejaVu Sans). Loaded file-backed
/// after document/base faces via `HostInputs::with_font_fallback_dirs`;
/// non-existent entries are dropped.
pub fn engine_font_fallback_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".fonts"));
        dirs.push(home.join(".local/share/fonts"));
    }
    dirs.retain(|dir| dir.is_dir());
    dirs
}

/// Default `fontdb` for native rendering: CJK sans + color emoji fallback,
/// extended with [`engine_font_fallback_dirs`] as the glyph-fallback pool.
///
/// Internal helper — kept for callers that still pass a pre-built `fontdb`.
/// New code should use [`engine_default_font_faces`] with
/// `HostInputs::with_base_font_faces`.
pub fn engine_default_font_db() -> Arc<fontdb::Database> {
    let mut db = opencat_core::text::font_db_from_bytes(
        &engine_default_font_faces(),
        NOTO_SANS_SC_FAMILY,
    );
    for dir in engine_font_fallback_dirs() {
        db.load_fonts_dir(dir);
    }
    Arc::new(db)
}
