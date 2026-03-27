use std::{any::Any, ops::Deref, sync::Arc};

use crate::{FrameCtx, style::{ComputedTextStyle, NodeStyle}};
use skia_safe::{Canvas, Rect};

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

pub trait ViewNode: Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn style_ref(&self) -> &NodeStyle;

    fn intrinsic_size(
        &self,
        _ctx: &FrameCtx,
        _computed_style: &ComputedTextStyle,
    ) -> Option<(f32, f32)> {
        None
    }

    fn draw(&self, ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, computed_style: &ComputedTextStyle);
}
