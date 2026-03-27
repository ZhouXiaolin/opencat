use std::sync::Arc;

use skia_safe::{Canvas, Color, Paint, Rect};

use crate::{FrameCtx, Node, ViewNode, view::IntoNode};

#[derive(Debug, Clone, Copy)]
enum MainAxis {
    Start,
    Center,
}

#[derive(Debug, Clone, Copy)]
enum CrossAxis {
    Start,
    Center,
}

pub struct AbsoluteFill {
    background: Option<Color>,
    justify: MainAxis,
    align: CrossAxis,
    child: Option<Node>,
}

impl AbsoluteFill {
    pub fn new() -> Self {
        Self {
            background: None,
            justify: MainAxis::Start,
            align: CrossAxis::Start,
            child: None,
        }
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn justify_center(mut self) -> Self {
        self.justify = MainAxis::Center;
        self
    }

    pub fn align_center(mut self) -> Self {
        self.align = CrossAxis::Center;
        self
    }

    pub fn child(mut self, child: Node) -> Self {
        self.child = Some(child);
        self
    }
}

impl ViewNode for AbsoluteFill {
    fn draw(&self, ctx: &FrameCtx, canvas: &Canvas, bounds: Rect) {
        if let Some(color) = self.background {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.set_anti_alias(true);
            canvas.draw_rect(bounds, &paint);
        }

        let Some(child) = &self.child else {
            return;
        };

        let child_bounds = if let Some((width, height)) = child.intrinsic_size(ctx) {
            let x = match self.align {
                CrossAxis::Start => bounds.left,
                CrossAxis::Center => bounds.left + ((bounds.width() - width) / 2.0),
            };
            let y = match self.justify {
                MainAxis::Start => bounds.top,
                MainAxis::Center => bounds.top + ((bounds.height() - height) / 2.0),
            };
            Rect::from_xywh(x, y, width, height)
        } else {
            bounds
        };

        child.draw(ctx, canvas, child_bounds);
    }
}

impl IntoNode for AbsoluteFill {
    fn into_node(self) -> Node {
        Arc::new(self)
    }
}
