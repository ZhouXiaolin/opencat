use std::any::Any;

use crate::{
    FrameCtx, Node, ViewNode,
    style::{ColorToken, NodeStyle, impl_node_style_api},
};

/// A container node with flex layout support.
/// By default, acts as a flex container and positioning context (like `relative` in CSS).
#[derive(Clone)]
pub struct Div {
    pub(crate) style: NodeStyle,
    pub(crate) children: Vec<Node>,
}

impl Div {
    pub fn new() -> Self {
        Self {
            style: NodeStyle {
                bg_color: Some(ColorToken::White),
                ..Default::default()
            },
            children: Vec::new(),
        }
    }

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
}

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

impl_node_style_api!(Div);

impl ViewNode for Div {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    fn duration_in_frames(&self, ctx: &FrameCtx) -> Option<u32> {
        self.children
            .iter()
            .filter_map(|child| child.duration_in_frames(ctx))
            .max()
    }
}
