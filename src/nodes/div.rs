use std::any::Any;

use skia_safe::{Canvas, Paint, Rect, RRect};

use crate::{
    FrameCtx, Node, ViewNode,
    style::{ColorToken, ComputedTextStyle, NodeStyle, impl_node_style_api, resolve_text_style},
};

/// A container node with flex layout support.
/// By default, acts as a flex container and positioning context (like `relative` in CSS).
#[derive(Clone)]
pub struct Div {
    pub(crate) style: NodeStyle,
    pub(crate) children: Vec<Node>,
}

impl Div {
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

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

impl_node_style_api!(Div);

impl ViewNode for Div {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    fn draw(&self, _ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, _computed_style: &ComputedTextStyle) {
        let bg_color = self.style.bg_color;
        let border_radius = self.style.border_radius.unwrap_or(0.0);
        let border_width = self.style.border_width;
        let border_color = self.style.border_color;

        // Draw background with optional rounded corners
        if bg_color.is_some() || border_width.is_some() {
            let mut paint = Paint::default();
            paint.set_anti_alias(true);

            if border_radius > 0.0 {
                // Rounded rectangle
                let rrect = RRect::new_rect_xy(bounds, border_radius, border_radius);

                // Fill background
                if let Some(color) = bg_color {
                    paint.set_color(color.to_skia());
                    canvas.draw_rrect(rrect, &paint);
                }

                // Draw border
                if let (Some(width), Some(color)) = (border_width, border_color) {
                    paint.set_color(color.to_skia());
                    paint.set_style(skia_safe::PaintStyle::Stroke);
                    paint.set_stroke_width(width);
                    canvas.draw_rrect(rrect, &paint);
                }
            } else {
                // Regular rectangle
                if let Some(color) = bg_color {
                    paint.set_color(color.to_skia());
                    canvas.draw_rect(bounds, &paint);
                }

                // Draw border
                if let (Some(width), Some(color)) = (border_width, border_color) {
                    paint.set_color(color.to_skia());
                    paint.set_style(skia_safe::PaintStyle::Stroke);
                    paint.set_stroke_width(width);
                    canvas.draw_rect(bounds, &paint);
                }
            }
        }
    }
}
