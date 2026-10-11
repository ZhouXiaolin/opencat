use opencat_core::parse::document::ParsedComposition;

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

    // JSX sources need a JS realm for module extraction, which the wasm host
    // does not run — the host side extracts and hands over the composition.
    anyhow::bail!(
        "web parse_source: only JSONL text sources parse in-wasm; JSX sources are extracted host-side"
    )
}
