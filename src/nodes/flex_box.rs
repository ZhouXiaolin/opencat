use std::sync::Arc;

use skia_safe::{Canvas, Rect};

use crate::{FrameCtx, Node, ViewNode, view::IntoNode};

#[derive(Debug, Clone, Copy)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy)]
pub enum JustifyContent {
    Start,
    Center,
}

#[derive(Debug, Clone, Copy)]
pub enum AlignItems {
    Start,
    Center,
}

pub struct FlexBox {
    direction: FlexDirection,
    justify_content: JustifyContent,
    align_items: AlignItems,
    children: Vec<Node>,
}

impl FlexBox {
    pub fn new() -> Self {
        Self {
            direction: FlexDirection::Column,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Start,
            children: Vec::new(),
        }
    }

    pub fn direction(mut self, direction: FlexDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn justify_content(mut self, justify_content: JustifyContent) -> Self {
        self.justify_content = justify_content;
        self
    }

    pub fn align_items(mut self, align_items: AlignItems) -> Self {
        self.align_items = align_items;
        self
    }

    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    fn measure_children(&self, ctx: &FrameCtx) -> Vec<(f32, f32)> {
        self.children
            .iter()
            .map(|c| c.intrinsic_size(ctx).unwrap_or((0.0, 0.0)))
            .collect()
    }
}

impl ViewNode for FlexBox {
    fn intrinsic_size(&self, _ctx: &FrameCtx) -> Option<(f32, f32)> {
        None
    }

    fn draw(&self, ctx: &FrameCtx, canvas: &Canvas, bounds: Rect) {
        let sizes = self.measure_children(ctx);

        let main_total = match self.direction {
            FlexDirection::Row => sizes.iter().map(|(w, _)| *w).sum::<f32>(),
            FlexDirection::Column => sizes.iter().map(|(_, h)| *h).sum::<f32>(),
        };

        let container_main = match self.direction {
            FlexDirection::Row => bounds.width(),
            FlexDirection::Column => bounds.height(),
        };

        let mut main_cursor = match self.justify_content {
            JustifyContent::Start => 0.0,
            JustifyContent::Center => (container_main - main_total) / 2.0,
        };

        for (child, (child_w, child_h)) in self.children.iter().zip(sizes.iter()) {
            let cross_offset = match (self.direction, self.align_items) {
                (FlexDirection::Row, AlignItems::Start) => 0.0,
                (FlexDirection::Row, AlignItems::Center) => (bounds.height() - child_h) / 2.0,
                (FlexDirection::Column, AlignItems::Start) => 0.0,
                (FlexDirection::Column, AlignItems::Center) => (bounds.width() - child_w) / 2.0,
            };

            let child_rect = match self.direction {
                FlexDirection::Row => Rect::from_xywh(
                    bounds.left + main_cursor,
                    bounds.top + cross_offset,
                    *child_w,
                    *child_h,
                ),
                FlexDirection::Column => Rect::from_xywh(
                    bounds.left + cross_offset,
                    bounds.top + main_cursor,
                    *child_w,
                    *child_h,
                ),
            };

            child.draw(ctx, canvas, child_rect);

            main_cursor += match self.direction {
                FlexDirection::Row => *child_w,
                FlexDirection::Column => *child_h,
            };
        }
    }
}

impl IntoNode for FlexBox {
    fn into_node(self) -> Node {
        Arc::new(self)
    }
}
