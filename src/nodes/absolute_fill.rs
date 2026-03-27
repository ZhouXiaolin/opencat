use std::any::Any;

use skia_safe::{Canvas, Color, Paint, Rect};

use crate::{
    FrameCtx, Node, ViewNode,
    view::{TextStyle},
};

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

pub struct AbsoluteFill {
    background_color: Color,
    justify_content: JustifyContent,
    align_items: AlignItems,
    font_size: Option<f32>,
    color: Option<Color>,
    child: Option<Node>,
}

impl AbsoluteFill {
    pub fn new() -> Self {
        Self {
            background_color: Color::WHITE,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Start,
            font_size: None,
            color: None,
            child: None,
        }
    }

    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = color;
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

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn child<T: Into<Node>>(mut self, child: T) -> Self {
        self.child = Some(child.into());
        self
    }

    pub fn background_color_value(&self) -> Color {
        self.background_color
    }

    pub fn justify_content_value(&self) -> JustifyContent {
        self.justify_content
    }

    pub fn align_items_value(&self) -> AlignItems {
        self.align_items
    }

    pub fn child_ref(&self) -> Option<&Node> {
        self.child.as_ref()
    }

    pub fn resolve_text_style(&self, inherited: &TextStyle) -> TextStyle {
        TextStyle {
            font_size: self.font_size.unwrap_or(inherited.font_size),
            color: self.color.unwrap_or(inherited.color),
        }
    }
}

impl ViewNode for AbsoluteFill {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn draw(&self, ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, text_style: &TextStyle) {
        let mut paint = Paint::default();
        paint.set_color(self.background_color);
        paint.set_anti_alias(true);
        canvas.draw_rect(bounds, &paint);

        let Some(child) = &self.child else {
            return;
        };

        let next_style = self.resolve_text_style(text_style);

        let child_bounds = if let Some((width, height)) = child.intrinsic_size(ctx, &next_style) {
            let x = match self.align_items {
                AlignItems::Start => bounds.left,
                AlignItems::Center => bounds.left + ((bounds.width() - width) / 2.0),
            };
            let y = match self.justify_content {
                JustifyContent::Start => bounds.top,
                JustifyContent::Center => bounds.top + ((bounds.height() - height) / 2.0),
            };
            Rect::from_xywh(x, y, width, height)
        } else {
            bounds
        };

        child.draw(ctx, canvas, child_bounds, &next_style);
    }
}
