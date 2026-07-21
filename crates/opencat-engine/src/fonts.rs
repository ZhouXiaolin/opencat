//! Engine-only default fonts (Noto Sans SC + Noto Color Emoji).
//!
//! Core stays font-free for defaults; the desktop engine embeds these so
//! examples and CLI work without an explicit `<fonts>` block. Document font
//! merge (precedence, family index, fallback) is owned by core prepare (#19);
//! the engine only supplies this base database plus raw document face bytes.

use std::sync::Arc;

const NOTO_SANS_SC: &[u8] = include_bytes!("../../../assets/NotoSansSC-Regular.otf");
const NOTO_COLOR_EMOJI: &[u8] = include_bytes!("../../../assets/NotoColorEmoji.ttf");
const NOTO_SANS_SC_FAMILY: &str = "Noto Sans SC";

/// Default `fontdb` for native rendering: CJK sans + color emoji fallback.
pub fn engine_default_font_db() -> Arc<fontdb::Database> {
    Arc::new(opencat_core::text::font_db_from_bytes(
        &[NOTO_SANS_SC.to_vec(), NOTO_COLOR_EMOJI.to_vec()],
        NOTO_SANS_SC_FAMILY,
    ))
}
