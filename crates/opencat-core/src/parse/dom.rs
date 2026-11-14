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
    AudioSource, ImageSource, OpenverseQuery, VideoSource, canvas, div, image, path, span, text,
    video, video_url,
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
        /// HTML 前端（docs/html-align/DESIGN.md）的可见窗协议属性：
        /// `[start, start+duration)` 半开窗外 = Display::None（整子树退出
        /// 布局与绘制）。JSX 通道不产生，serde default 向后兼容。
        #[serde(default, rename = "data-start")]
        data_start: Option<f64>,
        #[serde(default, rename = "data-duration")]
        data_duration: Option<f64>,
        /// 原始 HTML 标签（html5ever 前端产；JSX 通道为 None）。保留它是为了
        /// 行内/块级判定（`<span>` 带元素子节点时也落 Div 承载）与 CSS tag
        /// 选择器输入。JS 端 payload 原样回传（`uaTag`）。
        #[serde(default, rename = "uaTag")]
        ua_tag: Option<String>,
        /// 原文属性表（文档序）。属性选择器（`[data-composition-id="…"]`）
        /// 的匹配输入；JSX 通道与脚本新建节点为空。JS 端 payload 原样回传。
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attrs: Vec<(String, String)>,
        #[serde(default)]
        children: Vec<DomElement>,
    },
    // HTML 语义的文本标签：p=块级段落、span=行内片段。两者当前共用同一
    // 文本节点机制（单字符串、宽约束断行、text-align）；UA 表取 p
    // { margin: 0 }（swgl 对齐基线是旧 text 渲的，版式零漂移优先）。
    #[serde(rename = "p")]
    P {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        text: String,
        /// 映射前的 HTML 语义标签（h1-h6→p、b/strong/i/em→span）：进 CSS
        /// 层叠的 tag 选择器输入，UA 兜底表按它命中字形默认。
        #[serde(rename = "uaTag", default)]
        ua_tag: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "span")]
    Span {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        text: String,
        /// 同 `P::ua_tag`（b/strong/i/em 的真标签名）。
        #[serde(rename = "uaTag", default)]
        ua_tag: Option<String>,
        duration: Option<f64>,
    },
    #[serde(rename = "canvas")]
    Canvas {
        id: String,
        #[serde(rename = "className", alias = "class")]
        class: Option<String>,
        style: Option<String>,
        duration: Option<f64>,
        /// `width`/`height` 属性：2D 画布的像素缓冲尺寸（缺省 300x150，
        /// 与浏览器一致）。页面的 getContext('2d') 面按它开缓冲。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<f32>,
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
            | DomElement::P { id, .. } | DomElement::Span { id, .. }
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

    /// 子节点可变视图（只有容器变体返回 Some）。嵌套 composition 展开时
    /// 用来把子工程根挂进宿主元素。
    pub fn children_mut(&mut self) -> Option<&mut Vec<DomElement>> {
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
            | DomElement::P { duration, .. } | DomElement::Span { duration, .. }
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

    /// HTML 可见窗协议的绝对起点（`data-start`，秒）。timeline 场景的段长
    /// 锚定读它（build_timeline）；窗外隐藏由 resolve 的 clip_window_hides
    /// 按组合全局秒判定。
    pub fn data_start(&self) -> Option<f64> {
        match self {
            DomElement::Div { data_start, .. } | DomElement::Video { data_start, .. } => {
                *data_start
            }
            _ => None,
        }
    }

    pub(crate) fn is_tl(&self) -> bool {
        matches!(self, DomElement::Tl { .. })
    }

    /// 级联匹配的 tag 输入（与 build_node 各分支的标签语义一致）。
    pub fn css_tag(&self) -> &'static str {
        match self {
            DomElement::Div { ua_tag, .. } => match ua_tag.as_deref() {
                Some("h1") => "h1",
                Some("h2") => "h2",
                Some("h3") => "h3",
                Some("h4") => "h4",
                Some("h5") => "h5",
                Some("h6") => "h6",
                Some(other) => css_tag_static(other),
                None => "div",
            },
            DomElement::P { ua_tag, .. } => match ua_tag.as_deref() {
                Some("h1") => "h1",
                Some("h2") => "h2",
                Some("h3") => "h3",
                Some("h4") => "h4",
                Some("h5") => "h5",
                Some("h6") => "h6",
                _ => "p",
            },
            // 行内 UA 标签（b/strong/i/em/u）保留原标签：UA 兜底表的
            // 字重/斜体规则按标签匹配，一律返回 "span" 会让它们失效
            // （figma 语料的 `· 301K` 粗体丢失）。其余标签无 UA 规则，
            // 维持 "span" 兜底。
            DomElement::Span { ua_tag, .. } => match ua_tag.as_deref() {
                Some("b") | Some("strong") | Some("i") | Some("em") | Some("u") => {
                    css_tag_static(ua_tag.as_deref().unwrap_or("span"))
                }
                _ => "span",
            },
            DomElement::Canvas { .. } => "canvas",
            DomElement::Image { .. } => "image",
            DomElement::Video { .. } => "video",
            DomElement::Lottie { .. } => "lottie",
            DomElement::Path { .. } => "path",
            DomElement::Svg { .. } => "svg",
            DomElement::Tl { .. } => "tl",
            DomElement::Audio { .. } => "audio",
            DomElement::Transition { .. } => "transition",
        }
    }

    pub fn css_class(&self) -> Option<&str> {
        match self {
            DomElement::Div { class, .. }
            | DomElement::P { class, .. }
            | DomElement::Span { class, .. }
            | DomElement::Canvas { class, .. }
            | DomElement::Image { class, .. }
            | DomElement::Video { class, .. }
            | DomElement::Lottie { class, .. }
            | DomElement::Path { class, .. }
            | DomElement::Svg { class, .. }
            | DomElement::Tl { class, .. } => class.as_deref(),
            _ => None,
        }
    }

    /// 原文属性表（HTML 前端产；其余变体为空）。属性选择器的匹配输入。
    pub fn html_attrs(&self) -> &[(String, String)] {
        match self {
            DomElement::Div { attrs, .. } => attrs,
            _ => &[],
        }
    }

    /// 内联 `style` 属性原文（可变）。嵌套 composition 展开时改写其中的
    /// 相对 `url()`。
    pub fn inline_style_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            DomElement::Div { style, .. }
            | DomElement::P { style, .. }
            | DomElement::Span { style, .. }
            | DomElement::Canvas { style, .. }
            | DomElement::Image { style, .. }
            | DomElement::Video { style, .. }
            | DomElement::Lottie { style, .. }
            | DomElement::Path { style, .. }
            | DomElement::Svg { style, .. }
            | DomElement::Tl { style, .. } => Some(style),
            DomElement::Transition { .. } | DomElement::Audio { .. } => None,
        }
    }
}

