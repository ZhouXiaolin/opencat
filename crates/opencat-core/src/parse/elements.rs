use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::parse::document::{DeclaredScript, ParsedComposition};
use crate::parse::dom::{DomAudio, DomDocument, DomElement, composition_from_dom, parse_audio_source};
use crate::style::NodeStyle;

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(tag = "type")]
pub enum JsonLine {
    #[serde(rename = "composition")]
    Composition {
        width: i32,
        height: i32,
        fps: i32,
        duration: f64,
    },
    #[serde(rename = "script")]
    Script {
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(alias = "content")]
        src: Option<String>,
        path: Option<String>,
    },
    #[serde(rename = "div")]
    Div {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "p")]
    P {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        text: String,
        duration: Option<f64>,
    },
    #[serde(rename = "span")]
    Span {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        text: String,
        duration: Option<f64>,
    },
    #[serde(rename = "canvas")]
    Canvas {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "image")]
    Image {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
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
    #[serde(rename = "audio")]
    Audio {
        id: String,
        attach: String,
        #[allow(dead_code)]
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "video")]
    Video {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
        /// Media window, timing and color-matrix overrides, mirroring the
        /// markup `data-*` attributes on `<video>`. Both the kebab-case
        /// markup spelling and the camelCase JSX spelling are accepted.
        /// All optional; absent fields keep the legacy behaviour (window
        /// opens at 0s, auto color matrix, node holds its last frame).
        #[serde(rename = "data-start", alias = "dataStart")]
        data_start: Option<f64>,
        #[serde(rename = "data-duration", alias = "dataDuration")]
        data_duration: Option<f64>,
        #[serde(rename = "data-media-start", alias = "dataMediaStart")]
        data_media_start: Option<f64>,
        #[serde(rename = "data-color-matrix", alias = "dataColorMatrix")]
        data_color_matrix: Option<String>,
        #[serde(rename = "data-hide-after-end")]
        hide_after_end: Option<bool>,
        #[serde(rename = "loop")]
        looping: Option<bool>,
    },
    #[serde(rename = "transition")]
    Transition {
        #[serde(rename = "parentId")]
        parent_id: String,
        from: String,
        to: String,
        effect: String,
        duration: f64,
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
    #[serde(rename = "tl")]
    Tl {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        /// A `<tl>` is a scene like any other element, so it declares its own
        /// length. Required when the timeline is a direct child of another
        /// timeline — the enclosing sequence reads each child's duration to
        /// place it. Inert when the timeline is a plain child of a div.
        duration: Option<f64>,
    },
    #[serde(rename = "path")]
    Path {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        d: String,
        /// Optional shared SVG view box, `"minx,miny,w,h"` — same grammar as the
        /// markup `view-box` attribute.
        #[serde(rename = "viewBox")]
        view_box: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "svg")]
    Svg {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        #[serde(default)]
        src: String,
        /// Inline svg markup (e.g. a bundled lucide-react icon). Wins over
        /// `src`; no file resolution happens for it.
        #[serde(default)]
        inline: Option<String>,
        width: Option<f32>,
        height: Option<f32>,
        #[serde(rename = "preserveAspectRatio")]
        preserve_aspect_ratio: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "lottie")]
    Lottie {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className", alias = "class")]
        class_name: Option<String>,
        /// Inline `style` attribute, already serialized to CSS declaration
        /// text by the JSX runtime. Wins over class rules, per CSS cascade.
        style: Option<String>,
        path: Option<String>,
        url: Option<String>,
        duration: Option<f64>,
    },
}



/// 文本格式装配：JSONL 行形（[`JsonLine`]，文本序列化格式）按 `parentId`
/// 分组成 [`DomDocument`]，再走共享 DOM 装配。`audio` 是文档级音频池
/// （JSX `export const audio` 解析产物）；测试夹具与遗留文本入口从这里进。
pub(crate) fn composition_from_lines(
    lines: Vec<JsonLine>,
    audio: Vec<DomAudio>,
    base_dir: Option<&Path>,
    manifest: Option<&crate::fonts::FontManifest>,
    css: Option<&crate::css::CssEngine>,
) -> anyhow::Result<ParsedComposition> {
    let mut doc = lines_to_dom(lines)?;
    doc.audio.extend(audio);
    composition_from_dom(doc, base_dir, manifest, css)
}

/// Keep external scripts as logical locators. Host supplies text via lifecycle
/// HostInputs — core never reads script files or rewrites the input string.
fn resolve_script_declaration(
    src: Option<String>,
    path: Option<String>,
) -> anyhow::Result<DeclaredScript> {
    match (src, path) {
        (Some(source), None) => Ok(DeclaredScript::Inline(source)),
        (None, Some(locator)) => Ok(DeclaredScript::External { locator }),
        (Some(_), Some(_)) => anyhow::bail!("script node must use only one of src/content or path"),
        (None, None) => anyhow::bail!("script node requires either src/content or path"),
    }
}

