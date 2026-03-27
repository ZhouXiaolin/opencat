use std::{any::Any, ops::Deref, sync::Arc};

use crate::FrameCtx;
use skia_safe::{Canvas, Color, Rect};

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

#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    pub font_size: f32,
    pub color: Color,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            color: Color::BLACK,
        }
    }
}

pub trait ViewNode: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn intrinsic_size(&self, _ctx: &FrameCtx, _text_style: &TextStyle) -> Option<(f32, f32)> {
        None
    }

    fn draw(&self, ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, text_style: &TextStyle);
}
