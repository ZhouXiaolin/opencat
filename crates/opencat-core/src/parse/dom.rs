//! DOM 层 —— 统一文档对象模型。
//!
//! 作者心智 ≈ HTML：`div`/`text`/`canvas`/`image`/`video`/`lottie`/`path`/`svg`
//! 是普通元素，`tl`（timeline）与 `transition` 是两个有明确语义的特例节点。
//! 两类前端都把树直接写进这一层：
//!
//! - JSX：`h()` 描述子在提取期物化成树（extract payload 的 `tree` 字段）
//! - XML/JSONL 文本：逐行解析 → 按 `parentId` 分组成树（[`JsonLine`] 因此只是
//!   文本序列化格式，不再是装配模型）
//!
//! 本层只做一件事：`DomDocument` → `Node` 树（[`composition_from_dom`]）。
//! 旧的 `ParsedElement` 扁平表 + `build_tree` 装配脚手架已退役。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::media::{VideoColorMatrix, VideoFrameTiming};
use crate::parse::composition::{AudioAttachment, CompositionAudioSource};
use crate::parse::document::{DeclaredScript, ParsedComposition, script_driver_from_decls};
use crate::parse::easing::{Easing, SpringConfig, easing_from_name};
use crate::parse::node::Node;
use crate::parse::primitives::{
    AudioSource, ImageSource, OpenverseQuery, VideoSource, canvas, div, image, path, text, video,
    video_url,
};
use crate::parse::transition::{
    Transition, clock_wipe, cut, fade, gl_transition, iris, light_leak, slide, timeline, wipe,
};
use crate::style::NodeStyle;

/// 树形文档元素。字段拼写与文本序列化（JSONL）一致；`children` 把
/// parentId 扁平表换成了直接的树。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomElement {
    #[serde(rename = "div")]
    Div {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        duration: Option<f64>,
        #[serde(default)]
        children: Vec<DomElement>,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        text: String,
        duration: Option<f64>,
    },
    #[serde(rename = "canvas")]
    Canvas {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        duration: Option<f64>,
        #[serde(default)]
        children: Vec<DomElement>,
    },
    #[serde(rename = "image")]
    Image {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        query: Option<String>,
        #[serde(rename = "queryCount")]
        query_count: Option<usize>,
        #[serde(rename = "aspectRatio")]
        aspect_ratio: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "video")]
    Video {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
        #[serde(rename = "data-start", alias = "dataStart")]
        data_start: Option<f64>,
        #[serde(rename = "data-duration", alias = "dataDuration")]
        data_duration: Option<f64>,
        #[serde(rename = "data-media-start", alias = "dataMediaStart")]
        data_media_start: Option<f64>,
        #[serde(rename = "data-color-matrix", alias = "dataColorMatrix")]
        data_color_matrix: Option<String>,
        #[serde(rename = "data-hide-after-end", alias = "dataHideAfterEnd")]
        hide_after_end: Option<bool>,
        #[serde(rename = "loop")]
        looping: Option<bool>,
        #[serde(default)]
        children: Vec<DomElement>,
    },
    #[serde(rename = "lottie")]
    Lottie {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "path")]
    Path {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        d: String,
        #[serde(rename = "viewBox")]
        view_box: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "svg")]
    Svg {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        #[serde(default)]
        src: String,
        #[serde(default)]
        inline: Option<String>,
        width: Option<f32>,
        height: Option<f32>,
        #[serde(rename = "preserveAspectRatio")]
        preserve_aspect_ratio: Option<String>,
        duration: Option<f64>,
    },
    /// 特例一：timeline。直接子元素是场景（各带 `duration`），其间可穿插
    /// [`DomElement::Transition`]。
    #[serde(rename = "tl")]
    Tl {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        duration: Option<f64>,
        #[serde(default)]
        children: Vec<DomElement>,
    },
    /// 特例二：transition。只作为 `tl` 的直接子元素出现。
    #[serde(rename = "transition")]
    Transition {
        from: String,
        to: String,
        effect: String,
        duration: Option<f64>,
        direction: Option<String>,
        timing: Option<String>,
        damping: Option<f32>,
        stiffness: Option<f32>,
        mass: Option<f32>,
        seed: Option<f32>,
        #[serde(rename = "hueShift")]
        hue_shift: Option<f32>,
        #[serde(rename = "maskScale")]
        mask_scale: Option<f32>,
    },
    /// 音频元素：树内声明、不渲染。装配期被收进文档音频池（[`DomDocument::audio`]），
    /// `attach` 缺省指向挂载父元素。
    #[serde(rename = "audio")]
    Audio {
        id: String,
        #[serde(default)]
        attach: String,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
    },
}