fn lines_to_dom(lines: Vec<JsonLine>) -> anyhow::Result<DomDocument> {
    let mut doc = DomDocument::default();
    let mut by_parent: HashMap<String, Vec<DomElement>> = HashMap::new();
    let mut transitions_by_parent: HashMap<String, Vec<DomElement>> = HashMap::new();
    let mut roots: Vec<DomElement> = Vec::new();

    for line in lines {
        match line {
            JsonLine::Composition {
                width,
                height,
                fps,
                duration,
            } => {
                doc.width = width;
                doc.height = height;
                doc.fps = fps;
                doc.duration = duration;
            }
            JsonLine::Script {
                parent_id,
                src,
                path,
            } => {
                let decl = resolve_script_declaration(src, path)?;
                if let Some(parent_id) = parent_id {
                    doc.scripts_by_parent.entry(parent_id).or_default().push(decl);
                } else {
                    doc.global_scripts.push(decl);
                }
            }
            JsonLine::Audio {
                id,
                attach,
                class_name: _,
                path,
                url,
                duration,
            } => {
                doc.audio.push(DomAudio {
                    id,
                    attach,
                    source: parse_audio_source(path, url)?,
                    duration,
                });
            }
            JsonLine::Transition {
                parent_id,
                from,
                to,
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
            } => {
                transitions_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(DomElement::Transition {
                        from,
                        to,
                        effect,
                        duration: Some(duration),
                        direction,
                        timing,
                        damping,
                        stiffness,
                        mass,
                        seed,
                        hue_shift,
                        mask_scale,
                    });
            }
            other => {
                let (parent_id, el) = line_to_element(other);
                push_element(&mut by_parent, &mut roots, parent_id, el);
            }
        }
    }

    let mut assembled = Vec::with_capacity(roots.len());
    for root in roots {
        assembled.push(attach_children(root, &mut by_parent, &mut transitions_by_parent)?);
    }
    // 残留 transition = parent 从未作为元素出现（或非 tl），先于根判定报错。
    if let Some((parent_id, _)) = transitions_by_parent.iter().next() {
        anyhow::bail!("transition parentId `{parent_id}` must reference a tl node");
    }
    for root in assembled {
        doc_root(&mut doc, root)?;
    }
    Ok(doc)
}

fn push_element(
    by_parent: &mut HashMap<String, Vec<DomElement>>,
    roots: &mut Vec<DomElement>,
    parent_id: Option<String>,
    el: DomElement,
) {
    match parent_id {
        Some(parent_id) => by_parent.entry(parent_id).or_default().push(el),
        None => roots.push(el),
    }
}

fn doc_root(doc: &mut DomDocument, root: DomElement) -> anyhow::Result<()> {
    match doc.root {
        None => {
            doc.root = Some(root);
            Ok(())
        }
        Some(_) => anyhow::bail!("multiple root elements found"),
    }
}

fn line_to_element(line: JsonLine) -> (Option<String>, DomElement) {
    match line {
        JsonLine::Div {
            id,
            parent_id,
            class_name,
            style,
            duration,
        } => (
            parent_id,
            DomElement::Div {
                id,
                class: class_name,
                style,
                duration,
                children: Vec::new(),
            },
        ),
        JsonLine::P {
            id,
            parent_id,
            class_name,
            style,
            text,
            duration,
        } => (
            parent_id,
            DomElement::P {
                id,
                class: class_name,
                style,
                text,
                duration,
            },
        ),
        JsonLine::Span {
            id,
            parent_id,
            class_name,
            style,
            text,
            duration,
        } => (
            parent_id,
            DomElement::Span {
                id,
                class: class_name,
                style,
                text,
                duration,
            },
        ),
        JsonLine::Canvas {
            id,
            parent_id,
            class_name,
            style,
            duration,
        } => (
            parent_id,
            DomElement::Canvas {
                id,
                class: class_name,
                style,
                duration,
                children: Vec::new(),
            },
        ),
        JsonLine::Image {
            id,
            parent_id,
            class_name,
            style,
            path,
            url,
            query,
            query_count,
            aspect_ratio,
            duration,
        } => (
            parent_id,
            DomElement::Image {
                id,
                class: class_name,
                style,
                path,
                url,
                query,
                query_count,
                aspect_ratio,
                duration,
            },
        ),
        JsonLine::Video {
            id,
            parent_id,
            class_name,
            style,
            path,
            url,
            duration,
            data_start,
            data_duration,
            data_media_start,
            data_color_matrix,
            looping,
            hide_after_end,
        } => (
            parent_id,
            DomElement::Video {
                id,
                class: class_name,
                style,
                path,
                url,
                duration,
                data_start,
                data_duration,
                data_media_start,
                data_color_matrix,
                hide_after_end,
                looping,
                children: Vec::new(),
            },
        ),
        JsonLine::Tl {
            id,
            parent_id,
            class_name,
            style,
            duration,
        } => (
            parent_id,
            DomElement::Tl {
                id,
                class: class_name,
                style,
                duration,
                children: Vec::new(),
            },
        ),
        JsonLine::Path {
            id,
            parent_id,
            class_name,
            style,
            d,
            view_box,
            duration,
        } => (
            parent_id,
            DomElement::Path {
                id,
                class: class_name,
                style,
                d,
                view_box,
                duration,
            },
        ),
        JsonLine::Svg {
            id,
            parent_id,
            class_name,
            style,
            src,
            inline,
            width,
            height,
            preserve_aspect_ratio,
            duration,
        } => (
            parent_id,
            DomElement::Svg {
                id,
                class: class_name,
                style,
                src,
                inline,
                width,
                height,
                preserve_aspect_ratio,
                duration,
            },
        ),
        JsonLine::Lottie {
            id,
            parent_id,
            class_name,
            style,
            path,
            url,
            duration,
        } => (
            parent_id,
            DomElement::Lottie {
                id,
                class: class_name,
                style,
                path,
                url,
                duration,
            },
        ),
        JsonLine::Audio { id, attach, .. } => (
            // 树内不渲染音频：兜底成独立元素，装配期给出明确报错。
            None,
            DomElement::Audio {
                id,
                attach,
                path: None,
                url: None,
                duration: None,
            },
        ),
        other => (None, other_to_transition(other)),
    }
}

