//! JSX frontend: author in JS + JSX (with `import` and the `use*` capability
//! registry), render through the same pipeline as XML/JSONL.
//!
//! The host (engine) evaluates the entry module once in a throwaway realm and
//! hands the extracted tree here ([`from_extract`]). This module only reshapes
//! that tree into element lines and delegates to [`crate::parse::elements`], so
//! every downstream stage (prepare, pipeline, per-frame drivers) is untouched.
//!
//! The per-frame driver source is embedded as a `script` line: it
//! re-evaluates the module graph each frame (fresh globals — the same
//! `frame = f(t)` contract the XML `<script>` block has always had) and replays
//! the `use*` hooks.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::parse::ParsedComposition;
use crate::parse::elements::JsonLine;
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
    let font_manifest = font_manifest_from_extract(extract, &meta)?;

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
    lines.extend(audio_lines_from_extract(extract)?);
    if !driver.is_empty() {
        lines.push(json!({ "type": "script", "src": driver }));
    }

    let jsonl_lines = lines
        .into_iter()
        .map(serde_json::from_value::<JsonLine>)
        .collect::<std::result::Result<Vec<_>, serde_json::Error>>()?;

    crate::parse::elements::composition_from_lines(jsonl_lines, base_dir, Some(&font_manifest))
}

/// `export const audio = [{ id, path | url, duration? }]` — a global audio
/// POOL, declared once next to `meta` / `fonts` like a font manifest. It has
/// no `attach`: attachment is stated where the audio is USED, as an `audioId`
/// prop on the visual node (scene or `<tl>`) that plays it. Audio has no visual
/// box, so it never lives in the element tree itself.
///
/// Emitted lines mirror `JsonLine::Audio`, so the downstream attach
/// validation and audio track assembly stay untouched — this only decides
/// *what* each attachment points at.
pub fn audio_lines_from_extract(extract: &Value) -> Result<Vec<Value>> {
    let audio = match extract.get("audio") {
        Some(audio) => audio,
        None => return Ok(Vec::new()),
    };
    let items = audio
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("jsx audio: `audio` must be an array"))?;

    let mut pool = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let where_ = format!("jsx audio[{index}]");
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("{where_}: missing `id`"))?
            .to_string();

        let path = item.get("path").and_then(Value::as_str);
        let url = item.get("url").and_then(Value::as_str);
        let mut source = json!({});
        match (path, url) {
            (Some(path), None) => {
                if path.is_empty() {
                    anyhow::bail!("jsx audio `{id}`: path must be non-empty");
                }
                source["path"] = json!(path);
            }
            (None, Some(url)) => {
                if url.is_empty() {
                    anyhow::bail!("jsx audio `{id}`: url must be non-empty");
                }
                source["url"] = json!(url);
            }
            (Some(_), Some(_)) => anyhow::bail!("jsx audio `{id}`: only one of path / url"),
            (None, None) => anyhow::bail!("jsx audio `{id}`: needs path or url"),
        }

        let duration = item.get("duration").and_then(Value::as_f64);
        if let Some(d) = duration {
            if !d.is_finite() || d <= 0.0 {
                anyhow::bail!("jsx audio `{id}`: duration must be > 0");
            }
        }
        pool.push((id, source, duration));
    }

    // Attachment sites: the visual nodes that named an `audioId`. A timeline id
    // attaches the whole timeline; a scene id attaches that scene. Unknown ids
    // are surfaced by the shared assembly (same error as the XML/jsonl path).
    let sites = extract.get("audioSites").and_then(Value::as_array);
    let mut lines = Vec::new();
    for site in sites.into_iter().flatten() {
        let audio_id = site
            .get("audioId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let Some(audio_id) = audio_id else { continue };
        let node_id = site.get("nodeId").and_then(Value::as_str).unwrap_or_default();
        let Some((id, source, duration)) = pool.iter().find(|(id, _, _)| id == audio_id) else {
            anyhow::bail!(
                "audioId `{audio_id}` (on `{node_id}`) is not declared in `export const audio`"
            );
        };
        let mut line = json!(source);
        line["type"] = json!("audio");
        line["id"] = json!(id);
        line["attach"] = json!(node_id);
        if let Some(d) = duration {
            line["duration"] = json!(d);
        }
        lines.push(line);
    }
    Ok(lines)
}

/// `export const fonts = { default, faces: [{ id, family, path, role }] }`,
/// read from the extract's top-level `fonts` export (with the legacy
/// `meta.fonts` shape as fallback).
///
/// Paths resolve against `base_dir`, same manifest shape the font loader
/// consumes for every frontend.
pub fn font_manifest_from_extract(extract: &Value, meta: &Value) -> Result<FontManifest> {
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
                if path.is_empty() {
                    anyhow::bail!("jsx fonts `{id}`: path must be non-empty");
                }
                // Logical locator only — host joins document base at fetch time
                // (same contract as image/video/font AssetId stability).
                FontSource::Path(PathBuf::from(path))
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