/// HTML 标签 → 静态字符串（tag 选择器输入需要 `&'static str`）。
fn css_tag_static(tag: &str) -> &'static str {
    const TAGS: &[&str] = &[
        "div", "section", "main", "header", "footer", "aside", "nav", "article", "figure",
        "figcaption", "ul", "ol", "li", "table", "thead", "tbody", "tr", "td", "th", "button",
        "form", "label", "a", "blockquote", "hr", "body", "html", "p", "span", "b", "strong",
        "i", "em", "u", "small", "mark", "code",
    ];
    TAGS.iter().find(|t| **t == tag).copied().unwrap_or("div")
}

/// HTML UA 表里默认 `display: inline`（或替换元素 inline-block）的标签。
const INLINE_TAGS: &[&str] = &[
    "span", "a", "b", "strong", "i", "em", "u", "s", "small", "mark", "code", "label", "sub",
    "sup", "abbr", "cite", "q", "time", "kbd", "var", "samp", "button",
];

/// 行内格式化的子项分级（按级联后的 `display`；未声明时取 HTML 标签默认）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum FlowLevel {
    /// 块级盒：所在容器不建立行内流。
    Block,
    /// 行内级：文本 run、行内元素、原子行内盒。
    Inline,
    /// 不在流里（绝对定位、display:none），不影响容器判定。
    OutOfFlow,
}

fn flow_level(el: &DomElement, ctx: &DomContext<'_>) -> FlowLevel {
    if anonymous_text_run(el) {
        return FlowLevel::Inline;
    }
    let none: &[(String, String)] = &[];
    let (id, tag, class, inline_style, attrs) = match el {
        DomElement::Span { id, class, style, ua_tag, .. } => {
            (id.as_str(), ua_tag.as_deref().unwrap_or("span"), class, style, none)
        }
        DomElement::Div { id, class, style, ua_tag, attrs, .. } => (
            id.as_str(),
            ua_tag.as_deref().map(css_tag_static).unwrap_or("div"),
            class,
            style,
            attrs.as_slice(),
        ),
        DomElement::P { id, class, style, ua_tag, .. } => {
            (id.as_str(), ua_tag.as_deref().unwrap_or("p"), class, style, none)
        }
        DomElement::Image { id, class, style, .. } => (id.as_str(), "image", class, style, none),
        DomElement::Svg { id, class, style, .. } => (id.as_str(), "svg", class, style, none),
        DomElement::Canvas { id, class, style, .. } => (id.as_str(), "canvas", class, style, none),
        DomElement::Video { id, class, style, .. } => (id.as_str(), "video", class, style, none),
        _ => return FlowLevel::Block,
    };
    let style = resolve_style_for(id, tag, class.as_deref(), inline_style.as_deref(), attrs, ctx);
    if style.css_layout.as_ref().is_some_and(|css| {
        css.0.display == taffy::prelude::Display::None
            || css.0.position == taffy::prelude::Position::Absolute
    }) {
        return FlowLevel::OutOfFlow;
    }
    // 替换元素（img/svg/canvas/video）默认是原子行内盒。
    let default_inline = INLINE_TAGS.contains(&tag)
        || matches!(
            el,
            DomElement::Image { .. } | DomElement::Svg { .. } | DomElement::Canvas { .. } | DomElement::Video { .. }
        );
    if style.inline_level || (!style.display_block_level && default_inline) {
        FlowLevel::Inline
    } else {
        FlowLevel::Block
    }
}

