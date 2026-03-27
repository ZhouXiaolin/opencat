use std::{any::Any, ops::Deref, sync::Arc};

use crate::{FrameCtx, style::NodeStyle};

#[derive(Clone)]
pub struct Node(Arc<dyn ViewNode>);

impl Node {
    pub fn new<T>(node: T) -> Self
    where
        T: ViewNode + 'static,
    {
        Self(Arc::new(node))
    }
}

#[derive(Clone)]
pub struct ComponentNode {
    render: Arc<dyn Fn(&FrameCtx) -> Node + Send + Sync>,
    duration_in_frames: Option<Arc<dyn Fn() -> u32 + Send + Sync>>,
    style: NodeStyle,
}

impl ComponentNode {
    pub fn new<F>(render: F) -> Self
    where
        F: Fn(&FrameCtx) -> Node + Send + Sync + 'static,
    {
        Self {
            render: Arc::new(render),
            duration_in_frames: None,
            style: NodeStyle::default(),
        }
    }

    pub fn with_duration<F, D>(render: F, duration_in_frames: D) -> Self
    where
        F: Fn(&FrameCtx) -> Node + Send + Sync + 'static,
        D: Fn() -> u32 + Send + Sync + 'static,
    {
        Self {
            render: Arc::new(render),
            duration_in_frames: Some(Arc::new(duration_in_frames)),
            style: NodeStyle::default(),
        }
    }

    pub fn render(&self, ctx: &FrameCtx) -> Node {
        (self.render)(ctx)
    }
}

pub fn component_node<F>(render: F) -> Node
where
    F: Fn(&FrameCtx) -> Node + Send + Sync + 'static,
{
    Node::new(ComponentNode::new(render))
}

pub fn component_node_with_duration<F, D>(render: F, duration_in_frames: D) -> Node
where
    F: Fn(&FrameCtx) -> Node + Send + Sync + 'static,
    D: Fn() -> u32 + Send + Sync + 'static,
{
    Node::new(ComponentNode::with_duration(render, duration_in_frames))
}

impl<T> From<T> for Node
where
    T: ViewNode + 'static,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl Deref for Node {
    type Target = dyn ViewNode;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

pub trait ViewNode: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn style_ref(&self) -> &NodeStyle;

    fn duration_in_frames(&self, _ctx: &FrameCtx) -> Option<u32> {
        None
    }
}

impl ViewNode for ComponentNode {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    fn duration_in_frames(&self, ctx: &FrameCtx) -> Option<u32> {
        if let Some(duration_in_frames) = &self.duration_in_frames {
            return Some(duration_in_frames());
        }

        self.render(ctx).duration_in_frames(ctx)
    }
}