impl DomElement {
    pub fn id(&self) -> &str {
        match self {
            DomElement::Div { id, .. }
            | DomElement::Text { id, .. }
            | DomElement::Canvas { id, .. }
            | DomElement::Image { id, .. }
            | DomElement::Video { id, .. }
            | DomElement::Lottie { id, .. }
            | DomElement::Path { id, .. }
            | DomElement::Svg { id, .. }
            | DomElement::Tl { id, .. }
            | DomElement::Audio { id, .. } => id,
            DomElement::Transition { .. } => "",
        }
    }

    pub fn children(&self) -> &[DomElement] {
        match self {
            DomElement::Div { children, .. }
            | DomElement::Canvas { children, .. }
            | DomElement::Video { children, .. }
            | DomElement::Tl { children, .. } => children,
            _ => &[],
        }
    }

    fn children_mut(&mut self) -> Option<&mut Vec<DomElement>> {
        match self {
            DomElement::Div { children, .. }
            | DomElement::Canvas { children, .. }
            | DomElement::Video { children, .. }
            | DomElement::Tl { children, .. } => Some(children),
            _ => None,
        }
    }

    pub fn duration(&self) -> Option<f64> {
        match self {
            DomElement::Div { duration, .. }
            | DomElement::Text { duration, .. }
            | DomElement::Canvas { duration, .. }
            | DomElement::Image { duration, .. }
            | DomElement::Video { duration, .. }
            | DomElement::Lottie { duration, .. }
            | DomElement::Path { duration, .. }
            | DomElement::Svg { duration, .. }
            | DomElement::Tl { duration, .. }
            | DomElement::Audio { duration, .. } => *duration,
            DomElement::Transition { .. } => None,
        }
    }

    pub(crate) fn is_tl(&self) -> bool {
        matches!(self, DomElement::Tl { .. })
    }
}

/// 全局音频池元素（文本格式 `<audio>` 行 / JSX `export const audio`）与树内
/// `<audio>` 元素共用同一 attach 语义。
#[derive(Debug, Clone)]
pub struct DomAudio {
    pub id: String,
    pub attach: String,
    pub source: AudioSource,
    pub duration: Option<f64>,
}

/// 一篇 DOM 文档：视口元数据 + 元素树 + 脚本声明 + 音频池。
#[derive(Debug, Clone, Default)]
pub struct DomDocument {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub duration: f64,
    pub root: Option<DomElement>,
    /// `parentId` 挂载的脚本声明（文本格式 `<script parentId>` / JSX 提取的
    /// 每元素 driver）。
    pub scripts_by_parent: HashMap<String, Vec<DeclaredScript>>,
    pub global_scripts: Vec<DeclaredScript>,
    pub audio: Vec<DomAudio>,
}

