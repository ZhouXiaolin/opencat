use std::sync::Arc;

use crate::{
    FrameCtx,
    nodes::{Div, Image, Text, Video},
    style::NodeStyle,
    timeline::TimelineNode,
};

#[derive(Clone)]
pub struct Node(Arc<NodeKind>);

impl Node {
    pub fn new<T>(node: T) -> Self
    where
        T: Into<NodeKind>,
    {
        Self(Arc::new(node.into()))
    }

    pub fn kind(&self) -> &NodeKind {
        self.0.as_ref()
    }

    pub fn style_ref(&self) -> &NodeStyle {
        self.kind().style_ref()
    }

    pub fn duration_in_frames(&self, ctx: &FrameCtx) -> Option<u32> {
        self.kind().duration_in_frames(ctx)
    }
}

#[derive(Clone)]
pub enum NodeKind {
    Component(ComponentNode),
    Div(Div),
    Text(Text),
    Image(Image),
    Video(Video),
    Timeline(TimelineNode),
}

impl NodeKind {
    pub fn style_ref(&self) -> &NodeStyle {
        match self {
            Self::Component(node) => node.style_ref(),
            Self::Div(node) => node.style_ref(),
            Self::Text(node) => node.style_ref(),
            Self::Image(node) => node.style_ref(),
            Self::Video(node) => node.style_ref(),
            Self::Timeline(node) => node.style_ref(),
        }
    }

    pub fn duration_in_frames(&self, ctx: &FrameCtx) -> Option<u32> {
        match self {
            Self::Component(node) => node.duration_in_frames(ctx),
            Self::Div(node) => node.duration_in_frames(ctx),
            Self::Timeline(node) => Some(node.duration_in_frames()),
            Self::Text(_) | Self::Image(_) | Self::Video(_) => None,
        }
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

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn duration_in_frames(&self, ctx: &FrameCtx) -> Option<u32> {
        if let Some(duration_in_frames) = &self.duration_in_frames {
            return Some(duration_in_frames());
        }

        self.render(ctx).duration_in_frames(ctx)
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

impl From<ComponentNode> for NodeKind {
    fn from(value: ComponentNode) -> Self {
        Self::Component(value)
    }
}

impl From<Div> for NodeKind {
    fn from(value: Div) -> Self {
        Self::Div(value)
    }
}

impl From<Text> for NodeKind {
    fn from(value: Text) -> Self {
        Self::Text(value)
    }
}

impl From<Image> for NodeKind {
    fn from(value: Image) -> Self {
        Self::Image(value)
    }
}

impl From<Video> for NodeKind {
    fn from(value: Video) -> Self {
        Self::Video(value)
    }
}

impl From<TimelineNode> for NodeKind {
    fn from(value: TimelineNode) -> Self {
        Self::Timeline(value)
    }
}

impl<T> From<T> for Node
where
    T: Into<NodeKind>,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