/// 容器是否建立行内流：在流子项全为行内级，且至少有一个行内元素（纯文本
/// 容器在降级期已成文本节点）。`ctx` 的祖先链须已压入容器本身。
fn establishes_inline_flow(children: &[DomElement], ctx: &mut DomContext<'_>) -> bool {
    // 廉价预筛：没有任何可能行内的元素子项就不必逐个级联。`<br>` 的降级
    // 产物（P + ua_tag）也触发——纯文本 + br 的容器没有任何行内元素，但
    // br 正是行内流要处理的断点（`needs<br>different` 不入行内流就会被
    // 当块级堆叠，行距里多出一个空行盒）。
    let candidate = children.iter().any(|c| {
        matches!(c, DomElement::Span { .. }
            | DomElement::Div { .. }
            | DomElement::Image { .. }
            | DomElement::Svg { .. }
            | DomElement::Canvas { .. }
            | DomElement::Video { .. })
            || line_break(c)
    });
    if !candidate {
        return false;
    }
    let mut has_element = false;
    let mut siblings: Vec<CssNodeInfo> = Vec::new();
    for child in children {
        ctx.prev_siblings = siblings.clone();
        match flow_level(child, ctx) {
            FlowLevel::Block => return false,
            FlowLevel::Inline => has_element |= !anonymous_text_run(child) || line_break(child),
            FlowLevel::OutOfFlow => {}
        }
        siblings.push(CssNodeInfo::of(child));
    }
    has_element
}

/// 匿名文本 run：html 前端把混排里的裸文本包成无标签的合成 P（真 `<p>`
/// 与 h1-h6 都带 ua_tag）。它在行内流里与行内元素同排；`<br>` 同属行内。
fn anonymous_text_run(el: &DomElement) -> bool {
    matches!(el, DomElement::P { ua_tag: None, .. }) || line_break(el)
}

/// `<br>` 的降级产物（html.rs `Lowered::Break`）。
fn line_break(el: &DomElement) -> bool {
    matches!(el, DomElement::P { ua_tag: Some(tag), .. } if tag == "br")
}

/// hyperframes 时间窗协议：计时元素（`data-start`）在窗内由运行时显式置
/// 可见——作者惯用 `.clip { visibility: hidden }` 防首帧闪现（heygen-04），
/// 窗外 resolve 期整子树退出。脚本写入（gsap autoAlpha）照常压过它。
fn timed_clip_visible(style: &mut NodeStyle, ctx: &DomContext<'_>) {
    if ctx.html_frontend && style.clip_start.is_some() {
        style.visibility = Some(crate::style::Visibility::Visible);
    }
}

/// 纯（可折叠）空白的匿名文本 run；`&nbsp;` 不可折叠，不算空白。
fn blank_text_run(el: &DomElement) -> bool {
    matches!(el, DomElement::P { ua_tag: None, text, .. }
        if text.chars().all(|c| c.is_whitespace() && c != '\u{a0}'))
}

/// 匿名文本 run 裁掉行首（`lead`）/行尾（`trail`）的可折叠空白。
fn trim_run(el: &mut DomElement, lead: bool, trail: bool) {
    if let DomElement::P { ua_tag: None, text, .. } = el {
        let ws = |c: char| c.is_whitespace() && c != '\u{a0}';
        let trimmed = match (lead, trail) {
            (true, true) => text.trim_matches(ws),
            (true, false) => text.trim_start_matches(ws),
            (false, true) => text.trim_end_matches(ws),
            (false, false) => text.as_str(),
        };
        if trimmed.len() != text.len() {
            *text = trimmed.to_string();
        }
    }
}

/// 行内流的首尾：两端的空白 run 丢弃，首个 run 裁头、末个 run 裁尾。
fn trim_flow_edges(mut children: Vec<DomElement>) -> Vec<DomElement> {
    while children.first().is_some_and(blank_text_run) {
        children.remove(0);
    }
    while children.last().is_some_and(blank_text_run) {
        children.pop();
    }
    if let Some(first) = children.first_mut() {
        trim_run(first, true, false);
    }
    if let Some(last) = children.last_mut() {
        trim_run(last, false, true);
    }
    children
}

/// 行内流的空白折叠跨过脱流子盒（绝对定位不打断折叠）：行首、行尾、或
/// 前一在流项已以空白结尾的空白 run 丢弃。输入是 (是否空白 run, 节点)。
fn collapse_flow_spaces(children: Vec<(bool, Node)>) -> Vec<(bool, Node)> {
    use crate::parse::node::NodeKind;
    use crate::style::Position;
    let in_flow = |node: &Node| node.style_ref().position != Some(Position::Absolute);
    let mut keep = vec![true; children.len()];
    let mut after_space = true;
    for (slot, (blank, node)) in children.iter().enumerate() {
        if !in_flow(node) {
            continue;
        }
        if *blank {
            keep[slot] = !after_space;
            after_space = true;
        } else {
            after_space = matches!(node.kind(), NodeKind::Text(t) if t.content().ends_with(' '));
        }
    }
    for (slot, (blank, node)) in children.iter().enumerate().rev() {
        if !in_flow(node) {
            continue;
        }
        if !*blank {
            break;
        }
        keep[slot] = false;
    }
    children.into_iter().zip(keep).filter_map(|(child, keep)| keep.then_some(child)).collect()
}

