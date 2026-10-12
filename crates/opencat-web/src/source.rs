use opencat_core::parse::document::ParsedComposition;

/// Parse composition source for WASM rendering.
///
/// JSX extraction needs a JS realm, which the wasm host does not run — hosts
/// extract the module graph engine-side and hand over the envelope
/// `{"extract": {meta, fonts, lines}, "driver": "..."}` produced by
/// `web_envelope_for_oracle`. `from_extract` is pure Rust: it reshapes the
/// lines and embeds the per-frame driver as the composition script.
/// Document font family refs (`font-sans` / `font-[id]`) are applied by core
/// from the extracted manifest. Document font *bytes* are not merged here —
/// the host supplies them via
/// [`opencat_core::lifecycle::HostInputs::insert_document_font`] and core
/// prepare merges them over the base font database (#19).
pub fn parse_source(input: &str, _base_font_db: &fontdb::Database) -> anyhow::Result<ParsedComposition> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('{') {
        anyhow::bail!(
            "web parse_source: JSX sources are extracted host-side; pass the {{extract, driver}} envelope"
        )
    }
    let envelope: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| anyhow::anyhow!("web parse_source: invalid extract envelope: {e}"))?;
    let extract = envelope
        .get("extract")
        .ok_or_else(|| anyhow::anyhow!("web parse_source: envelope missing `extract`"))?;
    let driver = envelope
        .get("driver")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    opencat_core::parse::jsx::from_extract(extract, driver, None)
}
