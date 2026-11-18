//! 共有扩展协议层：HyperFrames 与 opencat 都要用的 HTML 扩展属性与运行时协议。
//!
//! 标准层只处理 W3C / 浏览器语义。本模块定义两边共用的组合协议词汇表：
//! 组合标识、可见时间窗、合成尺寸、媒体相位、音量、着色矩阵与
//! `window.__timelines` 注册表。标准层通过这里的窄接口读取协议，
//! 不写协议字面量。
//!
//! 各框架私有扩展放在自己的模块：`hyperframes` 放 HF 独有属性与
//! `DomElement::Timeline` / `Transition` 独有节点。

pub mod attr {
    /// 共有协议属性名。标准层不得再出现这些字面量。
    ///
    /// 单源：这张表同时派生具名常量（Rust 侧消费）与
    /// `protocol-vocab.generated.js` 的 `__ocVocab.attr`（JS 侧消费，
    /// ocir-schema-gen 生成）——新增属性只改这张表。
    macro_rules! protocol_attrs {
        ($($name:ident => $key:literal),* $(,)?) => {
            /// (名, data-* 键) 对照——JS 词表的生成事实源。
            pub const PROTOCOL_ATTRS: &[(&str, &str)] = &[$((stringify!($name), $key)),*];
            $(
                pub const $name: &str = $key;
            )*
        };
    }

    protocol_attrs! {
        COMPOSITION_ID => "data-composition-id",
        COMPOSITION_SRC => "data-composition-src",
        START => "data-start",
        DURATION => "data-duration",
        WIDTH => "data-width",
        HEIGHT => "data-height",
        FPS => "data-fps",
        MEDIA_START => "data-media-start",
        VOLUME => "data-volume",
        COLOR_MATRIX => "data-color-matrix",
        HIDE_AFTER_END => "data-hide-after-end",
    }
}

pub use attr::PROTOCOL_ATTRS;

pub trait AttrSource {
    fn attr(&self, name: &str) -> Option<&str>;
    fn num(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(|v| v.trim().parse().ok())
    }
    fn flag(&self, name: &str) -> Option<bool> {
        self.attr(name).map(|v| v.is_empty() || v == "true")
    }
    fn text(&self, name: &str) -> Option<String> {
        self.attr(name)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ElementProtocol {
    pub composition_id: Option<String>,
    pub composition_src: Option<String>,
    pub start: Option<f64>,
    pub duration: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub fps: Option<f64>,
    pub media_start: Option<f64>,
    pub color_matrix: Option<String>,
    pub hide_after_end: Option<bool>,
}

impl ElementProtocol {
    pub fn read(src: &impl AttrSource) -> Self {
        Self {
            composition_id: src.text(attr::COMPOSITION_ID),
            composition_src: src.text(attr::COMPOSITION_SRC),
            start: src.num(attr::START),
            duration: src.num(attr::DURATION),
            width: src.num(attr::WIDTH),
            height: src.num(attr::HEIGHT),
            fps: src.num(attr::FPS),
            media_start: src.num(attr::MEDIA_START),
            color_matrix: src.text(attr::COLOR_MATRIX),
            hide_after_end: src.flag(attr::HIDE_AFTER_END),
        }
    }
}