/// 行内流按 `<br>` 分行，每行包成 `display:inline-block` 的匿名行盒；
/// 不足两行返回 `None`（原样保留）。行首行尾的空白 run 按浏览器语义去掉。
fn wrap_lines(id: &str, children: &[DomElement]) -> Option<Vec<DomElement>> {
    let blank = blank_text_run;
    let lines: Vec<&[DomElement]> = children
        .split(line_break)
        .map(|mut seg| {
            while let [first, rest @ ..] = seg
                && blank(first)
            {
                seg = rest;
            }
            while let [rest @ .., last] = seg
                && blank(last)
            {
                seg = rest;
            }
            seg
        })
        .filter(|seg| !seg.is_empty())
        .collect();
    if lines.len() < 2 {
        return None;
    }
    Some(
        lines
            .into_iter()
            .enumerate()
            .map(|(n, seg)| DomElement::Div {
                id: format!("{id}:line{n}"),
                class: None,
                style: Some("display:inline-block".to_string()),
                duration: None,
                data_start: None,
                data_duration: None,
                ua_tag: None,
                attrs: Vec::new(),
                children: seg.to_vec(),
            })
            .collect(),
    )
}

/// 级联匹配上下文链上的节点快照（owned：build_node 消费所有权前提取）。
#[derive(Debug, Clone, Default)]
pub struct CssNodeInfo {
    pub tag: String,
    pub id: String,
    pub classes: String,
    /// 原文属性表（属性选择器用；JSX 通道为空）。
    pub attrs: Vec<(String, String)>,
}

