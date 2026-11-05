//! HTML 前端（docs/html-align/DESIGN.md）：html5ever 解析 hyperframes
//! .html 工程，降级成 DomElement 种子 + 侧通道（css/scripts/links/meta）。
//!
//! 与 JSX 前端的分工：JSX 的树由 JS 物化，这里由 Rust 解析出「种子」——
//! 同一份 DomElement serde 形的 JSON，交给 JS realm 物化成带 DOM API 的
//! 元素面，页面 `<script>` 在其上动态建 DOM，最终树仍从 JS 提取回
//! `from_extract`。本模块只负责把静态 HTML 文本变成那颗种子。
//!
//! 降级规则（批次 A 语料近似，详见 DESIGN.md §文本降级）：
//! - 仅文本子节点 → 该元素直接承载 text（P/Span 按 UA 标签映射）；
//! - 混合内容 → 文本 run 包成合成 P（连续 run 合并）；
//! - 空白按 CSS 折叠语义压成单空格（`<pre>` 与 pre 类内联样式例外）。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use anyhow::Result;
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{
    ElementFlags, ElemName, NextParserState, NodeOrText, QuirksMode, TreeSink,
};
use html5ever::{Attribute, LocalName, Namespace, ParseOpts, QualName, parse_document};
use serde_json::{Value, json};

use crate::parse::dom::DomElement;

/// 页面 `<script>`：外链（src 相对工程根或远程 URL）或内联文本。
#[derive(Debug, Clone)]
pub struct HtmlScript {
    pub src: Option<String>,
    pub inline: Option<String>,
}

/// `<link>` 侧通道：stylesheet 的 href 交网络层，其余忽略。
#[derive(Debug, Clone)]
pub struct HtmlLink {
    pub rel: String,
    pub href: String,
}

/// `data-composition-src` 宿主：host 元素 id → 子工程文件路径。
#[derive(Debug, Clone)]
pub struct CompositionHost {
    pub id: String,
    pub src: String,
}

/// 降级期记录的元素协议属性（data-composition-* / 尺寸 / 时长）。DomElement
/// 不保留这些 data-*，按 id 建副索引供根选择与嵌套拼接。
#[derive(Debug, Default, Clone)]
pub struct ElementProto {
    pub composition_id: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub duration: Option<f64>,
    pub fps: Option<f64>,
    pub src: Option<String>,
}

/// 一份 .html 的解析产物。
#[derive(Debug, Clone, Default)]
pub struct HtmlDocument {
    /// `{width,height,fps,duration}`（data-width/height/fps/duration，含默认）。
    pub meta: Value,
    pub root: Option<DomElement>,
    /// `<style>` 文本按文档序拼接（head 与 body 内联均收）。
    pub css_text: String,
    pub scripts: Vec<HtmlScript>,
    pub links: Vec<HtmlLink>,
    pub composition_hosts: Vec<CompositionHost>,
    /// 元素 id → 协议属性（含根与嵌套宿主）。
    pub protos: HashMap<String, ElementProto>,
}

// ── sink 层：html5ever TreeSink 的最小 Rc 树 ─────────────────────────

#[derive(Debug)]
enum SinkData {
    Document,
    Text(String),
    Comment,
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        /// `<template>`：内容根单独持有（get_template_contents 语义）。
        template_contents: Option<Handle>,
    },
}

#[derive(Debug)]
struct SinkNode {
    data: SinkData,
    children: Vec<Handle>,
    parent: Option<std::rc::Weak<RefCell<SinkNode>>>,
}

type Handle = Rc<RefCell<SinkNode>>;

/// [`TreeSink::ElemName`] 的 owned 载体：RefCell 借用跨不出调用帧，这里
/// 按值克隆 (ns, local)（Atom 克隆是引用计数）。
#[derive(Debug)]
struct OwnedElemName {
    ns: Namespace,
    local: LocalName,
}

