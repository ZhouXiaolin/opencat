use crate::style::{NodeStyle, impl_node_style_api};

#[derive(Clone)]
pub struct Path {
    pub(crate) data: String,
    pub(crate) style: NodeStyle,
}

impl Path {
    pub fn data(&self) -> &str {
        &self.data
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

pub fn path(data: impl Into<String>) -> Path {
    Path {
        data: data.into(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Path);
