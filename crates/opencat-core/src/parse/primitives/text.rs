use crate::style::{NodeStyle, impl_node_style_api};

/// 作者写的 HTML 文本标签。脚本侧 DOM 的 tagName 反映它（SplitText 等
/// 插件按 tagName 分支）；布局面 p/span 当前同机制。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextTag {
    #[default]
    P,
    Span,
}

impl TextTag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::P => "p",
            Self::Span => "span",
        }
    }
}

#[derive(Clone)]
pub struct Text {
    text: String,
    pub(crate) tag: TextTag,
    pub(crate) style: NodeStyle,
}

impl Text {
    pub fn content(&self) -> &str {
        &self.text
    }

    pub fn tag(&self) -> TextTag {
        self.tag
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn set_style(&mut self, style: NodeStyle) {
        self.style = style;
    }
}

pub fn text(content: impl Into<String>) -> Text {
    Text {
        text: content.into(),
        tag: TextTag::P,
        style: NodeStyle::default(),
    }
}

/// `<span>`：与 p 同一文本节点机制，tag 进脚本侧 tagName。
pub fn span(content: impl Into<String>) -> Text {
    Text {
        tag: TextTag::Span,
        ..text(content)
    }
}

impl_node_style_api!(Text);
