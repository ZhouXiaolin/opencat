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
    #[serde(rename = "text")]
    Text {
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

/// Test-only line-text helper: core unit tests describe fixtures in the line
/// shape without a product text frontend. Decodes text lines into
/// [`JsonLine`] and runs the shared assembly.
#[cfg(test)]
pub(crate) fn parse_lines_for_tests(
    input: &str,
    base_dir: Option<&Path>,
) -> anyhow::Result<ParsedComposition> {
    let lines = input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<JsonLine>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    // 测试行里的 className 与生产一致：交给标准 CSS 引擎。夹具样式表是
    // 布局/解析测试用到的类经 tailwindcss CLI 预编译的产物。
    let engine = crate::css::CssEngine::from_css_text(
        crate::test_support::layout_test_css(),
        crate::css::EvalContext::default(),
    )
    .expect("test css compiles");
    composition_from_lines(lines, Vec::new(), base_dir, None, Some(&engine))
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
        JsonLine::Text {
            id,
            parent_id,
            class_name,
            style,
            text,
            duration,
        } => (
            parent_id,
            DomElement::Text {
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

#[cfg(test)]
mod tests {

    fn parse(input: &str) -> anyhow::Result<crate::parse::document::ParsedComposition> {
        super::parse_lines_for_tests(input, None)
    }
    use crate::parse::{composition::AudioAttachment, node::NodeKind};
    use crate::style::Position;

    #[test]
    fn jsonl_parser_uses_shared_parsed_composition_type() {
        let parsed: crate::parse::document::ParsedComposition = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}"#,
        )
        .expect("jsonl should parse");

        assert_eq!(parsed.width, 320);
        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn parser_maps_path_view_box_when_present() {
        let parsed = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"type":"path","id":"a","parentId":"root","className":"w-[115px] h-[115px]","d":"M0 0 L10 10","viewBox":"0 0 512 512"}
{"type":"path","id":"b","parentId":"root","className":"w-[10px] h-[10px]","d":"M0 0 L10 10"}"#,
        )
        .expect("jsonl should parse");

        let crate::parse::node::NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be a div");
        };
        let find = |id: &str| {
            root.children_ref()
                .iter()
                .find(|n| n.style_ref().id == id)
                .unwrap_or_else(|| panic!("path `{id}` should exist"))
        };
        let crate::parse::node::NodeKind::Path(a) = find("a").kind() else {
            panic!("`a` should be a path");
        };
        assert_eq!(a.view_box(), Some([0.0, 0.0, 512.0, 512.0]));
        let crate::parse::node::NodeKind::Path(b) = find("b").kind() else {
            panic!("`b` should be a path");
        };
        assert_eq!(b.view_box(), None);
    }

    #[test]
    fn parser_keeps_script_line() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","src":"ctx.getNode('root').opacity(0.5);"}"#,
        )
        .expect("jsonl should parse");

        assert_eq!(parsed.width, 640);
        assert_eq!(parsed.height, 360);
        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('root').opacity(0.5);")
        );
    }

    #[test]
    fn parser_keeps_legacy_script_content_line() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","content":"ctx.getNode('root').opacity(0.5);"}"#,
        )
        .expect("jsonl with legacy content should parse");

        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('root').opacity(0.5);")
        );
    }

    #[test]
    fn parser_rejects_root_level_timeline_without_explicit_tl() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":33.3}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":0.333333333333}
{"type":"script","parentId":"root-a","src":"ctx.getNode('root-a').opacity(0.6);"}
{"type":"transition","parentId":"main-tl","from":"root-a","to":"root-b","effect":"fade","duration":0.166666666666}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":0.333333333333}"#,
        )
        .err()
        .expect("legacy root timeline should fail");

        assert!(err.to_string().contains("tl"));
    }

    #[test]
    fn parser_rejects_multiple_root_sequences_without_explicit_tl() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":33.3}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":0.333333333333}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":0.333333333333}"#,
        )
        .err()
        .expect("multiple roots without tl should fail");

        assert!(err.to_string().contains("multiple root"));
    }

    #[test]
    fn parser_rejects_non_adjacent_tl_transition_pairs() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":1.166666666667}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":1.166666666667}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-c","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-c","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("non-adjacent transitions should fail");

        assert!(err.to_string().contains("adjacent"));
    }

    #[test]
    fn parser_requires_sequence_duration_when_building_timeline() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":""}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("timeline without durations should fail");

        assert!(err.to_string().contains("missing a duration"));
    }

    #[test]
    fn parser_treats_timeline_attached_audio_as_timeline_audio_source() {
        let parsed = parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"duration":6}
{"id":"main-tl","parentId":null,"type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"w-full h-full","duration":1}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"w-full h-full","duration":1}
{"id":"bgm","attach":"main-tl","type":"audio","path":"/tmp/demo.mp3"}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.033333333333}"#,
        )
        .expect("jsonl with timeline-attached audio should parse");

        assert_eq!(parsed.audio_sources.len(), 1);
        assert!(matches!(
            parsed.audio_sources[0].attach,
            AudioAttachment::Timeline
        ));
    }

    #[test]
    fn parser_attaches_audio_to_owning_scene() {
        let parsed = parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"duration":6}
{"id":"scene-a","parentId":null,"type":"div","className":"w-full h-full","duration":1}
{"id":"voice","attach":"scene-a","type":"audio","path":"/tmp/voice.mp3"}"#,
        )
        .expect("jsonl with scene audio should parse");

        assert_eq!(parsed.audio_sources.len(), 1);
        assert!(matches!(
            &parsed.audio_sources[0].attach,
            AudioAttachment::Scene { scene_id } if scene_id == "scene-a"
        ));
    }

    #[test]
    fn parser_accepts_explicit_tl_root_and_local_transition() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .expect("explicit tl jsonl should parse");

        assert!((parsed.duration - 25.0 / 30.0).abs() < 1e-9);
        assert!(matches!(parsed.root.kind(), NodeKind::Div(_)));
    }

    #[test]
    fn parser_requires_transition_parent_id() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("missing transition parentId should fail");

        assert!(err.to_string().contains("parentId"));
    }

    #[test]
    fn parser_rejects_legacy_layer_records() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"scene-a","parentId":null,"type":"div","className":"","duration":0.333333333333}
{"type":"layer","children":["scene-a"]}"#,
        )
        .err()
        .expect("legacy layer input should fail");

        assert!(err.to_string().contains("layer"));
    }

    #[test]
    fn parser_builds_tl_node_from_direct_children() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"type":"script","parentId":"main-tl","src":"ctx.getNode('main-tl').opacity(0.5);"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":0.166666666666}"#,
        )
        .expect("tl jsonl should parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Timeline(tl) = root.children_ref()[0].kind() else {
            panic!("child should be timeline");
        };
        let frame_ctx = crate::frame_ctx::FrameCtx {
            frame: 0,
            fps: parsed.fps as u32,
            width: parsed.width,
            height: parsed.height,
            frames: crate::frame_ctx::duration_secs_to_frames(parsed.duration, parsed.fps as u32),
        };
        assert_eq!(tl.duration_in_frames(&frame_ctx), 25);
        assert_eq!(tl.style_ref().id, "main-tl");
        assert_eq!(tl.style_ref().position, Some(Position::Absolute));
        assert!(tl.style_ref().script_driver.is_some());
    }

    #[test]
    fn parser_rejects_transition_that_targets_non_adjacent_tl_child() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":1.166666666667}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":1.166666666667}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-c","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-c","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("non-adjacent transition should fail");

        assert!(err.to_string().contains("adjacent"));
    }

    #[test]
    fn parser_rejects_transition_to_non_direct_tl_descendant() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.833333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.833333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"nested","parentId":"scene-b","type":"div","className":"","duration":0.333333333333}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"nested","effect":"fade","duration":0.166666666666}"#,
        )
        .err()
        .expect("transition to nested descendant should fail");

        assert!(err.to_string().contains("direct child"));
    }

    #[test]
    fn parser_rejects_tl_with_single_scene_child() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.333333333333}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.333333333333}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}"#,
        )
        .err()
        .expect("single-scene tl should fail");

        assert!(err.to_string().contains("at least two"));
    }

    #[test]
    fn parser_accepts_tl_hard_cut_between_adjacent_children() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":0.666666666667}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":0.666666666667}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":0.333333333333}"#,
        )
        .expect("adjacent scenes without a transition should parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Timeline(timeline) = root.children_ref()[0].kind() else {
            panic!("expected a timeline node");
        };
        assert_eq!(timeline.segments().len(), 2);
    }

    #[test]
    fn jsonl_allows_canvas_hidden_visual_children() {
        // JSONL (and therefore the JSX frontend, which lowers to JSONL lines)
        // accepts canvas hidden picture subtrees, same as markup
        // (markup_allows_canvas_hidden_visual_children): visual children are
        // rasterized offscreen and reachable only through
        // canvas.getSubTree().makeShader(...).
        let parsed = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"stage","parentId":"root","type":"canvas","className":"w-full h-full"}
{"id":"hidden","parentId":"stage","type":"text","className":"text-[12px]","text":"hidden"}"#,
        )
        .expect("canvas hidden children should parse");

        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn jsonl_rejects_duplicate_visual_ids() {
        let err = parse(
            r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":0.033333333333}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"dup","parentId":"root","type":"div","className":""}
{"id":"dup","parentId":"root","type":"text","className":"","text":"x"}"#,
        )
        .expect_err("duplicate ids should fail");

        assert!(err.to_string().contains("duplicate id `dup`"));
    }
}
