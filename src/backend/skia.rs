use anyhow::Result;
use skia_safe::{Canvas, Paint, PaintStyle, RRect, Rect};

use crate::{
    display::list::{
        DisplayCommand, DisplayItem, DisplayList, DisplayTransform, RectDisplayItem,
        TextDisplayItem,
    },
    layout::tree::LayoutRect,
    style::Transform,
    typography,
};

pub struct SkiaBackend<'a> {
    canvas: &'a Canvas,
}

impl<'a> SkiaBackend<'a> {
    pub fn new(canvas: &'a Canvas) -> Self {
        Self { canvas }
    }

    pub fn execute(&mut self, list: &DisplayList) -> Result<()> {
        for command in &list.commands {
            self.execute_command(command);
        }
        Ok(())
    }

    fn execute_command(&mut self, command: &DisplayCommand) {
        match command {
            DisplayCommand::Save => {
                self.canvas.save();
            }
            DisplayCommand::Restore => {
                self.canvas.restore();
            }
            DisplayCommand::SaveLayer { layer } => {
                let alpha = (layer.opacity * 255.0).round() as u32;
                self.canvas
                    .save_layer_alpha(layout_rect_to_skia(layer.bounds), alpha);
            }
            DisplayCommand::ApplyTransform { transform } => {
                apply_transform(self.canvas, transform);
            }
            DisplayCommand::Draw { item } => {
                draw_item(self.canvas, item);
            }
        }
    }
}

fn draw_item(canvas: &Canvas, item: &DisplayItem) {
    match item {
        DisplayItem::Rect(rect) => draw_rect(canvas, rect),
        DisplayItem::Text(text) => draw_text(canvas, text),
    }
}

fn draw_rect(canvas: &Canvas, rect: &RectDisplayItem) {
    let style = &rect.paint;
    if style.background.is_none() && style.border_width.is_none() {
        return;
    }

    let rect = layout_rect_to_skia(rect.bounds);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    if style.border_radius > 0.0 {
        let rrect = RRect::new_rect_xy(rect, style.border_radius, style.border_radius);

        if let Some(color) = style.background {
            paint.set_color(color.to_skia());
            canvas.draw_rrect(rrect, &paint);
        }

        if let (Some(width), Some(color)) = (style.border_width, style.border_color) {
            paint.set_color(color.to_skia());
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            canvas.draw_rrect(rrect, &paint);
        }
    } else {
        if let Some(color) = style.background {
            paint.set_color(color.to_skia());
            canvas.draw_rect(rect, &paint);
        }

        if let (Some(width), Some(color)) = (style.border_width, style.border_color) {
            paint.set_color(color.to_skia());
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(width);
            canvas.draw_rect(rect, &paint);
        }
    }
}

fn draw_text(canvas: &Canvas, text: &TextDisplayItem) {
    typography::draw_text(
        canvas,
        &text.text,
        text.bounds.x,
        text.bounds.y,
        &text.style,
    );
}

fn apply_transform(canvas: &Canvas, transform: &DisplayTransform) {
    canvas.translate((transform.translation_x, transform.translation_y));
    if transform.transforms.is_empty() {
        return;
    }

    let rect = layout_rect_to_skia(transform.bounds);
    let center_x = rect.width() / 2.0;
    let center_y = rect.height() / 2.0;

    for transform in transform.transforms.iter().rev() {
        match *transform {
            Transform::TranslateX(x) => {
                canvas.translate((x, 0.0));
            }
            Transform::TranslateY(y) => {
                canvas.translate((0.0, y));
            }
            Transform::Translate(x, y) => {
                canvas.translate((x, y));
            }
            Transform::Scale(value) => {
                canvas.translate((center_x, center_y));
                canvas.scale((value, value));
                canvas.translate((-center_x, -center_y));
            }
            Transform::ScaleX(value) => {
                canvas.translate((center_x, center_y));
                canvas.scale((value, 1.0));
                canvas.translate((-center_x, -center_y));
            }
            Transform::ScaleY(value) => {
                canvas.translate((center_x, center_y));
                canvas.scale((1.0, value));
                canvas.translate((-center_x, -center_y));
            }
            Transform::RotateDeg(deg) => {
                canvas.rotate(deg, Some((center_x, center_y).into()));
            }
            Transform::SkewXDeg(deg) => {
                canvas.translate((center_x, center_y));
                canvas.skew((deg.to_radians().tan(), 0.0));
                canvas.translate((-center_x, -center_y));
            }
            Transform::SkewYDeg(deg) => {
                canvas.translate((center_x, center_y));
                canvas.skew((0.0, deg.to_radians().tan()));
                canvas.translate((-center_x, -center_y));
            }
            Transform::SkewDeg(x_deg, y_deg) => {
                canvas.translate((center_x, center_y));
                canvas.skew((x_deg.to_radians().tan(), y_deg.to_radians().tan()));
                canvas.translate((-center_x, -center_y));
            }
        }
    }
}

fn layout_rect_to_skia(rect: LayoutRect) -> Rect {
    Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}