impl ElemName for OwnedElemName {
    fn ns(&self) -> &Namespace {
        &self.ns
    }
    fn local_name(&self) -> &LocalName {
        &self.local
    }
}

fn sink_element(name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
    Rc::new(RefCell::new(SinkNode {
        data: SinkData::Element {
            template_contents: flags.template.then(|| {
                Rc::new(RefCell::new(SinkNode {
                    data: SinkData::Document,
                    children: Vec::new(),
                    parent: None,
                }))
            }),
            name,
            attrs,
        },
        children: Vec::new(),
        parent: None,
    }))
}

struct OpenCtHtmlSink {
    document: Handle,
}

impl TreeSink for OpenCtHtmlSink {
    type Handle = Handle;
    type Output = Handle;
    type ElemName<'a> = OwnedElemName;

    fn finish(self) -> Handle {
        self.document
    }

    fn parse_error(&self, msg: std::borrow::Cow<'static, str>) {
        log::debug!("html: 解析器恢复性报错（按规范容错继续）：{msg}");
    }

    fn get_document(&self) -> Handle {
        self.document.clone()
    }

    fn elem_name<'a>(&'a self, target: &'a Handle) -> OwnedElemName {
        let node = target.borrow();
        match &node.data {
            SinkData::Element { name, .. } => OwnedElemName {
                ns: name.ns.clone(),
                local: name.local.clone(),
            },
            _ => OwnedElemName {
                ns: Namespace::from("http://www.w3.org/1999/xhtml"),
                local: LocalName::from(""),
            },
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        sink_element(name, attrs, flags)
    }

