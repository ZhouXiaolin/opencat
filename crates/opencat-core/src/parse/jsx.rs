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
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::parse::ParsedComposition;
use crate::parse::document::DeclaredScript;
use crate::parse::dom::{DomAudio, DomDocument, DomElement, composition_from_dom, parse_audio_source};
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
/// `tree` 是主通道：JSX 运行时在提取期物化的 DOM 树，直接进共享 DOM 装配。
/// `lines` 是遗留文本通道（JSONL 行形，[`JsonLine`] 文本序列化格式）：测试
/// 夹具与 wasm 桥从这里注入文本文档，经 `composition_from_lines` 分组成树。
pub fn from_extract(extract: &Value, driver: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let meta = extract.get("meta").cloned().unwrap_or(Value::Null);
    let font_manifest = font_manifest_from_extract(extract, &meta)?;

    // `export const css = \`...\``（tailwindcss CLI 编译产物或手写样式表）：
    // class 解析一律走标准 CSS 引擎；没有 css 时用空样式表，等价于无样式。
    let css_engine = match extract.get("css").and_then(Value::as_str) {
        Some(css_text) => Some(crate::css::CssEngine::from_css_text(
            css_text,
            crate::css::EvalContext {
                viewport: (
                    meta_f64(&meta, "width", 1920.0) as f32,
                    meta_f64(&meta, "height", 1080.0) as f32,
                ),
                rem_base: 16.0,
            },
        )?),
        None => None,
    };

    let Some(tree) = extract.get("tree") else {
        return from_extract_lines(extract, driver, base_dir, &font_manifest, css_engine.as_ref());
    };

    let mut doc = DomDocument {
        width: meta_f64(&meta, "width", 1920.0) as i32,
        height: meta_f64(&meta, "height", 1080.0) as i32,
        fps: meta_f64(&meta, "fps", 30.0) as i32,
        duration: meta_f64(&meta, "duration", 3.0),
        ..DomDocument::default()
    };
    doc.audio = audio_from_extract(extract)?;
    doc.audio.extend(tree_audio_from_extract(extract)?);

    let tree = serde_json::from_value::<Vec<DomElement>>(tree.clone())
        .map_err(|e| anyhow::anyhow!("jsx extract: bad `tree`: {e}"))?;
    match tree.len() {
        0 => {}
        1 => doc.root = tree.into_iter().next(),
        _ => anyhow::bail!("multiple root elements found"),
    }
    // 每帧驱动走全局脚本通道：装配期并入根节点的 script_driver。
    if !driver.is_empty() {
        doc.global_scripts
            .push(DeclaredScript::Inline(driver.to_string()));
    }

    composition_from_dom(doc, base_dir, Some(&font_manifest), css_engine.as_ref())
}

/// 遗留文本通道：`lines`（JSONL 行形）经共享桥装配。
fn from_extract_lines(
    extract: &Value,
    driver: &str,
    base_dir: Option<&Path>,
    font_manifest: &FontManifest,
    css: Option<&crate::css::CssEngine>,
) -> Result<ParsedComposition> {
    let tree_lines = extract
        .get("lines")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("jsx extract: missing `tree` array"))?;
    // meta 在行形通道里以合成 composition 行进入（夹具把首行拆成了 meta）。
    let meta = extract.get("meta").cloned().unwrap_or(Value::Null);
    let mut lines = vec![JsonLine::Composition {
        width: meta_f64(&meta, "width", 1920.0) as i32,
        height: meta_f64(&meta, "height", 1080.0) as i32,
        fps: meta_f64(&meta, "fps", 30.0) as i32,
        duration: meta_f64(&meta, "duration", 3.0),
    }];
    lines.extend(
        tree_lines
            .iter()
            .map(|line| serde_json::from_value::<JsonLine>(line.clone()))
            .collect::<std::result::Result<Vec<_>, serde_json::Error>>()?,
    );
    if !driver.is_empty() {
        lines.push(JsonLine::Script {
            parent_id: None,
            src: Some(driver.to_string()),
            path: None,
        });
    }
    let audio = audio_from_extract(extract)?;
    crate::parse::elements::composition_from_lines(
        lines,
        audio,
        base_dir,
        Some(font_manifest),
        css,
    )
}

/// `export const audio = [{ id, path | url, duration? }]` — a global audio
/// POOL, declared once next to `meta` / `fonts` like a font manifest. It has
/// no `attach`: attachment is stated where the audio is USED, as an `audioId`
/// prop on the visual node (scene or `<tl>`) that plays it. Audio has no visual
/// box, so it never lives in the element tree itself.
///
/// Each used site becomes one [`DomAudio`]; the shared assembly validates the
/// attach target (tl id → timeline, scene id → scene).
pub fn audio_from_extract(extract: &Value) -> Result<Vec<DomAudio>> {
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
        let source = match (path, url) {
            (Some(path), None) => {
                if path.is_empty() {
                    anyhow::bail!("jsx audio `{id}`: path must be non-empty");
                }
                (Some(path.to_string()), None)
            }
            (None, Some(url)) => {
                if url.is_empty() {
                    anyhow::bail!("jsx audio `{id}`: url must be non-empty");
                }
                (None, Some(url.to_string()))
            }
            (Some(_), Some(_)) => anyhow::bail!("jsx audio `{id}`: only one of path / url"),
            (None, None) => anyhow::bail!("jsx audio `{id}`: needs path or url"),
        };

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
    // are surfaced by the shared assembly (same error as the text path).
    let sites = extract.get("audioSites").and_then(Value::as_array);
    let mut out = Vec::new();
    for site in sites.into_iter().flatten() {
        let audio_id = site
            .get("audioId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let Some(audio_id) = audio_id else { continue };
        let node_id = site.get("nodeId").and_then(Value::as_str).unwrap_or_default();
        let Some((id, (path, url), duration)) = pool.iter().find(|(id, _, _)| id == audio_id) else {
            anyhow::bail!(
                "audioId `{audio_id}` (on `{node_id}`) is not declared in `export const audio`"
            );
        };
        out.push(DomAudio {
            id: id.clone(),
            attach: node_id.to_string(),
            source: parse_audio_source(path.clone(), url.clone())?,
            duration: *duration,
        });
    }
    Ok(out)
}

/// 树内 `<audio>`（emitElement 抽出进 `treeAudio`）：自带 attach（缺省指向
/// 挂载父元素，JS 侧已填），与全局池共用同一 DomAudio 形态。
fn tree_audio_from_extract(extract: &Value) -> Result<Vec<DomAudio>> {
    let items = extract.get("treeAudio").and_then(Value::as_array);
    let mut out = Vec::new();
    for item in items.into_iter().flatten() {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let attach = item
            .get("attach")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let path = item.get("path").and_then(Value::as_str).map(str::to_string);
        let url = item.get("url").and_then(Value::as_str).map(str::to_string);
        let duration = item.get("duration").and_then(Value::as_f64);
        out.push(DomAudio {
            id,
            attach,
            source: parse_audio_source(path, url)?,
            duration,
        });
    }
    Ok(out)
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
