use std::any::Any;

use skia_safe::{Canvas, Color, Font, FontMgr, FontStyle, Paint, Rect};

use crate::{FrameCtx, ViewNode, view::TextStyle};

pub struct Text {
    text: String,
    color: Option<Color>,
    font_size: Option<f32>,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            font_size: None,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = Some(font_size);
        self
    }

    fn make_font(&self, text_style: &TextStyle) -> Font {
        let font_size = self.font_size.unwrap_or(text_style.font_size);
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

    pub fn resolved_color(&self, text_style: &TextStyle) -> Color {
        self.color.unwrap_or(text_style.color)
    }

    pub fn resolved_font_size(&self, text_style: &TextStyle) -> f32 {
        self.font_size.unwrap_or(text_style.font_size)
    }

    pub fn measured_size(&self, text_style: &TextStyle) -> (f32, f32) {
        let font = self.make_font(text_style);
        let (width, bounds) = font.measure_str(&self.text, None);
        (width.max(1.0), bounds.height().max(1.0))
    }

    pub fn draw_at(&self, canvas: &Canvas, left: f32, top: f32, text_style: &TextStyle) {
        let mut paint = Paint::default();
        paint.set_color(self.resolved_color(text_style));
        paint.set_anti_alias(true);

        let font = self.make_font(text_style);
        let (_, bounds) = font.measure_str(&self.text, None);
        let baseline = top - bounds.top;
        canvas.draw_str(&self.text, (left, baseline), &font, &paint);
    }
}

impl ViewNode for Text {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn intrinsic_size(&self, _ctx: &FrameCtx, text_style: &TextStyle) -> Option<(f32, f32)> {
        Some(self.measured_size(text_style))
    }

    fn draw(&self, _ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, text_style: &TextStyle) {
        self.draw_at(canvas, bounds.left, bounds.top, text_style);
    }
}
