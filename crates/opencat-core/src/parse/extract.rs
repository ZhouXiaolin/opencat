//! HTML/dist 文档的提取结果装配层。
//!
//! core 不是浏览器内核。这里只把 HTML 提取宿主产出的 `{meta, css, tree,
//! driver}` 装配成视频管线可消费的静态组合树；帧仍由
//! `frame = f(t)` 驱动，动画仍由 `animate = g(t)` 写入。作者侧 JSX/Remotion
//! 源码先由 Vite 等打包器编译成 HTML/JS dist，再按标准 HTML 文档导入。

use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::fonts::{FontFaceDecl, FontManifest, FontRole, FontSource};
use crate::parse::ParsedComposition;
use crate::parse::document::DeclaredScript;
use crate::parse::dom::{DomAudio, DomDocument, DomElement, composition_from_dom};

fn meta_f64(meta: &Value, key: &str, default: f64) -> f64 {
    meta.get(key).and_then(Value::as_f64).unwrap_or(default)
}

/// 把 HTML 提取结果装配成 [`ParsedComposition`]。
pub fn from_extract(
    extract: &Value,
    driver: &str,
    base_dir: Option<&Path>,
) -> Result<ParsedComposition> {
    let meta = extract.get("meta").cloned().unwrap_or(Value::Null);
    let font_manifest = font_manifest_from_extract(extract, &meta)?;
    let css_engine = match extract.get("css").and_then(Value::as_str) {
        Some(css_text) => Some(crate::css::CssEngine::from_css_text(
            css_text,
            crate::css::EvalContext {
                viewport: (
                    meta_f64(&meta, "width", 1920.0) as f32,
                    meta_f64(&meta, "height", 1080.0) as f32,
                ),
                query_container: None,
                rem_base: 16.0,
                em_base: 16.0,
            },
        )?),
        None => None,
    };

    let tree = extract
        .get("tree")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("html extract: missing `tree` array"))?;
    let mut doc = DomDocument {
        width: meta_f64(&meta, "width", 1920.0) as i32,
        height: meta_f64(&meta, "height", 1080.0) as i32,
        fps: meta_f64(&meta, "fps", 30.0) as i32,
        duration: meta_f64(&meta, "duration", 3.0),
        html_frontend: extract
            .get("html")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ..DomDocument::default()
    };
    doc.audio = tree_audio_from_extract(extract)?;

    let tree = serde_json::from_value::<Vec<DomElement>>(Value::Array(tree.clone()))
        .map_err(|e| anyhow::anyhow!("html extract: bad `tree`: {e}"))?;
    match tree.len() {
        0 => {}
        1 => doc.root = Some(tree.into_iter().next().expect("len == 1")),
        _ => anyhow::bail!("multiple root elements found"),
    }
    if !driver.is_empty() {
        doc.global_scripts
            .push(DeclaredScript::Inline(driver.to_string()));
    }

    composition_from_dom(doc, base_dir, Some(&font_manifest), css_engine.as_ref())
}

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
            source: crate::parse::dom::parse_audio_source(path, url)?,
            duration,
        });
    }
    Ok(out)
}

/// 读取提取结果里的文档字体清单。字体字节由宿主按这里的逻辑路径预取。
pub fn font_manifest_from_extract(extract: &Value, meta: &Value) -> Result<FontManifest> {
    let mut manifest = FontManifest::default();
    let fonts = match extract.get("fonts").or_else(|| meta.get("fonts")) {
        Some(fonts) => fonts,
        None => return Ok(manifest),
    };
    manifest.css_faces = fonts.get("css").and_then(Value::as_bool).unwrap_or(false);
    if let Some(default) = fonts.get("default").and_then(Value::as_str) {
        if default.is_empty() {
            anyhow::bail!("html fonts: `default` must be non-empty");
        }
        manifest.default_face_id = Some(default.to_string());
    }
    let faces = fonts
        .get("faces")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("html fonts: `faces` must be an array"))?;
    for face in faces {
        let id = face
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("html fonts: face is missing `id`"))?
            .to_string();
        if manifest.face_by_id(&id).is_some() {
            anyhow::bail!("html fonts: duplicate font id `{id}`");
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
            Some(other) => anyhow::bail!("html fonts: unknown role `{other}`"),
        };
        let source = match (
            face.get("path").and_then(Value::as_str),
            face.get("url").and_then(Value::as_str),
        ) {
            (Some(path), None) => {
                if path.is_empty() {
                    anyhow::bail!("html fonts `{id}`: path must be non-empty");
                }
                FontSource::Path(PathBuf::from(path))
            }
            (None, Some(url)) => FontSource::Url(url.to_string()),
            (Some(_), Some(_)) => anyhow::bail!("html fonts `{id}`: only one of path / url"),
            (None, None) => anyhow::bail!("html fonts `{id}`: needs path or url"),
        };
        manifest.faces.push(FontFaceDecl {
            id,
            family,
            source,
            role,
        });
    }
    if let Some(default) = manifest.default_face_id.as_deref()
        && manifest.face_by_id(default).is_none()
    {
        anyhow::bail!("html fonts: default `{default}` references an unknown face");
    }
    Ok(manifest)
}