/// DOM 文档 → [`ParsedComposition`]（Node 树）。
pub fn composition_from_dom(
    doc: DomDocument,
    base_dir: Option<&Path>,
    manifest: Option<&crate::fonts::FontManifest>,
    css: Option<&crate::css::CssEngine>,
) -> anyhow::Result<ParsedComposition> {
    let font_index = manifest.map(|m| m.build_family_index());
    let mut doc = doc;
    // 树内 `<audio>` 收进音频池：不渲染、attach 缺省指向挂载父元素。
    let mut pool = doc.audio;
    if let Some(mut root) = doc.root.take() {
        harvest_audio(&mut root, None, &mut pool)?;
        doc.root = Some(root);
    }
    doc.audio = pool;
    let ctx = DomContext {
        base_dir,
        css,
        manifest,
        font_index: font_index.as_ref(),
        scripts_by_parent: &doc.scripts_by_parent,
        element_ids: HashSet::new(),
        tl_ids: HashSet::new(),
    };
    let mut ctx = ctx;
    // 音频池元素也占 id 命名空间（`<audio>` 是文档元素）。
    for audio in &doc.audio {
        if !ctx.element_ids.insert(audio.id.clone()) {
            anyhow::bail!("duplicate id `{}`", audio.id);
        }
    }
    collect_ids(doc.root.as_ref(), &mut ctx)?;

    let mut root = match doc.root {
        Some(root) => build_node(root, &mut ctx)?,
        None => anyhow::bail!("no root element found"),
    };

    if !doc.global_scripts.is_empty() {
        // Merge with any scripts already attached to root (parentId == root).
        let mut decls = Vec::new();
        if let Some(existing) = root.style_ref().script_driver.as_ref() {
            for frag in &existing.inline_fragments {
                if !frag.is_empty() {
                    decls.push(DeclaredScript::Inline(frag.clone()));
                }
            }
            if existing.externals.is_empty()
                && !existing.source.is_empty()
                && existing.inline_fragments.is_empty()
            {
                decls.push(DeclaredScript::Inline(existing.source.clone()));
            }
            for ext in &existing.externals {
                decls.push(DeclaredScript::External {
                    locator: ext.locator.clone(),
                });
            }
        }
        decls.extend(doc.global_scripts.iter().cloned());
        if let Some(driver) = script_driver_from_decls(decls)? {
            root = root.script_driver(driver);
        }
    }

    let audio_sources = doc
        .audio
        .into_iter()
        .map(|audio| resolve_audio_source(audio, &ctx))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(ParsedComposition {
        font_manifest: manifest.cloned().unwrap_or_default(),
        width: doc.width,
        height: doc.height,
        fps: doc.fps,
        duration: doc.duration,
        root,
        script: crate::parse::document::join_scripts(
            doc.global_scripts
                .iter()
                .filter_map(|s| match s {
                    DeclaredScript::Inline(t) => Some(t.clone()),
                    DeclaredScript::External { .. } => None,
                })
                .collect(),
        ),
        audio_sources,
    })
}

struct DomContext<'a> {
    base_dir: Option<&'a Path>,
    css: Option<&'a crate::css::CssEngine>,
    manifest: Option<&'a crate::fonts::FontManifest>,
    font_index: Option<&'a crate::fonts::FontFamilyIndex>,
    scripts_by_parent: &'a HashMap<String, Vec<DeclaredScript>>,
    element_ids: HashSet<String>,
    tl_ids: HashSet<String>,
}

/// 树内 `<audio>` 收进音频池（保持文档顺序）；其余子树继续下探。
fn harvest_audio(
    el: &mut DomElement,
    parent_id: Option<&str>,
    out: &mut Vec<DomAudio>,
) -> anyhow::Result<()> {
    let id = el.id().to_string();
    let Some(children) = el.children_mut() else {
        return Ok(()); // 非容器元素没有子树可收
    };
    let mut kept = Vec::with_capacity(children.len());
    for child in children.drain(..) {
        match child {
            DomElement::Audio {
                id,
                mut attach,
                path,
                url,
                duration,
            } => {
                if attach.is_empty() {
                    attach = parent_id.unwrap_or_default().to_string();
                }
                out.push(DomAudio {
                    id,
                    attach,
                    source: parse_audio_source(path, url)?,
                    duration,
                });
            }
            mut other => {
                harvest_audio(&mut other, Some(&id), out)?;
                kept.push(other);
            }
        }
    }
    *children = kept;
    Ok(())
}

/// id 唯一性校验（一次性 DFS，替代旧扁平表上的 validate_unique_ids）。
fn collect_ids(el: Option<&DomElement>, ctx: &mut DomContext<'_>) -> anyhow::Result<()> {    let Some(el) = el else { return Ok(()) };
    if el.id().is_empty() {
        return Ok(()); // transition 无 id
    }
    if !ctx.element_ids.insert(el.id().to_string()) {
        anyhow::bail!("duplicate id `{}`", el.id());
    }
    if el.is_tl() {
        ctx.tl_ids.insert(el.id().to_string());
    }
    for child in el.children() {
        collect_ids(Some(child), ctx)?;
    }
    Ok(())
}

