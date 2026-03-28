use crate::{
    FrameCtx, Node,
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

pub fn div() -> Div {
    Div {
        style: NodeStyle {
            bg_color: Some(ColorToken::White),
            ..Default::default()
        },
        children: Vec::new(),
    }
}

impl Default for Div {
    fn default() -> Self {
        div()
    }
}

impl_node_style_api!(Div);