    fn create_comment(&self, _text: StrTendril) -> Handle {
        Rc::new(RefCell::new(SinkNode {
            data: SinkData::Comment,
            children: Vec::new(),
            parent: None,
        }))
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> Handle {
        self.create_comment("".into())
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        match child {
            NodeOrText::AppendNode(node) => {
                node.borrow_mut().parent = Some(Rc::downgrade(parent));
                parent.borrow_mut().children.push(node);
            }
            NodeOrText::AppendText(text) => {
                append_text(parent, &text);
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev_element: &Handle,
        child: NodeOrText<Handle>,
    ) {
        // foster parenting 通路：element 有父则插它前面，否则落 prev_element。
        let has_parent = element
            .borrow()
            .parent
            .as_ref()
            .and_then(std::rc::Weak::upgrade)
            .is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
    }

    fn get_template_contents(&self, target: &Handle) -> Handle {
        let node = target.borrow();
        match &node.data {
            SinkData::Element {
                template_contents: Some(frag),
                ..
            } => frag.clone(),
            _ => target.clone(),
        }
    }

    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        Rc::ptr_eq(x, y)
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, new_node: NodeOrText<Handle>) {
        let parent = sibling
            .borrow()
            .parent
            .as_ref()
            .and_then(std::rc::Weak::upgrade);
        let Some(parent) = parent else {
            // 兄弟无父（document 顶层的稀少路径）：按 append 到 document。
            self.append(&self.document.clone(), new_node);
            return;
        };
        let index = parent
            .borrow()
            .children
            .iter()
            .position(|c| Rc::ptr_eq(c, sibling));
        let Some(index) = index else {
            self.append(&parent, new_node);
            return;
        };
        match new_node {
            NodeOrText::AppendNode(node) => {
                node.borrow_mut().parent = Some(Rc::downgrade(&parent));
                parent.borrow_mut().children.insert(index, node);
            }
            NodeOrText::AppendText(text) => {
                // 与 append 同一合并语义：前一个兄弟是文本则续写。
                let merged = index > 0 && {
                    let mut guard = parent.borrow_mut();
                    match guard.children.get_mut(index - 1) {
                        Some(prev) => {
                            let mut prev_ref = prev.borrow_mut();
                            match &mut prev_ref.data {
                                SinkData::Text(slot) => {
                                    slot.push_str(&text);
                                    true
                                }
                                _ => false,
                            }
                        }
                        None => false,
                    }
                };
                if !merged {
                    let node = Rc::new(RefCell::new(SinkNode {
                        data: SinkData::Text(text.to_string()),
                        children: Vec::new(),
                        parent: Some(Rc::downgrade(&parent)),
                    }));
                    parent.borrow_mut().children.insert(index, node);
                }
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        let mut node = target.borrow_mut();
        let SinkData::Element { attrs: slot, .. } = &mut node.data else {
            return;
        };
        for attr in attrs {
            let key = attr.name.local.clone();
            if slot.iter().any(|a| a.name.local == key) {
                continue;
            }
            slot.push(attr);
        }
    }

    fn remove_from_parent(&self, target: &Handle) {
        let parent = target
            .borrow()
            .parent
            .as_ref()
            .and_then(std::rc::Weak::upgrade);
        if let Some(parent) = parent {
            parent
                .borrow_mut()
                .children
                .retain(|c| !Rc::ptr_eq(c, target));
            target.borrow_mut().parent = None;
        }
    }

    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        let kids = std::mem::take(&mut node.borrow_mut().children);
        for kid in kids {
            kid.borrow_mut().parent = Some(Rc::downgrade(new_parent));
            new_parent.borrow_mut().children.push(kid);
        }
    }

    fn complete_script(&self, _node: &Handle) -> NextParserState {
        NextParserState::Continue
    }
}

fn append_text(parent: &Handle, text: &str) {
    let merged = {
        let mut guard = parent.borrow_mut();
        match guard.children.last_mut() {
            Some(last) => {
                let mut last_ref = last.borrow_mut();
                match &mut last_ref.data {
                    SinkData::Text(slot) => {
                        slot.push_str(text);
                        true
                    }
                    _ => false,
                }
            }
            None => false,
        }
    };
    if !merged {
        let node = Rc::new(RefCell::new(SinkNode {
            data: SinkData::Text(text.to_string()),
            children: Vec::new(),
            parent: Some(Rc::downgrade(parent)),
        }));
        parent.borrow_mut().children.push(node);
    }
}

// ── 降级层：sink 树 → DomElement 种子 ────────────────────────────────

/// 属性便捷视图（重复属性取首个，与浏览器一致）。
struct Attrs<'a> {
    items: &'a [Attribute],
}

impl Attrs<'_> {
    fn get(&self, name: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|a| &*a.name.local == name)
            .map(|a| &*a.value)
    }
    fn f64(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(|v| v.trim().parse().ok())
    }
}

fn is_pre_tag(tag: &str) -> bool {
    matches!(tag, "pre" | "listing" | "textarea" | "plaintext")
}

fn has_pre_inline_style(style: Option<&str>) -> bool {
    style.is_some_and(|s| {
        let lower = s.to_ascii_lowercase();
        lower.split(';').any(|decl| {
            let Some((prop, value)) = decl.split_once(':') else {
                return false;
            };
            prop.trim() == "white-space"
                && value.trim().trim_start_matches('-').starts_with("pre")
        })
    })
}

