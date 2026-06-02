//! Engine-only default fonts (Noto Sans SC + Noto Color Emoji).
//!
//! Core stays font-free; the desktop engine embeds these so examples and CLI
//! work without an explicit `<fonts>` block.

use std::sync::Arc;

const NOTO_SANS_SC: &[u8] = include_bytes!("../../../assets/NotoSansSC-Regular.otf");
const NOTO_COLOR_EMOJI: &[u8] = include_bytes!("../../../assets/NotoColorEmoji.ttf");

/// Default `fontdb` for native rendering: CJK sans + color emoji fallback.
pub fn engine_default_font_db() -> Arc<fontdb::Database> {
    Arc::new(opencat_core::text::font_db_from_bytes(
        &[NOTO_SANS_SC.to_vec(), NOTO_COLOR_EMOJI.to_vec()],
        "Noto Sans SC",
    ))
}