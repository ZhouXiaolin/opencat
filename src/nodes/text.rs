use std::any::Any;

use skia_safe::{Canvas, Font, FontMgr, FontStyle, Paint, Rect};

use crate::{
    FrameCtx, ViewNode,
    style::{ColorToken, ComputedTextStyle, NodeStyle, impl_node_style_api, resolve_text_style},
};

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

    fn make_font(&self, computed_style: &ComputedTextStyle) -> Font {
        let font_size = self.resolve_text_style(computed_style).text_px;
        let font_mgr = FontMgr::new();
        if let Some(typeface) = font_mgr.legacy_make_typeface(None, FontStyle::normal()) {
            Font::new(typeface, font_size)
        } else {
            let mut font = Font::default();
            font.set_size(font_size);
            font
        }
    }

    pub fn content(&self) -> &str {
        &self.text
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn resolve_text_style(&self, inherited: &ComputedTextStyle) -> ComputedTextStyle {
        resolve_text_style(inherited, &self.style)
    }

    pub fn resolved_color(&self, computed_style: &ComputedTextStyle) -> ColorToken {
        self.resolve_text_style(computed_style).color
    }

    pub fn resolved_font_size(&self, computed_style: &ComputedTextStyle) -> f32 {
        self.resolve_text_style(computed_style).text_px
    }

    pub fn measured_size(&self, computed_style: &ComputedTextStyle) -> (f32, f32) {
        let font = self.make_font(computed_style);
        let (width, bounds) = font.measure_str(&self.text, None);
        (width.max(1.0), bounds.height().max(1.0))
    }

    pub fn draw_at(&self, canvas: &Canvas, left: f32, top: f32, computed_style: &ComputedTextStyle) {
        let mut paint = Paint::default();
        paint.set_color(self.resolved_color(computed_style).to_skia());
        paint.set_anti_alias(true);

        let font = self.make_font(computed_style);
        let (_, bounds) = font.measure_str(&self.text, None);
        let baseline = top - bounds.top;
        canvas.draw_str(&self.text, (left, baseline), &font, &paint);
    }

    pub(crate) fn draw_resolved(
        canvas: &Canvas,
        left: f32,
        top: f32,
        text: impl Into<String>,
        color: ColorToken,
        font_size: f32,
    ) {
        let text_node = Text::new(text);
        let style = ComputedTextStyle {
            text_px: font_size,
            color,
        };
        text_node.draw_at(canvas, left, top, &style);
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

    fn intrinsic_size(&self, _ctx: &FrameCtx, computed_style: &ComputedTextStyle) -> Option<(f32, f32)> {
        Some(self.measured_size(computed_style))
    }

    fn draw(&self, _ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, computed_style: &ComputedTextStyle) {
        self.draw_at(canvas, bounds.left, bounds.top, computed_style);
    }
}