/// CSS 折叠语义近似：连续空白序列 → 单空格；纯空白输入产出空串。
/// `keep`（pre 语境）原样保留。
fn collapse_ws(text: &str, keep: bool) -> String {
    if keep {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut pending_ws = false;
    for ch in text.chars() {
        // U+00A0（&nbsp;）是不可折叠空格：浏览器把它渲染成一个空格字形，
        // k3 的逐字 span 正用它占位，不能当空白折叠掉。
        if ch.is_whitespace() && ch != '\u{a0}' {
            pending_ws = !out.is_empty();
            continue;
        }
        if pending_ws {
            out.push(' ');
            pending_ws = false;
        }
        out.push(ch);
    }
    out
}

/// 降级过程中的子项。
enum Lowered {
    Element(DomElement),
    /// 文本 run（原始空白保留，折叠/裁剪在组装时按语境做）。
    TextRun(String),
    /// `<br>` 硬断点。
    Break,
}

const SKIP_TAGS: &[&str] = &["meta", "title", "head", "base", "noscript", "template"];
const CONTAINER_TAGS: &[&str] = &[
    "div", "section", "main", "header", "footer", "aside", "nav", "article", "figure",
    "figcaption", "ul", "ol", "li", "table", "thead", "tbody", "tr", "td", "th",
    "button", "form", "label", "a", "blockquote", "hr", "body", "html",
];

/// 文本承载标签 → CSS 层叠的 ua_tag。
fn text_bearing_tag(tag: &str) -> Option<&'static str> {
    const TAGS: &[&str] = &[
        "p", "h1", "h2", "h3", "h4", "h5", "h6", "span", "b", "strong", "i", "em", "u",
        "small", "mark", "code",
    ];
    TAGS.iter().find(|t| **t == tag).copied()
}

