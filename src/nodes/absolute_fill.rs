use std::any::Any;

use skia_safe::{Canvas, Paint, Rect};

use crate::{
    FrameCtx, Node, ViewNode,
    style::{ColorToken, ComputedTextStyle, NodeStyle, impl_node_style_api, resolve_text_style},
};
#[derive(Clone)]
pub struct AbsoluteFill {
    pub(crate) style: NodeStyle,
    children: Vec<Node>,
}

impl AbsoluteFill {
    pub fn new() -> Self {
        Self {
            style: NodeStyle {
                bg_color: Some(ColorToken::White),
                ..Default::default()
            },
            children: Vec::new(),
        }
    }

    pub fn child<T: Into<Node>>(mut self, child: T) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children_ref(&self) -> &[Node] {
        &self.children
    }

    pub fn background_color_value(&self) -> ColorToken {
        self.style.bg_color.unwrap_or(ColorToken::White)
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn resolve_text_style(&self, inherited: &ComputedTextStyle) -> ComputedTextStyle {
        resolve_text_style(inherited, &self.style)
    }
}

impl_node_style_api!(AbsoluteFill);

impl ViewNode for AbsoluteFill {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    fn draw(&self, _ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, _computed_style: &ComputedTextStyle) {
        // Draw background - layout is handled by taffy
        let mut paint = Paint::default();
        paint.set_color(self.background_color_value().to_skia());
        paint.set_anti_alias(true);
        canvas.draw_rect(bounds, &paint);
    }
}