impl CssNodeInfo {
    pub fn of(el: &DomElement) -> Self {
        Self {
            tag: el.css_tag().to_string(),
            id: el.id().to_string(),
            classes: el.css_class().unwrap_or_default().to_string(),
            attrs: el.html_attrs().to_vec(),
        }
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
    /// HTML 前端标记（docs/html-align/DESIGN.md）：根节点叠 html/body
    /// tag 别名（`html, body {}` 规则命中合成根）。
    pub html_frontend: bool,
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
        ancestors: Vec::new(),
        prev_siblings: Vec::new(),
        tag_aliases: doc
            .html_frontend
            .then(|| vec!["body", "html"])
            .unwrap_or_default(),
        html_frontend: doc.html_frontend,
        font_px: Vec::new(),
        vars: Vec::new(),
        in_inline_flow: false,
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
        Some(root) => build_node(root, &mut ctx)?.ok_or_else(|| anyhow::anyhow!("no root element found"))?,
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
        // html 前端的构建上下文快照：运行期 appendChild 的宿主子树重建
        // （resolve 期按 store 的 child_overrides 走 build_node）要与主树
        // 同一套级联输入。jsx 通道结构不可变，None。
        html_build: doc.html_frontend.then(|| {
            std::sync::Arc::new(crate::parse::document::HtmlBuildCtx {
                css: css.map(|engine| std::sync::Arc::new(engine.clone())),
                manifest: manifest.cloned().unwrap_or_default(),
                base_dir: base_dir.map(|p| p.to_path_buf()),
                html_frontend: true,
            })
        }),
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

pub(crate) struct DomContext<'a> {
    pub(crate) base_dir: Option<&'a Path>,
    pub(crate) css: Option<&'a crate::css::CssEngine>,
    pub(crate) manifest: Option<&'a crate::fonts::FontManifest>,
    pub(crate) font_index: Option<&'a crate::fonts::FontFamilyIndex>,
    pub(crate) scripts_by_parent: &'a HashMap<String, Vec<DeclaredScript>>,
    pub(crate) element_ids: HashSet<String>,
    pub(crate) tl_ids: HashSet<String>,
    /// 级联匹配栈（docs/html-align/DESIGN.md）：祖先链（近→远）由 build_node
    /// DFS push/pop；前兄链由父分支的子循环维护。
    pub(crate) ancestors: Vec<CssNodeInfo>,
    pub(crate) prev_siblings: Vec<CssNodeInfo>,
    /// 当前待装配节点的 tag 别名（html 前端根 = body/html），装配后清空。
    pub(crate) tag_aliases: Vec<&'static str>,
    /// HTML 前端（docs/html-align/DESIGN.md）：开启行内格式化标记——
    /// 子节点全为 span 的 div 是行内内容容器，布局端串接重排。
    pub(crate) html_frontend: bool,
    /// 祖先计算字号栈（HTML 前端）：栈顶是父元素字号，`em` 单位按它求值。
    pub(crate) font_px: Vec<f32>,
    /// 祖先计算自定义属性栈：栈顶是父元素的计算表（自定义属性继承）。
    /// `ancestors` 同步 push/pop；`:root` 只命中空祖先链的根，根变量
    /// 靠它送达整棵树。
    pub(crate) vars: Vec<crate::css::CssVars>,
    /// 父元素建立了行内流（子文本节点是行内 run）。
    pub(crate) in_inline_flow: bool,
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
                id: audio_id,
                mut attach,
                path,
                url,
                duration,
            } => {
                if attach.is_empty() {
                    // 根下直挂的 <audio>（html 顶层 body 级 clip）挂根自身。
                    attach = parent_id.unwrap_or(&id).to_string();
                }
                out.push(DomAudio {
                    id: audio_id,
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
    attrs: &[(String, String)],
    ctx: &DomContext<'_>,
) -> NodeStyle {
    resolve_style_vars_for(el_id, tag, class, inline_style, attrs, ctx).0
}

/// HTML 文本叶按 `white-space` 折行（初始值 `normal`）。盒宽归布局算法：
/// flex 子项的 min/max-content 贡献与 `max-width` 钳制都由 taffy 给出。
/// JSX 前端沿用「标题默认单行」（见 `NodeStyle::wrap_text`）。
fn wrap_html_text(style: &mut NodeStyle, ctx: &DomContext<'_>) {
    if ctx.html_frontend && !style.no_wrap {
        style.wrap_text = true;
    }
}

/// 同 [`resolve_style_for`]，并带回元素的计算自定义属性表（容器元素压进
/// 变量栈，供子元素继承）。
fn resolve_style_vars_for(
    el_id: &str,
    tag: &str,
    class: Option<&str>,
    inline_style: Option<&str>,
    attrs: &[(String, String)],
    ctx: &DomContext<'_>,
) -> (NodeStyle, crate::css::CssVars) {
    let aliases = ctx.tag_aliases.clone();
    // 栈是远→近（根在最底），级联匹配要近→远（[0] = 父）。
    let ancestor_views: Vec<crate::css::SelectorNode<'_>> = ctx
        .ancestors
        .iter()
        .rev()
        .map(|n| crate::css::SelectorNode {
            tag: &n.tag,
            id: &n.id,
            classes: &n.classes,
            attrs: &n.attrs,
        })
        .collect();
    let sibling_views: Vec<crate::css::SelectorNode<'_>> = ctx
        .prev_siblings
        .iter()
        .rev()
        .map(|n| crate::css::SelectorNode {
            tag: &n.tag,
            id: &n.id,
            classes: &n.classes,
            attrs: &n.attrs,
        })
        .collect();
    let sel_ctx = crate::css::SelectorContext {
        ancestors: &ancestor_views,
        prev_siblings: &sibling_views,
        tag_aliases: &aliases,
        subject_attrs: attrs,
        parent_font_px: parent_font_px(ctx),
        inherited_vars: ctx.vars.last(),
        tree_root: ctx.vars.is_empty(),
    };
    let (mut style, vars) = crate::parse::elements::resolve_style_ctx(
        ctx.css,
        class.unwrap_or(""),
        inline_style,
        el_id,
        tag,
        &sel_ctx,
    );
    // 字体引用（`font-[faceId]` / 文档默认字体）在进 Node 前解析到 family。
    if let (Some(manifest), Some(index)) = (ctx.manifest, ctx.font_index)
        && !manifest.is_empty()
    {
        manifest.apply_font_refs_to_style(index, &mut style);
    }
    (style, vars)
}

/// 父元素计算字号（HTML 前端）：根元素的父字号是浏览器默认 16px。
fn parent_font_px(ctx: &DomContext<'_>) -> Option<f32> {
    ctx.html_frontend
        .then(|| ctx.font_px.last().copied().unwrap_or(16.0))
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
pub(crate) fn build_node(el: DomElement, ctx: &mut DomContext<'_>) -> anyhow::Result<Option<Node>> {
    match el {
        DomElement::Div {
            id,
            class,
            style: inline_style,
            duration: _,
            data_start,
            data_duration,
            ua_tag,
            attrs,
            children,
        } => {
            // HTML 前端的根节点就是 <body>（<html> 无独立节点），
            // 组合器选择器 `body .x` 要靠祖先链上的真标签命中。
            let is_html_root = ctx.html_frontend && ctx.ancestors.is_empty();
            let tag = if is_html_root {
                "body"
            } else {
                ua_tag.as_deref().map(css_tag_static).unwrap_or("div")
            };
            let (mut style, vars) =
                resolve_style_vars_for(&id, tag, class.as_deref(), inline_style.as_deref(), &attrs, ctx);
            // CSS 画布传播（css-backgrounds 文档根语义）：html/body 规则的
            // 背景传播到画布，与根元素自身声明的背景无关。tag 别名把
            // `html,body{…}` 压进根元素级联后，作者 id/inline 声明（如
            // `#root{background:transparent}`）会按 specificity 压掉传播色，
            // 页面失去底色（body 无独立节点，传播只能落根盒）。作者通道
            // 声明背景时，把 tag 通道的纯色追回为最底层（resolve 端层序：
            // 末层最远先画）。渐变画布/`html{A} body{B}` 分写形态语料未涉及。
            if is_html_root {
                // 作者通道：tag 换 "div" 且临时摘掉别名，html/body 裸规则不命中。
                let aliases = std::mem::take(&mut ctx.tag_aliases);
                let author = resolve_style_vars_for(&id, "div", class.as_deref(), inline_style.as_deref(), &attrs, ctx).0;
                if author.bg_color.is_some() || !author.background_layers.is_empty() {
                    let tag_only = resolve_style_vars_for("", "body", None, None, &[], ctx).0;
                    style.bg_canvas_color = tag_only.bg_color;
                }
                ctx.tag_aliases = aliases;
            }
            // tag 别名只作用于根节点（html 前端根 = body/html）。
            ctx.tag_aliases.clear();
            // HTML 可见窗协议属性（docs/html-align/DESIGN.md）。
            style.clip_start = data_start;
            style.clip_duration = data_duration;
            timed_clip_visible(&mut style, ctx);
            attach_driver(&mut style, &id, ctx)?;
            // 子元素的 em 基准：本元素字号（未声明则继承父字号）。
            let own_font_px = parent_font_px(ctx).map(|parent| style.text_px.unwrap_or(parent));
            let mut self_info = CssNodeInfo::of(&DomElement::Div {
                id: id.clone(),
                class: class.clone(),
                style: inline_style.clone(),
                duration: None,
                data_start: None,
                data_duration: None,
                ua_tag: ua_tag.clone(),
                attrs: attrs.clone(),
                children: Vec::new(),
            });
            if is_html_root {
                self_info.tag = tag.to_string();
            }
            ctx.ancestors.push(self_info);
            ctx.vars.push(vars);
            if let Some(px) = own_font_px {
                ctx.font_px.push(px);
            }
            let saved_siblings = std::mem::take(&mut ctx.prev_siblings);
            // 行内格式化：HTML 前端的块级盒，在流子项全为行内级（文本 run、
            // 行内元素、原子行内盒）且至少有一个行内元素时建立行内流（布局
            // 端折行重排，taffy 无 IFC）。纯文本容器走 Text 节点路径；flex/grid
            // 容器的子节点是 flex/grid 项，不进行内流。
            let flex_or_grid = style.css_layout.as_ref().is_some_and(|css| {
                matches!(
                    css.0.display,
                    taffy::prelude::Display::Flex | taffy::prelude::Display::Grid
                )
            });
            style.inline_flow =
                ctx.html_frontend && !flex_or_grid && establishes_inline_flow(&children, ctx);
            // `<br>` 切开的行内流：每行包成一个匿名行盒（inline-block 行内流
            // 容器）在本元素里竖排。行高由 taffy 原生计入（父盒按真实行数
            // 居中），各行再按 text-align 定位（resolve 期 shrink-to-fit +
            // auto 边距）。行内 pass 只会单行串接。
            let children = match style.inline_flow.then(|| wrap_lines(&id, &children)).flatten() {
                Some(lines) => {
                    style.inline_flow = false;
                    lines
                }
                None => children,
            };
            // 行内兄弟之间的纯空白 run（html.rs 折成的单空格）只在行内流里
            // 占位；块/flex/grid 子项之间的空白不产盒。行首行尾的可折叠空白
            // 按 CSS 删除（行内流只有首尾两端，其余容器每个 run 自成一行）。
            let children = if style.inline_flow {
                trim_flow_edges(children)
            } else {
                children
                    .into_iter()
                    .filter(|child| !blank_text_run(child))
                    .map(|mut child| {
                        trim_run(&mut child, true, true);
                        child
                    })
                    .collect()
            };
            let mut div_node = div();
            div_node.style = style;
            let saved_flow = std::mem::replace(&mut ctx.in_inline_flow, div_node.style.inline_flow);
            let mut sibling_infos: Vec<CssNodeInfo> = Vec::new();
            let mut built = Vec::with_capacity(children.len());
            for child in children {
                ctx.prev_siblings = sibling_infos.clone();
                let child_info = CssNodeInfo::of(&child);
                let blank = blank_text_run(&child);
                if let Some(node) = build_node(child, ctx)? {
                    built.push((blank, node));
                }
                sibling_infos.push(child_info);
            }
            if div_node.style.inline_flow {
                built = collapse_flow_spaces(built);
            }
            for (_, node) in built {
                div_node = div_node.child(node);
            }
            ctx.in_inline_flow = saved_flow;
            ctx.prev_siblings = saved_siblings;
            if own_font_px.is_some() {
                ctx.font_px.pop();
            }
            ctx.vars.pop();
            ctx.ancestors.pop();
            Ok(Some(Node::new(div_node)))
        }
        // HTML 语义的文本标签：p=块级段落、span=行内片段（批次②起 p/span
        // 共用同一文本节点机制，p 的 UA margin 取 0 保版式零漂移；真标签
        // 名进 CSS 层叠的 tag 选择器输入——UA 兜底表按它命中字形默认）。
        DomElement::P {
            id,
            class,
            style: inline_style,
            text: content,
            ua_tag,
            ..
        } => {
            let mut style = resolve_style_for(
                &id,
                ua_tag.as_deref().unwrap_or("p"),
                class.as_deref(),
                inline_style.as_deref(),
                &[],
                ctx,
            );
            style.inline_run = ctx.in_inline_flow;
            wrap_html_text(&mut style, ctx);
            attach_driver(&mut style, &id, ctx)?;
            let mut text_node = text(content);
            text_node.style = style;
            Ok(Some(Node::new(text_node)))
        }
        DomElement::Span {
            id,
            class,
            style: inline_style,
            text: content,
            ua_tag,
            ..
        } => {
            let mut style = resolve_style_for(
                &id,
                ua_tag.as_deref().unwrap_or("span"),
                class.as_deref(),
                inline_style.as_deref(),
                &[],
                ctx,
            );
            style.inline_run = ctx.in_inline_flow;
            wrap_html_text(&mut style, ctx);
            attach_driver(&mut style, &id, ctx)?;
            let mut text_node = span(content);
            text_node.style = style;
            Ok(Some(Node::new(text_node)))
        }
        DomElement::Canvas {
            id,
            class,
            style: inline_style,
            children,
            width,
            height,
            ..
        } => {
            let mut style =
                resolve_style_for(&id, "canvas", class.as_deref(), inline_style.as_deref(), &[], ctx);
            attach_driver(&mut style, &id, ctx)?;
            // canvas 的 width/height 属性是固有尺寸（CSS 未给尺寸时生效），
            // 与浏览器一致：属性只提供默认盒，CSS 的 width/height 优先。
            if width.is_some() || height.is_some() {
                if let Some(css) = &mut style.css_layout {
                    if css.0.size.width.is_auto() {
                        if let Some(w) = width {
                            css.0.size.width = taffy::prelude::Dimension::length(w);
                        }
                    }
                    if css.0.size.height.is_auto() {
                        if let Some(h) = height {
                            css.0.size.height = taffy::prelude::Dimension::length(h);
                        }
                    }
                }
                if style.width.is_none() {
                    style.width = width;
                }
                if style.height.is_none() {
                    style.height = height;
                }
            }
            let mut canvas_node = canvas();
            canvas_node.style = style;
            // 位图尺寸 = width/height 属性（缺一侧按 HTML 默认 300×150）：
            // 绘制坐标在位图空间，CSS 盒另给尺寸时整体缩放铺满（inspector
            // 1600×1220 位图 → 4320×3300 盒）。
            if width.is_some() || height.is_some() {
                canvas_node.bitmap = Some((width.unwrap_or(300.0), height.unwrap_or(150.0)));
            }
            // Hidden picture subtree: children rasterize offscreen and are only
            // reachable through `canvas.getSubTree().makeShader(...)`.
            for child in children {
                if let Some(node) = build_node(child, ctx)? {
                    canvas_node = canvas_node.hidden_child(node);
                }
            }
            Ok(Some(Node::new(canvas_node)))
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
                resolve_style_for(&id, "image", class.as_deref(), inline_style.as_deref(), &[], ctx);
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
            Ok(Some(Node::new(image_node)))
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
                resolve_style_for(&id, "video", class.as_deref(), inline_style.as_deref(), &[], ctx);
            // 计时媒体同是 clip：窗外隐藏（timeline 的 act4 根级视频 21.0–26.7s
            // 窗外仍盖在 act5 之上）。
            if ctx.html_frontend {
                style.clip_start = data_start;
                style.clip_duration = data_duration;
                timed_clip_visible(&mut style, ctx);
            }
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
                if let Some(node) = build_node(child, ctx)? {
                    video_node = video_node.child(node);
                }
            }
            Ok(Some(Node::new(video_node)))
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
                resolve_style_for(&id, "div", class.as_deref(), inline_style.as_deref(), &[], ctx);
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
            Ok(Some(Node::new(lottie_node)))
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
                resolve_style_for(&id, "path", class.as_deref(), inline_style.as_deref(), &[], ctx);
            attach_driver(&mut style, &id, ctx)?;
            let view_box = view_box.as_deref().map(parse_view_box_attr).transpose()?;
            let mut path_node = path(d);
            if let Some(vb) = view_box {
                path_node.set_view_box(vb);
            }
            path_node.style = style;
            Ok(Some(Node::new(path_node)))
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
            // 选择器按 svg 标签匹配（`#cursor svg { width: 100% }` 这类后代
            // 类型选择器）；按 div 匹配会让 svg 的 CSS 尺寸整条落空。
            let mut style =
                resolve_style_for(&id, "svg", class.as_deref(), inline_style.as_deref(), &[], ctx);
            attach_driver(&mut style, &id, ctx)?;
            // 内联 markup 不落盘、无文件语义：src 仅在文件模式参与 base_dir 解析。
            let resolved_src = if inline.is_some() {
                PathBuf::new()
            } else if let Some(base_dir) = ctx.base_dir {
                base_dir.join(&src)
            } else {
                PathBuf::from(src)
            };
            // svg 内部元素的绘制注入现场：inline markup 不携带页面层叠，
            // `.pchart .pfill{fill:…}` 这类瞄准内部 path 的规则要靠注入。
            // 注入本体在 resolve 期执行（脚本 markup 覆盖之后），这里只把
            // 样式表与祖先链（远→近，翻转成匹配要的近→远）挂上节点。
            let inner_paint = match (inline.as_ref(), ctx.css) {
                (Some(_), Some(engine)) => Some(std::sync::Arc::new(
                    crate::parse::html::SvgInnerPaintCtx {
                        sheet: engine.sheet_arc(),
                        host_ancestors: std::sync::Arc::new(
                            ctx.ancestors
                                .iter()
                                .rev()
                                .map(|n| {
                                    (n.tag.clone(), n.id.clone(), n.classes.clone(), n.attrs.clone())
                                })
                                .collect(),
                        ),
                    },
                )),
                _ => None,
            };
            let mut svg_node = crate::parse::primitives::svg(resolved_src);
            svg_node.inline = inline;
            svg_node.width = width;
            svg_node.height = height;
            svg_node.preserve_aspect_ratio = preserve_aspect_ratio;
            svg_node.inner_paint = inner_paint;
            svg_node.style = style;
            Ok(Some(Node::new(svg_node)))
        }
        DomElement::Tl {
            id,
            class,
            style: inline_style,
            children,
            ..
        } => {
            let mut style = resolve_style_for(&id, "div", class.as_deref(), inline_style.as_deref(), &[], ctx);
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
            //
            // data-start 锚定：相邻场景都声明绝对起点时，段长取「下一起点 −
            // 游标」的差，窗间重叠量插 cut 段（重叠期间两窗都活跃，入场景
            // 整画盖住出场景）。纯窗长平铺会把后窗整体推迟重叠量（frame-md
            // scene-07/08 的 tween 相位滞后 0.64s/0.35s）；hyperframes 排布
            // 的总长 = Σ窗长 − Σ重叠（60.254 = 65.404 − 5.15），同样只有差
            // 分锚定能对上。尾段窗长扣掉前一个过渡的占用；场景窗长经
            // scene_window 的 before/after 拓宽精确恢复声明值，声明窗外
            // （gap、尾段）由 clip_window_hides 隐藏。
            let windows: Vec<(Option<f64>, Option<f64>)> = scenes
                .iter()
                .map(|scene| (scene.data_start(), scene.duration()))
                .collect();
            let mut tl_builder = timeline();
            let mut cursor_secs = 0.0;
            let mut before_secs = 0.0;
            for (index, scene) in scenes.into_iter().enumerate() {
                let duration = windows[index].1.ok_or_else(|| {
                    anyhow::anyhow!(
                        "timeline sequence `{}` is missing a duration",
                        child_ids[index]
                    )
                })?;
                let next_start = windows.get(index + 1).and_then(|(start, _)| *start);
                let (seg_len, overlap) =
                    match (windows[index].0, next_start) {
                        // 两端都声明起点：段长差分锚定，窗间重叠量留给 cut。
                        (Some(cur), Some(next)) if next > cur => {
                            (next - cursor_secs, Some((cur + duration - next).max(0.0)))
                        }
                        // 锚定链的尾段：窗长扣掉前一个过渡占的时间。
                        (Some(_), None) => (duration - before_secs, None),
                        // 未声明起点的场景保持窗长平铺（原行为）。
                        _ => (duration, None),
                    };
                let Some(node) = build_node(scene, ctx)? else {
                    continue;
                };
                tl_builder = tl_builder.sequence(seg_len, node);
                cursor_secs += seg_len;

                let Some(next_id) = child_ids.get(index + 1) else {
                    continue;
                };
                let key = (child_ids[index].clone(), next_id.clone());
                if let Some(transition) = transitions_by_pair.get(&key) {
                    tl_builder = tl_builder.transition(build_transition(transition)?);
                    cursor_secs += transition.duration().unwrap_or(0.0);
                    before_secs = transition.duration().unwrap_or(0.0);
                } else if let Some(overlap) = overlap.filter(|seconds| *seconds > 1e-9) {
                    tl_builder = tl_builder.transition(cut().timing(Easing::Linear, overlap));
                    cursor_secs += overlap;
                    before_secs = overlap;
                }
            }

            let timeline_node: Node = tl_builder.into();
            let mut kind = timeline_node.kind().clone();
            *kind.style_mut() = style;
            Ok(Some(Node::new(kind)))
        }
        DomElement::Transition { .. } => anyhow::bail!(
            "transition element must be a direct child of a `tl` node"
        ),
        // audio 与 video 同类容错：正常路径被 harvest 收进音频池，但
        // seed 物化的 face 树把 audio 带进上报子树（运行期 appendChild
        // 重建）时会到达这里。audio 不渲染（解码走音频池），跳过不产
        // 节点；bail 会让整帧渲染炸掉。
        DomElement::Audio { .. } => Ok(None),
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
