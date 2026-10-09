//! JSX frontend: author in JS + JSX (with `import` and the `use*` capability
//! registry), render through the same pipeline as XML/JSONL.
//!
//! The host (engine) evaluates the entry module once in a throwaway realm and
//! hands the extracted tree here ([`from_extract`]). This module only reshapes
//! that tree into JSONL lines and delegates to [`crate::parse::jsonl`], so every
//! downstream stage (prepare, pipeline, per-frame drivers) is untouched.
//!
//! The per-frame driver source is embedded as a JSONL `script` line: it
//! re-evaluates the module graph each frame (fresh globals — the same
//! `frame = f(t)` contract the XML `<script>` block has always had) and replays
//! the `use*` hooks.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

use crate::parse::ParsedComposition;

/// Entry-module specifier used as the module-system root.
pub const ENTRY_SPEC: &str = "<entry>";

fn meta_f64(meta: &Value, key: &str, default: f64) -> f64 {
    meta.get(key).and_then(Value::as_f64).unwrap_or(default)
}

/// Build a [`ParsedComposition`] from an extraction result and the per-frame
/// driver source produced by the host.
pub fn from_extract(extract: &Value, driver: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let meta = extract.get("meta").cloned().unwrap_or(Value::Null);
    let mut lines: Vec<Value> = vec![json!({
        "type": "composition",
        "width": meta_f64(&meta, "width", 1920.0) as i32,
        "height": meta_f64(&meta, "height", 1080.0) as i32,
        "fps": meta_f64(&meta, "fps", 30.0) as i32,
        "duration": meta_f64(&meta, "duration", 3.0),
    })];

    let tree_lines = extract
        .get("lines")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("jsx extract: missing `lines` array"))?;
    lines.extend(tree_lines.iter().cloned());

    lines.push(json!({ "type": "script", "src": driver }));

    let text = lines
        .iter()
        .map(|line| serde_json::to_string(line).map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?
        .join("\n");

    crate::parse::jsonl::parse_with_base_dir(&text, base_dir)
}
