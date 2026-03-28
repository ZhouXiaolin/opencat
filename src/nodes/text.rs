use crate::{
    style::{ComputedTextStyle, NodeStyle, impl_node_style_api},
    typography,
};

#[derive(Clone)]
pub struct Text {
    text: String,
    pub(crate) style: NodeStyle,
}

impl Text {
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

pub fn text(content: impl Into<String>) -> Text {
    Text {
        text: content.into(),
        style: NodeStyle::default(),
    }
}

impl_node_style_api!(Text);
