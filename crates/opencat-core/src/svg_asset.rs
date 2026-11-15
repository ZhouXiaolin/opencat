//! SVG 资产契约：`<svg src>` 文档（宿主取字节、core 解析）与
//! `<img src=*.svg>` 的解析入口。
//!
//! core 只解析 SVG，得到尺寸或 usvg 树。像素光栅由 engine 完成。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 端侧取回的 SVG 文档：markup + 其内部外部引用（`<image href>`）的字节。
///
/// core 不读盘、不联网。宿主先取回文档字节，用 [`external_refs`] 预读
/// 出外部引用清单，再把每个引用的字节随文档一起交回。`data:` URI 不进
/// `refs`——usvg 的解码器直接在 core 内解 base64。
///
/// 树缓存挂在文档上（按解析字体库身份分槽）：同一文档在 resolve 取
/// intrinsic 尺寸、render 展开 op 流时只解析一次，且引用字节变了也不会
/// 与新文档撞缓存。
#[derive(Clone)]
pub struct SvgDocument {
    markup: Arc<str>,
    refs: HashMap<String, Arc<[u8]>>,
    trees: Arc<Mutex<HashMap<usize, Option<Arc<usvg::Tree>>>>>,
}

impl SvgDocument {
    pub fn new(markup: impl Into<Arc<str>>, refs: HashMap<String, Arc<[u8]>>) -> Self {
        Self {
            markup: markup.into(),
            refs,
            trees: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 解析后的树（带缓存）。字体作用域与 inline svg 同口径：借当前文档
    /// 字体库，缺席时落系统扫描。
    pub fn tree(&self) -> Option<Arc<usvg::Tree>> {
        let db_id = Arc::as_ptr(&crate::text::script_font_db()) as usize;
        if let Ok(cache) = self.trees.lock()
            && let Some(tree) = cache.get(&db_id)
        {
            return tree.clone();
        }
        let tree = crate::render::svg::parse_document(&self.markup, &self.refs).map(Arc::new);
        if let Ok(mut cache) = self.trees.lock() {
            cache.insert(db_id, tree.clone());
        }
        tree
    }
}

impl std::fmt::Debug for SvgDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvgDocument")
            .field("markup_len", &self.markup.len())
            .field("refs", &self.refs.len())
            .finish()
    }
}

/// 预读：markup 里 `<image>` 指向的非 `data:` 外部引用（去重，保持首现
/// 顺序）。
///
/// 走 usvg 自己的 href 解析接缝（`ImageHrefResolver::resolve_string`），
/// 与真正解析时的判定同源——不在文本层用正则另造一套口径。第一遍解析
/// 不产出树，只收集引用；宿主取回字节后由 [`SvgDocument`] 带着 refs 再
/// 解析一次。
pub fn external_refs(markup: &str) -> Vec<String> {
    let refs = Mutex::new(Vec::new());
    let opt = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|href, _| {
                if !href.starts_with("data:") && !href.starts_with('#') {
                    refs.lock().expect("svg refs").push(href.to_string());
                }
                None
            }),
        },
        ..Default::default()
    };
    let _ = usvg::Tree::from_str(markup, &opt);
    // 闭包持有 `refs` 的借用；先放掉 opt 才能取回收集结果。
    drop(opt);
    let mut refs = refs.into_inner().expect("svg refs");
    let mut seen = std::collections::HashSet::new();
    refs.retain(|href| seen.insert(href.clone()));
    refs
}

/// svg 字节嗅探：utf-8 且以 `<` 开头（`<?xml` / `<svg`）。位图魔数从来
/// 不以 `<` 开头，误报不存在。
fn svg_text(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    text.trim_start().starts_with('<').then_some(text)
}

/// img 语义的字体作用域：显式置空文档字体作用域，parse_tree 落回系统
/// 扫描。浏览器里 `<img>` svg 是隔离文档——@font-face web 字体不可见，
/// 只认系统/内部资源；不隔离的话脚本桥（Image 解码发生在 frame 的
/// scope_font_db 内）会把文档字体泄漏进 img svg。
fn with_img_font_scope<R>(f: impl FnOnce() -> R) -> R {
    let empty = std::sync::Arc::new(fontdb::Database::new());
    crate::text::scope_font_db(&empty, f)
}

/// intrinsic 尺寸（width/height/viewBox 归一后的画布尺寸）。非 svg 字节
/// 或解析失败返回 None（调用方落回位图探测）。
pub fn svg_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    with_img_font_scope(|| {
        let text = svg_text(bytes)?;
        let tree = crate::render::svg::load_tree_markup(text)?;
        let (w, h) = crate::render::svg::tree_size(&tree);
        (w > 0.0 && h > 0.0).then(|| (w.round() as u32, h.round() as u32))
    })
}

/// 解析 img 语义的 SVG。字体作用域按浏览器 `<img>` 的隔离文档处理。
pub fn parse_img(bytes: &[u8]) -> Option<std::sync::Arc<usvg::Tree>> {
    with_img_font_scope(|| {
        let text = svg_text(bytes)?;
        crate::render::svg::load_tree_markup(text)
    })
}
