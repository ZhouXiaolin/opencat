use crate::style::{impl_node_style_api, NodeStyle};

#[derive(Clone)]
pub struct Lucide {
    icon: String,
    pub(crate) style: NodeStyle,
}

impl Lucide {
    pub fn icon(&self) -> &str {
        &self.icon
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn lucide(name: impl Into<String>) -> Lucide {
    Lucide {
        icon: name.into(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Lucide);
