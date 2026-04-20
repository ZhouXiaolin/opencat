use crate::{FrameCtx, Node, style::NodeStyle};

use crate::style::impl_node_style_api;

#[derive(Clone)]
pub struct LayerNode {
    pub(crate) style: NodeStyle,
    pub(crate) children: Vec<Node>,
}

impl LayerNode {
    pub fn child<T: Into<Node>>(mut self, child: T) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children_ref(&self) -> &[Node] {
        &self.children
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn duration_in_frames(&self, ctx: &FrameCtx) -> Option<u32> {
        self.children
            .iter()
            .filter_map(|child| child.duration_in_frames(ctx))
            .max()
    }
}

pub fn layer() -> LayerNode {
    LayerNode {
        style: NodeStyle::default(),
        children: Vec::new(),
    }
}

impl_node_style_api!(LayerNode);
