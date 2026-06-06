use opencat_core::parse::build_parsed_document;
use opencat_core::parse::document::{BuildOptions, CanvasChildrenMode, ParsedComposition};
use opencat_core::resource::fonts::{load_faces_with_fallbacks, merge_faces_into_db};

const NOTO_SANS_SC_FAMILY: &str = "Noto Sans SC";
const NOTO_COLOR_EMOJI_FAMILY: &str = "Noto Color Emoji";

/// Parse composition source for WASM rendering (applies preloaded `<fonts>` when present).
pub fn parse_source(
    input: &str,
    _base_font_db: &fontdb::Database,
) -> anyhow::Result<ParsedComposition> {
    let trimmed = input.trim();
    if trimmed.starts_with('{') {
        return opencat_core::parse::jsonl::parse_with_base_dir(input, None);
    }

    let parts = opencat_core::parse::markup::parse_parts_with_base_dir(input, None)?;
    if parts.font_manifest.is_empty() {
        return build_parsed_document(
            parts,
            BuildOptions {
                canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
            },
            None,
        );
    }

    let bytes = crate::resource::font_store::get_manifest_bytes(&parts.font_manifest);
    if bytes.len() != parts.font_manifest.faces.len() {
        anyhow::bail!(
            "document declares {} font(s) but only {} preloaded; call preload_assets first",
            parts.font_manifest.faces.len(),
            bytes.len()
        );
    }

    let index = parts.font_manifest.build_family_index();
    build_parsed_document(
        parts,
        BuildOptions {
            canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
        },
        Some(&index),
    )
}

/// Merge preloaded document fonts into the session fontdb (idempotent per source hash).
pub fn merge_preloaded_fonts(
    session_db: &std::sync::Arc<fontdb::Database>,
    input: &str,
    default_fonts: Option<(&[u8], &[u8])>,
) -> anyhow::Result<std::sync::Arc<fontdb::Database>> {
    let trimmed = input.trim();
    if trimmed.starts_with('{') {
        return Ok(session_db.clone());
    }
    let parts = opencat_core::parse::markup::parse_parts_with_base_dir(input, None)?;
    if parts.font_manifest.is_empty() {
        return Ok(session_db.clone());
    }
    let bytes = crate::resource::font_store::get_manifest_bytes(&parts.font_manifest);
    if bytes.is_empty() {
        return Ok(session_db.clone());
    }
    let (db, _) = if let Some((sans_sc, color_emoji)) = default_fonts {
        load_faces_with_fallbacks(
            &parts.font_manifest,
            &bytes,
            &[
                (NOTO_SANS_SC_FAMILY, sans_sc),
                (NOTO_COLOR_EMOJI_FAMILY, color_emoji),
            ],
        )?
    } else {
        merge_faces_into_db(fontdb::Database::new(), &parts.font_manifest, &bytes)?
    };
    Ok(std::sync::Arc::new(db))
}
