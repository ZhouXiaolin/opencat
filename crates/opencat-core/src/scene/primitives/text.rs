use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone)]
pub struct Text {
    text: String,
    pub(crate) style: NodeStyle,
}

impl Text {
    pub fn content(&self) -> &str {
        &self.text
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn text(content: impl Into<String>) -> Text {
    Text {
        text: content.into(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Text);
