//! Engine-only default fonts (Noto Sans SC + Noto Color Emoji).
//!
//! Core stays font-free; the desktop engine embeds these so examples and CLI
//! work without an explicit `<fonts>` block.

use std::sync::Arc;

use opencat_core::resource::fonts::{FontFamilyIndex, FontManifest, load_faces_with_fallbacks};

const NOTO_SANS_SC: &[u8] = include_bytes!("../../../assets/NotoSansSC-Regular.otf");
const NOTO_COLOR_EMOJI: &[u8] = include_bytes!("../../../assets/NotoColorEmoji.ttf");
const NOTO_SANS_SC_FAMILY: &str = "Noto Sans SC";
const NOTO_COLOR_EMOJI_FAMILY: &str = "Noto Color Emoji";

/// Default `fontdb` for native rendering: CJK sans + color emoji fallback.
pub fn engine_default_font_db() -> Arc<fontdb::Database> {
    Arc::new(opencat_core::text::font_db_from_bytes(
        &[NOTO_SANS_SC.to_vec(), NOTO_COLOR_EMOJI.to_vec()],
        NOTO_SANS_SC_FAMILY,
    ))
}

/// Build a document font database where `<fonts>` wins over embedded fallbacks.
pub fn engine_font_db_with_document_fonts(
    manifest: &FontManifest,
    bytes_by_id: &std::collections::HashMap<String, Vec<u8>>,
) -> anyhow::Result<(fontdb::Database, FontFamilyIndex)> {
    load_faces_with_fallbacks(
        manifest,
        bytes_by_id,
        &[
            (NOTO_SANS_SC_FAMILY, NOTO_SANS_SC),
            (NOTO_COLOR_EMOJI_FAMILY, NOTO_COLOR_EMOJI),
        ],
    )
}
