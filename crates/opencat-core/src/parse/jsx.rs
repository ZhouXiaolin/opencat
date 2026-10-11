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
use std::path::{Path, PathBuf};

use crate::parse::ParsedComposition;
use crate::parse::jsonl::JsonLine;
use crate::fonts::FontManifest;

/// Entry-module specifier used as the module-system root.
pub const ENTRY_SPEC: &str = "<entry>";

fn meta_f64(meta: &Value, key: &str, default: f64) -> f64 {
    meta.get(key).and_then(Value::as_f64).unwrap_or(default)
}

/// Build a [`ParsedComposition`] from an extraction result and the per-frame
/// driver source produced by the host.
///
/// The extracted tree already has the JSONL line shape, so it is decoded
/// straight into [`JsonLine`] values and handed to the shared assembly — no
/// text serialization hop in between.
pub fn from_extract(extract: &Value, driver: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let meta = extract.get("meta").cloned().unwrap_or(Value::Null);
    let font_manifest = font_manifest_from_extract(extract, &meta, base_dir)?;

    let tree_lines = extract
        .get("lines")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("jsx extract: missing `lines` array"))?;

    let mut lines: Vec<Value> = vec![json!({
        "type": "composition",
        "width": meta_f64(&meta, "width", 1920.0) as i32,
        "height": meta_f64(&meta, "height", 1080.0) as i32,
        "fps": meta_f64(&meta, "fps", 30.0) as i32,
        "duration": meta_f64(&meta, "duration", 3.0),
    })];
    lines.extend(tree_lines.iter().cloned());
    lines.push(json!({ "type": "script", "src": driver }));

    let jsonl_lines = lines
        .into_iter()
        .map(serde_json::from_value::<JsonLine>)
        .collect::<std::result::Result<Vec<_>, serde_json::Error>>()?;

    crate::parse::jsonl::composition_from_lines(jsonl_lines, base_dir, Some(&font_manifest))
}

/// `export const fonts = { default, faces: [{ id, family, path, role }] }`,
/// read from the extract's top-level `fonts` export (with the legacy
/// `meta.fonts` shape as fallback).
///
/// Mirrors `parse::markup`'s `<fonts>` block — same manifest, same
/// path-against-base_dir resolution, so the host font loader treats both
/// frontends identically.
fn font_manifest_from_extract(extract: &Value, meta: &Value, base_dir: Option<&Path>) -> Result<FontManifest> {
    use crate::fonts::{FontFaceDecl, FontManifest, FontRole, FontSource};

    let mut manifest = FontManifest::default();
    let fonts = match extract.get("fonts").or_else(|| meta.get("fonts")) {
        Some(fonts) => fonts,
        None => return Ok(manifest),
    };
    if let Some(default) = fonts.get("default").and_then(Value::as_str) {
        if default.is_empty() {
            anyhow::bail!("jsx fonts: `default` must be non-empty");
        }
        manifest.default_face_id = Some(default.to_string());
    }
    let faces = fonts
        .get("faces")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("jsx fonts: `faces` must be an array"))?;
    for face in faces {
        let id = face
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("jsx fonts: face is missing `id`"))?
            .to_string();
        if manifest.face_by_id(&id).is_some() {
            anyhow::bail!("jsx fonts: duplicate font id `{id}`");
        }
        let family = face
            .get("family")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let role = match face.get("role").and_then(Value::as_str) {
            None => None,
            Some("sans") => Some(FontRole::Sans),
            Some("mono") => Some(FontRole::Mono),
            Some("emoji") => Some(FontRole::Emoji),
            Some(other) => anyhow::bail!("jsx fonts: unknown role `{other}`"),
        };
        let source = match (face.get("path").and_then(Value::as_str), face.get("url").and_then(Value::as_str)) {
            (Some(path), None) => {
                let path = PathBuf::from(path);
                FontSource::Path(if path.is_absolute() {
                    path
                } else {
                    base_dir.unwrap_or_else(|| Path::new(".")).join(path)
                })
            }
            (None, Some(url)) => FontSource::Url(url.to_string()),
            (Some(_), Some(_)) => anyhow::bail!("jsx fonts `{id}`: only one of path / url"),
            (None, None) => anyhow::bail!("jsx fonts `{id}`: needs path or url"),
        };
        manifest.faces.push(FontFaceDecl {
            id,
            family,
            source,
            role,
        });
    }
    if let Some(default) = manifest.default_face_id.as_deref() {
        if manifest.face_by_id(default).is_none() {
            anyhow::bail!("jsx fonts: default `{default}` references an unknown face");
        }
    }
    Ok(manifest)
}
