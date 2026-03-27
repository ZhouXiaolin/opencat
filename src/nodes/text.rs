use std::any::Any;

use crate::{
    ViewNode,
    style::{ComputedTextStyle, NodeStyle, impl_node_style_api},
    typography,
};

#[derive(Clone)]
pub struct Text {
    text: String,
    pub(crate) style: NodeStyle,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: NodeStyle::default(),
        }
    }

    pub fn content(&self) -> &str {
        &self.text
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn measured_size(&self, computed_style: &ComputedTextStyle) -> (f32, f32) {
        typography::measure_text(&self.text, computed_style)
    }

    pub fn draw_at(
        &self,
        canvas: &skia_safe::Canvas,
        left: f32,
        top: f32,
        computed_style: &ComputedTextStyle,
    ) {
        typography::draw_text(canvas, &self.text, left, top, computed_style);
    }
}

impl_node_style_api!(Text);

impl ViewNode for Text {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}