fn block_level_tag(tag: &str) -> bool {
    matches!(tag, "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

fn is_http_url(src: &str) -> bool {
    src.starts_with("http://")
        || src.starts_with("https://")
        || src.starts_with("data:")
        || src.starts_with("blob:")
}

struct LoweringState {
    synthetic: usize,
    used_ids: HashSet<String>,
    hosts: Vec<CompositionHost>,
    protos: HashMap<String, ElementProto>,
    css_chunks: Vec<String>,
    scripts: Vec<HtmlScript>,
    links: Vec<HtmlLink>,
}

impl LoweringState {
    fn assign_id(&mut self, authored: Option<&str>) -> String {
        if let Some(id) = authored.map(str::trim).filter(|s| !s.is_empty())
            && self.used_ids.insert(id.to_string())
        {
            return id.to_string();
        }
        loop {
            self.synthetic += 1;
            let id = format!("hf-{}", self.synthetic);
            if self.used_ids.insert(id.clone()) {
                return id;
            }
        }
    }

    fn record_proto(&mut self, id: &str, view: &Attrs<'_>) {
        let composition_id = view
            .get("data-composition-id")
            .map(str::to_string)
            .filter(|s| !s.is_empty());
        let src = view
            .get("data-composition-src")
            .map(str::to_string)
            .filter(|s| !s.is_empty());
        let proto = ElementProto {
            composition_id,
            width: view.f64("data-width"),
            height: view.f64("data-height"),
            duration: view.f64("data-duration"),
            fps: view.f64("data-fps"),
            src: src.clone(),
        };
        if let Some(src) = src {
            self.hosts.push(CompositionHost {
                id: id.to_string(),
                src,
            });
        }
        self.protos.insert(id.to_string(), proto);
    }
}

/// 解析整份文档。`fps_default` 是 data-fps 缺席时的合成帧率。
pub fn parse_html(input: &str, fps_default: i32) -> Result<HtmlDocument> {
    let sink = OpenCtHtmlSink {
        document: Rc::new(RefCell::new(SinkNode {
            data: SinkData::Document,
            children: Vec::new(),
            parent: None,
        })),
    };
    let document = parse_document(sink, ParseOpts::default()).one(input);

    let mut state = LoweringState {
        synthetic: 0,
        used_ids: Default::default(),
        hosts: Vec::new(),
        protos: HashMap::new(),
        css_chunks: Vec::new(),
        scripts: Vec::new(),
        links: Vec::new(),
    };

    // document → html → (head, body)。head 走同一 lower_element 通路摘取
    // 侧通道（style/script/link），meta/title 经 SKIP_TAGS 丢弃。
    let doc_children = document.borrow().children.clone();
    let html = doc_children
        .iter()
        .find(|c| tag_of(c).as_deref() == Some("html"));
    let html_kids = html
        .map(|h| h.borrow().children.clone())
        .unwrap_or_else(|| doc_children.clone());

    let mut top: Vec<DomElement> = Vec::new();
    for kid in &html_kids {
        match tag_of(kid).as_deref() {
            Some("head") => {
                let kids = kid.borrow().children.clone();
                for k in kids {
                    lower_element(&k, &mut state)?;
                }
            }
            Some("body") => {
                let items = lower_children(kid, &mut state, false)?;
                top.extend(assemble_children(items, &mut state, false, false));
            }
            _ => {
                lower_element(kid, &mut state)?;
            }
        }
    }

    // 根选择：顶层首个带 data-composition-id 的元素；否则合成根包裹全部。
    let root_id = top.iter().find_map(|el| {
        state
            .protos
            .get(el.id())
            .filter(|p| p.composition_id.is_some())
            .map(|_| el.id().to_string())
    });

    let (root, meta) = match root_id {
        Some(id) => {
            let mut root = top
                .iter()
                .find(|el| el.id() == id)
                .cloned()
                .expect("root id 来自同一列表");
            let proto = state.protos.get(&id).cloned().unwrap_or_default();
            // 画布尺寸兜底：data-width/height 烙进根内联样式（浏览器语义里
            // data-* 无样式效果，hyperframes 靠工程 CSS 定根尺寸；这里让根
            // 盒几何独立于工程 CSS 是否完备）。position:relative 让绝对定位
            // 子树锚定根盒（浏览器里锚定视口，几何等价：根在 0,0）。
            if let DomElement::Div {
                style: inline_style,
                ..
            } = &mut root
            {
                if let (Some(w), Some(h)) = (proto.width, proto.height) {
                    let sizing =
                        format!("width:{}px;height:{}px;position:relative", w, h);
                    *inline_style = Some(match inline_style.take() {
                        Some(existing) => format!("{existing};{sizing}"),
                        None => sizing,
                    });
                }
            }
            let meta = json!({
                "width": proto.width.unwrap_or(1920.0) as i64,
                "height": proto.height.unwrap_or(1080.0) as i64,
                "fps": proto.fps.unwrap_or(fps_default as f64) as i64,
                "duration": proto.duration.unwrap_or(3.0),
            });
            (Some(root), meta)
        }
        None => {
            let root = DomElement::Div {
                id: "hf-root".to_string(),
                class: None,
                style: None,
                duration: None,
                data_start: None,
                data_duration: None,
                ua_tag: None,
                children: top,
            };
            let meta = json!({
                "width": 1920,
                "height": 1080,
                "fps": fps_default,
                "duration": 3.0,
            });
            (Some(root), meta)
        }
    };

    Ok(HtmlDocument {
        meta,
        root,
        css_text: state.css_chunks.join("\n"),
        scripts: state.scripts,
        links: state.links,
        composition_hosts: state.hosts,
        protos: state.protos,
    })
}

fn tag_of(handle: &Handle) -> Option<String> {
    let node = handle.borrow();
    match &node.data {
        SinkData::Element { name, .. } => Some(name.local.to_string().to_ascii_lowercase()),
        _ => None,
    }
}

/// 元素的降级入口：侧通道标签在任意层级被拦截；返回 None = 不产树节点。
fn lower_element(handle: &Handle, state: &mut LoweringState) -> Result<Option<DomElement>> {
    let (tag, attrs) = {
        let node = handle.borrow();
        match &node.data {
            SinkData::Element { name, attrs, .. } => {
                (name.local.to_string().to_ascii_lowercase(), attrs.clone())
            }
            _ => return Ok(None),
        }
    };

    // 侧通道（任意层级：嵌套 composition 的 style/script 都在 template 里）。
    match tag.as_str() {
        "style" => {
            let text = collect_text(handle);
            state.css_chunks.push(text);
            return Ok(None);
        }
        "script" => {
            let view = Attrs { items: &attrs };
            let src = view.get("src").map(str::to_string);
            let inline = if src.is_none() {
                let text = collect_text(handle);
                (!text.trim().is_empty()).then_some(text)
            } else {
                None
            };
            state.scripts.push(HtmlScript { src, inline });
            return Ok(None);
        }
        "link" => {
            let view = Attrs { items: &attrs };
            let rel = view.get("rel").unwrap_or_default().to_ascii_lowercase();
            if rel.split_whitespace().any(|r| r == "stylesheet")
                && let Some(href) = view.get("href").filter(|h| !h.is_empty())
            {
                state.links.push(HtmlLink {
                    rel,
                    href: href.to_string(),
                });
            }
            return Ok(None);
        }
        "br" => return Ok(None), // 断点只在父循环里有意义
        _ => {}
    }
    if SKIP_TAGS.contains(&tag.as_str()) {
        return Ok(None);
    }

    let view = Attrs { items: &attrs };
    let pre = is_pre_tag(&tag) || has_pre_inline_style(view.get("style"));

    // 协议属性记录 + id 分配（先于子树：合成 id 的计数顺序 = 文档序）。
    let id = state.assign_id(view.get("id"));
    state.record_proto(&id, &view);

    let items = lower_children(handle, state, pre)?;
    let has_element_children = items
        .iter()
        .any(|i| matches!(i, Lowered::Element(_)));
    let inline = matches!(
        tag.as_str(),
        "span" | "a" | "b" | "i" | "em" | "strong" | "small" | "code" | "label"
    );
    let own_text = (!has_element_children).then(|| collect_inline_text(&items, pre, !inline));
    let children = assemble_children(items, state, pre, inline);

    let inline_style = view.get("style").map(str::to_string);
    let class = view.get("class").map(str::to_string);
    let duration = view.f64("data-duration");

    let element = match tag.as_str() {
        "img" => DomElement::Image {
            id,
            class,
            style: inline_style,
            path: view
                .get("src")
                .filter(|s| !is_http_url(s))
                .map(str::to_string),
            url: view
                .get("src")
                .filter(|s| is_http_url(s))
                .map(str::to_string),
            query: None,
            query_count: None,
            aspect_ratio: None,
            duration,
        },
        "video" => {
            let src = video_source(handle, &view);
            DomElement::Video {
                id,
                class,
                style: inline_style,
                path: src.clone().filter(|s| !is_http_url(s)),
                url: src.filter(|s| is_http_url(s)),
                duration,
                data_start: view.f64("data-start"),
                data_duration: view.f64("data-duration"),
                data_media_start: view.f64("data-media-start"),
                data_color_matrix: None,
                hide_after_end: view
                    .get("data-hide-after-end")
                    .map(|v| v.is_empty() || v == "true"),
                looping: view.get("loop").map(|v| v.is_empty() || v == "true"),
                children,
            }
        }
        "audio" => {
            let src = audio_source(handle, &view);
            DomElement::Audio {
                id,
                attach: String::new(),
                path: src.clone().filter(|s| !is_http_url(s)),
                url: src.filter(|s| is_http_url(s)),
                duration,
            }
        }
        "canvas" => DomElement::Canvas {
            id,
            class,
            style: inline_style,
            duration,
            width: view.f64("width").map(|v| v as f32),
            height: view.f64("height").map(|v| v as f32),
            children,
        },
        "svg" | "math" => DomElement::Svg {
            id,
            class,
            style: inline_style,
            src: String::new(),
            inline: Some(serialize_subtree(handle)),
            width: view.f64("width").map(|v| v as f32),
            height: view.f64("height").map(|v| v as f32),
            preserve_aspect_ratio: view.get("preserveAspectRatio").map(str::to_string),
            duration,
        },
        _ => {
            // 文本承载标签：无元素子节点时直接承载 text；混合内容近似为
            // Div 承载（子项含合成 P），身份属性（id/class/style）保留。
            if let Some(tag_name) = text_bearing_tag(&tag)
                && let Some(text) = own_text
                && !text.is_empty()
            {
                if block_level_tag(&tag) {
                    DomElement::P {
                        id,
                        class,
                        style: inline_style,
                        text,
                        ua_tag: Some(tag_name.to_string()),
                        duration,
                    }
                } else {
                    DomElement::Span {
                        id,
                        class,
                        style: inline_style,
                        text,
                        ua_tag: Some(tag_name.to_string()),
                        duration,
                    }
                }
            } else {
                if !CONTAINER_TAGS.contains(&tag.as_str()) {
                    log::debug!("html: 未知标签 `<{tag}>` 按 Div 承载（id={id}）");
                }
                DomElement::Div {
                    id,
                    class,
                    style: inline_style,
                    duration,
                    // 可见窗协议（section/div 通用 fallback 路径）：k3 的
                    // 场景容器全走这里，clip window 语义靠它。
                    data_start: view.f64("data-start"),
                    data_duration: view.f64("data-duration"),
                    // 真标签名进级联（section/button/li… 的 tag 选择器）。
                    ua_tag: Some(tag),
                    children,
                }
            }
        }
    };
    Ok(Some(element))
}

/// 子节点降级：文本累积成 run，元素经 lower_element，`<br>` 成 Break，
/// template 内容透明展开。`pre` 语境（父元素 white-space:pre）保留原始空白。
fn lower_children(handle: &Handle, state: &mut LoweringState, pre: bool) -> Result<Vec<Lowered>> {
    let node = handle.borrow();
    let contents = match &node.data {
        SinkData::Element {
            template_contents: Some(frag),
            ..
        } => frag.clone(),
        _ => handle.clone(),
    };
    drop(node);

    let kids = contents.borrow().children.clone();
    let mut items: Vec<Lowered> = Vec::new();
    let mut text_acc = String::new();
    let mut acc_dirty = false;
    for kid in kids {
        match &kid.borrow().data {
            SinkData::Text(t) => {
                let chunk = if pre { t.clone() } else { t.clone() };
                text_acc.push_str(&chunk);
                acc_dirty = true;
            }
            SinkData::Comment => {}
            SinkData::Element { .. } => {
                if tag_of(&kid).as_deref() == Some("br") {
                    items.push(Lowered::Break);
                    acc_dirty = true;
                    continue;
                }
                if let Some(el) = lower_element(&kid, state)? {
                    if acc_dirty {
                        items.push(Lowered::TextRun(std::mem::take(&mut text_acc)));
                        acc_dirty = false;
                        text_acc.clear();
                    }
                    items.push(Lowered::Element(el));
                }
            }
            SinkData::Document => {}
        }
    }
    if acc_dirty && !text_acc.is_empty() {
        items.push(Lowered::TextRun(text_acc));
    }
    Ok(items)
}

/// Lowered 列表 → DomElement 子项列表：文本 run 包合成 P（首位裁头、
/// 末位裁尾、纯空白丢弃；pre 语境原样），Break 成合成 P("\n")。
fn assemble_children(
    items: Vec<Lowered>,
    state: &mut LoweringState,
    pre: bool,
    inline: bool,
) -> Vec<DomElement> {
    let last = items.len().saturating_sub(1);
    let mut out = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        match item {
            Lowered::Element(el) => out.push(el),
            Lowered::TextRun(raw) => {
                let mut text = collapse_ws(&raw, pre);
                // 行内元素的首尾空白不能裁：`<span> token</span>` 的空格在浏览器
                // 里参与行内排版（k3 的「Every token」空格就在第二个 span 里）。
                if !pre && !inline {
                    if index == 0 {
                        text = text.trim_start().to_string();
                    }
                    if index == last {
                        text = text.trim_end().to_string();
                    }
                }
                if text.is_empty() {
                    continue;
                }
                let id = state.assign_id(None);
                out.push(DomElement::P {
                    id,
                    class: None,
                    style: None,
                    text,
                    ua_tag: None,
                    duration: None,
                });
            }
            Lowered::Break => {
                let id = state.assign_id(None);
                out.push(DomElement::P {
                    id,
                    class: None,
                    style: None,
                    text: "\n".to_string(),
                    ua_tag: None,
                    duration: None,
                });
            }
        }
    }
    out
}

/// 文本承载元素的自有文本：run 序列合并（首末裁剪，内部保单空格）。
fn collect_inline_text(items: &[Lowered], keep_raw: bool, trim: bool) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for item in items {
        if let Lowered::TextRun(raw) = item {
            parts.push(raw);
        }
    }
    if parts.is_empty() {
        return String::new();
    }
    let joined = parts.concat();
    if keep_raw {
        return joined;
    }
    let mut text = collapse_ws(&joined, false);
    if trim {
        return text.trim().to_string();
    }
    // 行内元素保留首尾空白（`<span> token</span>` 的空格参与行内排版）：
    // collapse_ws 会丢掉行首空白，这里按原串补回一个空格。
    if joined.starts_with(char::is_whitespace) {
        text.insert(0, ' ');
    }
    text
}

