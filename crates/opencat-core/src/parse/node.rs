use std::sync::Arc;

use crate::{
    frame_ctx::FrameCtx,
    parse::{
        primitives::{Canvas, CaptionNode, Div, Image, Lucide, Path, Text, Video},
        time::TimelineNode,
    },
    script::ScriptDriver,
    style::NodeStyle,
};

#[derive(Clone)]
pub struct Node(Arc<NodeKind>);

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Node").field(&self.0).finish()
    }
}

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

    pub fn script_driver(self, driver: ScriptDriver) -> Self {
        let mut kind = self.kind().clone();
        kind.style_mut().script_driver = Some(std::sync::Arc::new(driver));
        Self(Arc::new(kind))
    }

    pub fn script_source(self, source: &str) -> anyhow::Result<Self> {
        let driver = ScriptDriver::from_source(source)?;
        Ok(self.script_driver(driver))
    }
}

#[derive(Clone)]
pub enum NodeKind {
    Div(Div),
    Canvas(Canvas),
    Text(Text),
    Image(Image),
    Lucide(Lucide),
    Path(Path),
    Video(Video),
    Timeline(TimelineNode),
    Caption(CaptionNode),
}

impl std::fmt::Debug for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Div(_) => write!(f, "Div(..)"),
            Self::Canvas(_) => write!(f, "Canvas(..)"),
            Self::Text(_) => write!(f, "Text(..)"),
            Self::Image(_) => write!(f, "Image(..)"),
            Self::Lucide(_) => write!(f, "Lucide(..)"),
            Self::Path(_) => write!(f, "Path(..)"),
            Self::Video(_) => write!(f, "Video(..)"),
            Self::Timeline(_) => write!(f, "Timeline(..)"),
            Self::Caption(_) => write!(f, "Caption(..)"),
        }
    }
}

impl NodeKind {
    pub fn style_ref(&self) -> &NodeStyle {
        match self {
            Self::Div(node) => node.style_ref(),
            Self::Canvas(node) => node.style_ref(),
            Self::Text(node) => node.style_ref(),
            Self::Image(node) => node.style_ref(),
            Self::Lucide(node) => node.style_ref(),
            Self::Path(node) => node.style_ref(),
            Self::Video(node) => node.style_ref(),
            Self::Timeline(node) => node.style_ref(),
            Self::Caption(node) => node.style_ref(),
        }
    }

    pub fn duration_in_frames(&self, ctx: &FrameCtx) -> Option<u32> {
        match self {
            Self::Div(node) => node.duration_in_frames(ctx),
            Self::Timeline(node) => Some(node.duration_in_frames()),
            Self::Text(_)
            | Self::Canvas(_)
            | Self::Image(_)
            | Self::Lucide(_)
            | Self::Path(_)
            | Self::Video(_)
            | Self::Caption(_) => None,
        }
    }

    pub fn style_mut(&mut self) -> &mut NodeStyle {
        match self {
            Self::Div(node) => &mut node.style,
            Self::Canvas(node) => &mut node.style,
            Self::Text(node) => &mut node.style,
            Self::Image(node) => &mut node.style,
            Self::Lucide(node) => &mut node.style,
            Self::Path(node) => &mut node.style,
            Self::Video(node) => &mut node.style,
            Self::Timeline(node) => &mut node.style,
            Self::Caption(node) => &mut node.style,
        }
    }
}


impl From<Div> for NodeKind {
    fn from(value: Div) -> Self {
        Self::Div(value)
    }
}

impl From<Canvas> for NodeKind {
    fn from(value: Canvas) -> Self {
        Self::Canvas(value)
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

impl From<Lucide> for NodeKind {
    fn from(value: Lucide) -> Self {
        Self::Lucide(value)
    }
}

impl From<Path> for NodeKind {
    fn from(value: Path) -> Self {
        Self::Path(value)
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

impl From<CaptionNode> for NodeKind {
    fn from(value: CaptionNode) -> Self {
        Self::Caption(value)
    }
}

impl From<Div> for Node {
    fn from(value: Div) -> Self {
        Self::new(value)
    }
}

impl From<Canvas> for Node {
    fn from(value: Canvas) -> Self {
        Self::new(value)
    }
}

impl From<Text> for Node {
    fn from(value: Text) -> Self {
        Self::new(value)
    }
}

impl From<Image> for Node {
    fn from(value: Image) -> Self {
        Self::new(value)
    }
}

impl From<Lucide> for Node {
    fn from(value: Lucide) -> Self {
        Self::new(value)
    }
}

impl From<Path> for Node {
    fn from(value: Path) -> Self {
        Self::new(value)
    }
}

impl From<Video> for Node {
    fn from(value: Video) -> Self {
        Self::new(value)
    }
}

impl From<TimelineNode> for Node {
    fn from(value: TimelineNode) -> Self {
        Self::new(value)
    }
}

impl From<CaptionNode> for Node {
    fn from(value: CaptionNode) -> Self {
        Self::new(value)
    }
}