fn other_to_transition(line: JsonLine) -> DomElement {
    let JsonLine::Transition {
        parent_id: _,
        from,
        to,
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
    } = line
    else {
        unreachable!("line_to_element 已分流其余变体")
    };
    DomElement::Transition {
        from,
        to,
        effect,
        duration: Some(duration),
        direction,
        timing,
        damping,
        stiffness,
        mass,
        seed,
        hue_shift,
        mask_scale,
    }
}

/// 把 `by_parent` 里的孩子 DFS 挂到元素上。非容器元素的孩子沿用旧行为：
/// 忽略（旧扁平表装配只被 div/canvas/video/tl 消费）。
fn attach_children(
    el: DomElement,
    by_parent: &mut HashMap<String, Vec<DomElement>>,
    transitions_by_parent: &mut HashMap<String, Vec<DomElement>>,
) -> anyhow::Result<DomElement> {
    let id = el.id().to_string();
    let mut children = by_parent.remove(&id).unwrap_or_default();
    let transitions = transitions_by_parent.remove(&id).unwrap_or_default();
    if !transitions.is_empty() {
        if !el.is_tl() {
            anyhow::bail!("transition parentId `{id}` must reference a tl node");
        }
        children.extend(transitions);
    }
    let children = children
        .into_iter()
        .map(|child| attach_children(child, by_parent, transitions_by_parent))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(match el {
        DomElement::Div {
            id,
            class,
            style,
            duration,
            ..
        } => DomElement::Div {
            id,
            class,
            style,
            duration,
            children,
        },
        DomElement::Canvas {
            id,
            class,
            style,
            duration,
            ..
        } => DomElement::Canvas {
            id,
            class,
            style,
            duration,
            children,
        },
        DomElement::Video {
            id,
            class,
            style,
            path,
            url,
            duration,
            data_start,
            data_duration,
            data_media_start,
            data_color_matrix,
            hide_after_end,
            looping,
            ..
        } => DomElement::Video {
            id,
            class,
            style,
            path,
            url,
            duration,
            data_start,
            data_duration,
            data_media_start,
            data_color_matrix,
            hide_after_end,
            looping,
            children,
        },
        DomElement::Tl {
            id,
            class,
            style,
            duration,
            ..
        } => DomElement::Tl {
            id,
            class,
            style,
            duration,
            children,
        },
        other => other,
    })
}

/// class + inline style → NodeStyle 的统一入口。
///
/// class 由标准 CSS 引擎层叠（tailwind 写法的类由构建期插件编译成标准 CSS，
/// 以 `export const css` 嵌进 bundle）。内联 `style` 是标准 CSS 的一部分，
/// 层叠优先级高于任何选择器规则，因此作为最后一段声明应用。
///
/// 没有样式表时仍要解析内联声明：建一个空表，根变量为空，层叠只剩内联
/// 声明本身。样式表规模与构建代价无关（规则为空）。
pub(crate) fn resolve_style(
    css: Option<&crate::css::CssEngine>,
    class_name: &str,
    inline_style: Option<&str>,
    node_id: &str,
    tag: &str,
    _line_number: usize,
) -> NodeStyle {
    match css {
        Some(engine) => {
            let mut style = engine.compute_node_style(class_name, node_id, tag);
            if let Some(text) = inline_style {
                engine.apply_inline_style(&mut style, text, node_id, tag);
            }
            style
        }
        None => {
            let mut style = NodeStyle {
                id: node_id.to_string(),
                ..NodeStyle::default()
            };
            if let Some(text) = inline_style {
                match crate::css::CssEngine::from_css_text("", crate::css::EvalContext::default()) {
                    Ok(engine) => engine.apply_inline_style(&mut style, text, node_id, tag),
                    Err(err) => log::warn!("css: 内联 style 引擎不可用：{err}"),
                }
            }
            style
        }
    }
}