fn resolve_style_for(
    el_id: &str,
    tag: &str,
    class: Option<&str>,
    inline_style: Option<&str>,
    ctx: &DomContext<'_>,
) -> NodeStyle {
    let mut style = crate::parse::elements::resolve_style(
        ctx.css,
        class.unwrap_or(""),
        inline_style,
        el_id,
        tag,
        0,
    );
    // 字体引用（`font-[faceId]` / 文档默认字体）在进 Node 前解析到 family。
    if let (Some(manifest), Some(index)) = (ctx.manifest, ctx.font_index)
        && !manifest.is_empty()
    {
        manifest.apply_font_refs_to_style(index, &mut style);
    }
    style
}

fn attach_driver(
    style: &mut NodeStyle,
    el_id: &str,
    ctx: &DomContext<'_>,
) -> anyhow::Result<()> {
    if let Some(decls) = ctx.scripts_by_parent.get(el_id)
        && let Some(driver) = script_driver_from_decls(decls.clone())?
    {
        style.script_driver = Some(std::sync::Arc::new(driver));
    }
    Ok(())
}

/// DOM 元素 → Node（含子树递归）。逻辑与旧 builder.build_node_inner 一致，
/// 数据源从扁平表换成直接的树。
fn build_node(el: DomElement, ctx: &mut DomContext<'_>) -> anyhow::Result<Node> {
    match el {
        DomElement::Div {
            id,
            class,
            style: inline_style,
            duration: _,
            children,
        } => {
            let mut style = resolve_style_for(&id, "div", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            let mut div_node = div();
            div_node.style = style;
            for child in children {
                div_node = div_node.child(build_node(child, ctx)?);
            }
            Ok(Node::new(div_node))
        }
        DomElement::Text {
            id,
            class,
            style: inline_style,
            text: content,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "text", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            let mut text_node = text(content);
            text_node.style = style;
            Ok(Node::new(text_node))
        }
        DomElement::Canvas {
            id,
            class,
            style: inline_style,
            children,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "canvas", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            let mut canvas_node = canvas();
            canvas_node.style = style;
            // Hidden picture subtree: children rasterize offscreen and are only
            // reachable through `canvas.getSubTree().makeShader(...)`.
            for child in children {
                canvas_node = canvas_node.hidden_child(build_node(child, ctx)?);
            }
            Ok(Node::new(canvas_node))
        }
        DomElement::Image {
            id,
            class,
            style: inline_style,
            path,
            url,
            query,
            query_count,
            aspect_ratio,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "image", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            let source = parse_image_source(path, url, query, query_count, aspect_ratio)?;
            let mut image_node = image();
            image_node = match source {
                ImageSource::Unset => {
                    anyhow::bail!("image node requires one of: path, url, query")
                }
                ImageSource::Path(path) => image_node.path(path),
                ImageSource::Url(url) => image_node.url(url.clone()),
                ImageSource::Query(query) => {
                    let mut image_node = image_node.query(query.query.clone());
                    image_node = image_node.query_count(query.count);
                    if let Some(aspect_ratio) = &query.aspect_ratio {
                        image_node = image_node.aspect_ratio(aspect_ratio.clone());
                    }
                    image_node
                }
            };
            image_node.style = style;
            Ok(Node::new(image_node))
        }
        DomElement::Video {
            id,
            class,
            style: inline_style,
            path,
            url,
            data_start,
            data_duration,
            data_media_start,
            data_color_matrix,
            looping,
            hide_after_end,
            children,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "video", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            let source = match url {
                Some(u) => VideoSource::Url(u),
                None => VideoSource::Path(path.unwrap_or_default()),
            };
            let color_matrix = match data_color_matrix.as_deref() {
                None => None,
                Some(token) => VideoColorMatrix::parse_token(token).ok_or_else(|| {
                    anyhow::anyhow!(
                        "dataColorMatrix must be auto, bt601, or bt709 (got `{token}`)"
                    )
                })?,
            };
            let timing = VideoFrameTiming {
                timeline_start_secs: data_start.unwrap_or(0.0),
                timeline_duration_secs: data_duration,
                media_start_secs: data_media_start.unwrap_or(0.0),
                playback_rate: 1.0,
                looping: looping.unwrap_or(false),
                color_matrix,
                hide_after_end: hide_after_end.unwrap_or(false),
            };
            let mut video_node = match source {
                VideoSource::Path(p) => video(p),
                VideoSource::Url(u) => video_url(u),
            };
            video_node = video_node.with_timing(timing);
            video_node.style = style;
            for child in children {
                video_node = video_node.child(build_node(child, ctx)?);
            }
            Ok(Node::new(video_node))
        }
        DomElement::Lottie {
            id,
            class,
            style: inline_style,
            path,
            url,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "div", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            let source = match (path, url) {
                (Some(p), None) => crate::parse::primitives::LottieSource::Path(p),
                (None, Some(u)) => crate::parse::primitives::LottieSource::Url(u),
                (Some(_), Some(_)) => anyhow::bail!("lottie `{id}`: only one of path / url"),
                (None, None) => anyhow::bail!("lottie `{id}`: needs path or url"),
            };
            let mut lottie_node = crate::parse::primitives::lottie();
            lottie_node = match source {
                crate::parse::primitives::LottieSource::Unset => {
                    anyhow::bail!("lottie node requires one of: path, url, src")
                }
                crate::parse::primitives::LottieSource::Path(path) => lottie_node.path(path),
                crate::parse::primitives::LottieSource::Url(url) => lottie_node.url(url.clone()),
            };
            lottie_node = lottie_node.with_timing(VideoFrameTiming::default());
            lottie_node.style = style;
            Ok(Node::new(lottie_node))
        }
        DomElement::Path {
            id,
            class,
            style: inline_style,
            d,
            view_box,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "path", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            let view_box = view_box.as_deref().map(parse_view_box_attr).transpose()?;
            let mut path_node = path(d);
            if let Some(vb) = view_box {
                path_node.set_view_box(vb);
            }
            path_node.style = style;
            Ok(Node::new(path_node))
        }
        DomElement::Svg {
            id,
            class,
            style: inline_style,
            src,
            inline,
            width,
            height,
            preserve_aspect_ratio,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "div", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;
            // 内联 markup 不落盘、无文件语义：src 仅在文件模式参与 base_dir 解析。
            let resolved_src = if inline.is_some() {
                PathBuf::new()
            } else if let Some(base_dir) = ctx.base_dir {
                base_dir.join(&src)
            } else {
                PathBuf::from(src)
            };
            let mut svg_node = crate::parse::primitives::svg(resolved_src);
            svg_node.inline = inline;
            svg_node.width = width;
            svg_node.height = height;
            svg_node.preserve_aspect_ratio = preserve_aspect_ratio;
            svg_node.style = style;
            Ok(Node::new(svg_node))
        }
        DomElement::Tl {
            id,
            class,
            style: inline_style,
            children,
            ..
        } => {
            let mut style = resolve_style_for(&id, "div", class.as_deref(), inline_style.as_deref(), ctx);
            attach_driver(&mut style, &id, ctx)?;

            // 子元素分流：场景 vs transition（保持文档顺序）。
            let mut scenes: Vec<DomElement> = Vec::new();
            let mut transitions: Vec<DomElement> = Vec::new();
            for child in children {
                match child {
                    DomElement::Transition { .. } => transitions.push(child),
                    scene => scenes.push(scene),
                }
            }
            if scenes.len() < 2 {
                anyhow::bail!(
                    "timeline `{}` must have at least two direct child scenes",
                    id
                );
            }

            let child_ids: Vec<String> = scenes.iter().map(|c| c.id().to_string()).collect();
            let child_positions: HashMap<String, usize> =
                child_ids.iter().enumerate().map(|(i, id)| (id.clone(), i)).collect();

            let mut transitions_by_pair: HashMap<(String, String), DomElement> = HashMap::new();
            for t in transitions {
                let DomElement::Transition { from, to, .. } = &t else {
                    unreachable!("分流后只有 Transition");
                };
                let (from, to) = (from.clone(), to.clone());
                let Some(&from_idx) = child_positions.get(from.as_str()) else {
                    anyhow::bail!(
                        "transition `from` references `{from}`, which is not a direct child of this timeline"
                    );
                };
                let Some(&to_idx) = child_positions.get(to.as_str()) else {
                    anyhow::bail!(
                        "transition `to` references `{to}`, which is not a direct child of this timeline"
                    );
                };
                if to_idx != from_idx + 1 {
                    let next_id = child_ids.get(from_idx + 1).map(String::as_str).unwrap_or("<end>");
                    anyhow::bail!(
                        "transition `{from}` -> `{to}` is not between adjacent children (expected `{from}` -> `{next_id}`)"
                    );
                }
                if transitions_by_pair
                    .insert((from.clone(), to.clone()), t)
                    .is_some()
                {
                    anyhow::bail!(
                        "duplicate transition declared for `{}` -> `{}`",
                        from,
                        to
                    );
                }
            }

            // Adjacent scenes need not declare a transition: without one the
            // two scenes cut straight from one to the next.
            let mut tl_builder = timeline();
            for (index, scene) in scenes.into_iter().enumerate() {
                let duration = scene.duration().ok_or_else(|| {
                    anyhow::anyhow!(
                        "timeline sequence `{}` is missing a duration",
                        scene.id()
                    )
                })?;
                let node = build_node(scene, ctx)?;
                tl_builder = tl_builder.sequence(duration, node);

                let Some(next_id) = child_ids.get(index + 1) else {
                    continue;
                };
                let key = (child_ids[index].clone(), next_id.clone());
                if let Some(transition) = transitions_by_pair.get(&key) {
                    tl_builder = tl_builder.transition(build_transition(transition)?);
                }
            }

            let timeline_node: Node = tl_builder.into();
            let mut kind = timeline_node.kind().clone();
            *kind.style_mut() = style;
            Ok(Node::new(kind))
        }
        DomElement::Transition { .. } => anyhow::bail!(
            "transition element must be a direct child of a `tl` node"
        ),
        DomElement::Audio { .. } => anyhow::bail!(
            "audio element must live in the audio pool or be attached via `attach`; it does not render"
        ),
    }
}

fn build_transition(transition: &DomElement) -> anyhow::Result<Transition> {
    let DomElement::Transition {
        from: _,
        to: _,
        effect,
        duration,
        direction,
        timing,
        damping,
        stiffness,
        mass,
        seed,
        hue_shift,
        mask_scale,
    } = transition
    else {
        unreachable!("build_transition 只吃 Transition");
    };
    let easing = parse_transition_easing(*damping, *stiffness, *mass, timing.as_deref())?;
    let duration = duration.unwrap_or(0.0);
    let effect = normalize_transition_name(effect);

    match effect.as_str() {
        "fade" => Ok(fade().timing(easing, duration)),
        // Consumes the overlap but applies no effect: the incoming scene covers
        // the outgoing one outright, i.e. a hard cut at the seam.
        "cut" | "none" => Ok(cut().timing(easing, duration)),
        "clock_wipe" => Ok(clock_wipe().timing(easing, duration)),
        "iris" => Ok(iris().timing(easing, duration)),
        "slide" => match direction.as_deref().map(normalize_transition_name).as_deref() {
            None | Some("from_left") => Ok(slide().from_left().timing(easing, duration)),
            Some("from_right") => Ok(slide().from_right().timing(easing, duration)),
            Some("from_top") => Ok(slide().from_top().timing(easing, duration)),
            Some("from_bottom") => Ok(slide().from_bottom().timing(easing, duration)),
            Some(direction) => Err(anyhow::anyhow!("unsupported slide direction `{direction}`")),
        },
        "wipe" => match direction.as_deref().map(normalize_transition_name).as_deref() {
            None | Some("from_left") => Ok(wipe().from_left().timing(easing, duration)),
            Some("from_right") => Ok(wipe().from_right().timing(easing, duration)),
            Some("from_top") => Ok(wipe().from_top().timing(easing, duration)),
            Some("from_bottom") => Ok(wipe().from_bottom().timing(easing, duration)),
            Some("from_top_left") => Ok(wipe().from_top_left().timing(easing, duration)),
            Some("from_top_right") => Ok(wipe().from_top_right().timing(easing, duration)),
            Some("from_bottom_left") => Ok(wipe().from_bottom_left().timing(easing, duration)),
            Some("from_bottom_right") => Ok(wipe().from_bottom_right().timing(easing, duration)),
            Some(direction) => Err(anyhow::anyhow!("unsupported wipe direction `{direction}`")),
        },
        "light_leak" => {
            let mut builder = light_leak();
            if let Some(seed) = seed {
                builder = builder.seed(*seed);
            }
            if let Some(hue_shift) = hue_shift {
                builder = builder.hue_shift(*hue_shift);
            }
            if let Some(mask_scale) = mask_scale {
                builder = builder.mask_scale(*mask_scale);
            }
            Ok(builder.timing(easing, duration))
        }
        _ => Ok(gl_transition(effect).timing(easing, duration)),
    }
}

fn parse_transition_easing(
    damping: Option<f32>,
    stiffness: Option<f32>,
    mass: Option<f32>,
    timing: Option<&str>,
) -> anyhow::Result<Easing> {
    if damping.is_some() || stiffness.is_some() || mass.is_some() {
        let mut config = SpringConfig::default();
        if let Some(damping) = damping {
            config.damping = damping;
        }
        if let Some(stiffness) = stiffness {
            config.stiffness = stiffness;
        }
        if let Some(mass) = mass {
            config.mass = mass;
        }
        return Ok(Easing::Spring(config));
    }

    let normalized = timing
        .map(normalize_transition_name)
        .unwrap_or_else(|| "linear".to_string());

    easing_from_name(&normalized).ok_or_else(|| {
        anyhow::anyhow!(
            "unsupported transition timing `{}`",
            timing.unwrap_or("unknown")
        )
    })
}

fn normalize_transition_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn resolve_audio_source(audio: DomAudio, ctx: &DomContext<'_>) -> anyhow::Result<CompositionAudioSource> {
    let attachment = if ctx.tl_ids.contains(audio.attach.as_str()) {
        AudioAttachment::Timeline
    } else if ctx.element_ids.contains(audio.attach.as_str()) {
        AudioAttachment::Scene {
            scene_id: audio.attach.clone(),
        }
    } else {
        anyhow::bail!(
            "audio `{}` attach references non-existent element `{}`",
            audio.id,
            audio.attach
        );
    };
    Ok(CompositionAudioSource {
        id: audio.id,
        source: audio.source,
        attach: attachment,
        duration_secs: audio.duration,
    })
}

fn parse_image_source(
    path: Option<String>,
    url: Option<String>,
    query: Option<String>,
    query_count: Option<usize>,
    aspect_ratio: Option<String>,
) -> anyhow::Result<ImageSource> {
    let sources = [
        path.as_ref().map(|_| "path"),
        url.as_ref().map(|_| "url"),
        query.as_ref().map(|_| "query"),
    ]
    .into_iter()
    .flatten()
    .count();

    if sources == 0 {
        return Err(anyhow::anyhow!("image node requires one of: path, url, query"));
    }
    if sources > 1 {
        return Err(anyhow::anyhow!("image node accepts only one source: path, url, or query"));
    }

    if let Some(path) = path {
        return Ok(ImageSource::Path(path));
    }
    if let Some(url) = url {
        return Ok(ImageSource::Url(url));
    }
    Ok(ImageSource::Query(OpenverseQuery {
        query: query.unwrap_or_default(),
        count: query_count.unwrap_or(1).max(1),
        aspect_ratio,
    }))
}

fn parse_view_box_attr(raw: &str) -> anyhow::Result<[f32; 4]> {
    let parts: Vec<&str> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() != 4 {
        anyhow::bail!(
            "`viewBox` must be `min-x,min-y,width,height` (4 numbers, got {})",
            parts.len()
        );
    }
    let mut out = [0.0f32; 4];
    for (slot, part) in out.iter_mut().zip(parts.iter()) {
        *slot = part
            .parse::<f32>()
            .map_err(|e| anyhow::anyhow!("`viewBox`: invalid number `{part}`: {e}"))?;
        if !slot.is_finite() {
            anyhow::bail!("`viewBox` values must be finite");
        }
    }
    if out[2] <= 0.0 || out[3] <= 0.0 {
        anyhow::bail!("`viewBox` width and height must be positive");
    }
    Ok(out)
}

pub(crate) fn parse_audio_source(
    path: Option<String>,
    url: Option<String>,
) -> anyhow::Result<AudioSource> {
    match (path, url) {
        (Some(path), None) => Ok(AudioSource::Path(PathBuf::from(path))),
        (None, Some(url)) => Ok(AudioSource::Url(url)),
        (Some(_), Some(_)) => anyhow::bail!("audio must use only one of path / url"),
        (None, None) => anyhow::bail!("audio requires path or url"),
    }
}