/// `<video>`/`<audio>` 的源：src 属性优先，其次首个 `<source>` 子元素。
fn video_source(handle: &Handle, view: &Attrs<'_>) -> Option<String> {
    if let Some(src) = view.get("src").filter(|s| !s.is_empty()) {
        return Some(src.to_string());
    }
    first_source_child(handle)
}

fn audio_source(handle: &Handle, view: &Attrs<'_>) -> Option<String> {
    if let Some(src) = view.get("src").filter(|s| !s.is_empty()) {
        return Some(src.to_string());
    }
    first_source_child(handle)
}

fn first_source_child(handle: &Handle) -> Option<String> {
    let kids = handle.borrow().children.clone();
    for kid in kids {
        if tag_of(&kid).as_deref() == Some("source") {
            let node = kid.borrow();
            if let SinkData::Element { attrs, .. } = &node.data {
                let view = Attrs { items: attrs };
                if let Some(src) = view.get("src").filter(|s| !s.is_empty()) {
                    return Some(src.to_string());
                }
            }
        }
    }
    None
}

/// SVG/math 子树回序列化（Foreign 内容属性名大小写已由 html5ever 修正），
/// 产出 usvg 可解析的近似 XML。
fn serialize_subtree(handle: &Handle) -> String {
    let node = handle.borrow();
    match &node.data {
        SinkData::Element { name, attrs, .. } => {
            let tag = name.local.to_string();
            let mut out = format!("<{}", xml_escape(&tag, true));
            for attr in attrs {
                let key = attr.name.local.to_string();
                out.push(' ');
                out.push_str(&xml_escape(&key, true));
                out.push_str("=\"");
                out.push_str(&xml_escape(&attr.value, false));
                out.push('"');
            }
            let kids = node.children.clone();
            drop(node);
            if kids.is_empty() {
                out.push_str("/>");
                return out;
            }
            out.push('>');
            for kid in kids {
                out.push_str(&serialize_subtree(&kid));
            }
            out.push_str(&format!("</{}>", xml_escape(&tag, true)));
            out
        }
        SinkData::Text(t) => xml_escape(t, false),
        _ => String::new(),
    }
}

fn xml_escape(text: &str, attr: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// 深度优先拼接元素子树的文本（含模板内容）。
fn collect_text(handle: &Handle) -> String {
    let mut out = String::new();
    let node = handle.borrow();
    if let SinkData::Text(t) = &node.data {
        return t.clone();
    }
    let contents = match &node.data {
        SinkData::Element {
            template_contents: Some(frag),
            ..
        } => frag.clone(),
        _ => handle.clone(),
    };
    drop(node);
    let frag_kids = contents.borrow().children.clone();
    for kid in frag_kids {
        out.push_str(&collect_text(&kid));
    }
    out
}
