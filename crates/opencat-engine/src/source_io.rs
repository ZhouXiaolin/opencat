use std::path::Path;

use anyhow::{Context, Result};

use opencat_core::parse::document::ParsedComposition;

/// JSX is the only text frontend. External script locators stay logical:
/// hosts fill HostInputs::insert_script_text (issue #20).
pub fn parse_file(path: impl AsRef<Path>) -> Result<ParsedComposition> {
    let path = path.as_ref();
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    crate::jsx::parse_with_base_dir(&input, Some(base_dir))
}

pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    crate::jsx::parse_with_base_dir(input, base_dir)
}


