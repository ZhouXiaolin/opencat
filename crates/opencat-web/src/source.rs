use opencat_core::parse::build_parsed_document;
use opencat_core::parse::document::{BuildOptions, CanvasChildrenMode, ParsedComposition};

/// Parse composition source for WASM rendering.
///
/// Document font family refs (`font-sans` / `font-[id]`) are applied by core from
/// the parsed manifest. Document font *bytes* are not merged here — the host
/// supplies them via [`opencat_core::lifecycle::HostInputs::insert_document_font`]
/// and core prepare merges them over the base font database (#19).
pub fn parse_source(
    input: &str,
    _base_font_db: &fontdb::Database,
) -> anyhow::Result<ParsedComposition> {
    let trimmed = input.trim();
    if trimmed.starts_with('{') {
        return opencat_core::parse::jsonl::parse_with_base_dir(input, None);
    }

    let parts = opencat_core::parse::markup::parse_parts_with_base_dir(input, None)?;
    build_parsed_document(
        parts,
        BuildOptions {
            canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
        },
        None,
    )
}
