use std::sync::Arc;

use skia_safe::{Canvas, Color, Font, FontMgr, FontStyle, Paint, Rect};

use crate::{FrameCtx, Node, ViewNode, view::IntoNode};

pub struct Text {
    text: String,
    color: Color,
    font_size: f32,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: Color::BLACK,
            font_size: 16.0,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    fn make_font(&self) -> Font {
        let font_mgr = FontMgr::new();
        if let Some(typeface) = font_mgr.legacy_make_typeface(None, FontStyle::normal()) {
            Font::new(typeface, self.font_size)
        } else {
            let mut font = Font::default();
            font.set_size(self.font_size);
            font
        }
    }
}

impl ViewNode for Text {
    fn intrinsic_size(&self, _ctx: &FrameCtx) -> Option<(f32, f32)> {
        let font = self.make_font();
        let (width, bounds) = font.measure_str(&self.text, None);
        let height = bounds.height().max(self.font_size);
        Some((width.max(1.0), height.max(1.0)))
    }

    fn draw(&self, _ctx: &FrameCtx, canvas: &Canvas, bounds: Rect) {
        let mut paint = Paint::default();
        paint.set_color(self.color);
        paint.set_anti_alias(true);

        let font = self.make_font();
        let baseline = bounds.top + font.size();
        canvas.draw_str(&self.text, (bounds.left, baseline), &font, &paint);
    }
}

impl IntoNode for Text {
    fn into_node(self) -> Node {
        Arc::new(self)
    }
}
