use std::sync::Arc;

use crate::FrameCtx;
use skia_safe::{Canvas, Rect};

pub type Node = Arc<dyn ViewNode>;

pub trait ViewNode: Send + Sync {
    fn intrinsic_size(&self, _ctx: &FrameCtx) -> Option<(f32, f32)> {
        None
    }

    fn draw(&self, ctx: &FrameCtx, canvas: &Canvas, bounds: Rect);
}

pub trait IntoNode {
    fn into_node(self) -> Node;
}
