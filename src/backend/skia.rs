use anyhow::Result;
use skia_safe::{
    canvas::SrcRectConstraint, AlphaType, Canvas, ColorType, ImageInfo, Paint, PaintStyle, RRect,
    Rect,
};

use crate::{
    backend::skia_transition,
    display::list::{
        BitmapDisplayItem, DisplayCommand, DisplayItem, DisplayList, DisplayTransform,
        RectDisplayItem, TextDisplayItem,
    },
    layout::tree::LayoutRect,
    style::{ObjectFit, ShadowStyle, Transform},
    typography,
};

pub struct SkiaBackend<'a> {
    canvas: &'a Canvas,
    width: i32,
    height: i32,
}

impl<'a> SkiaBackend<'a> {
    pub fn new(canvas: &'a Canvas, width: i32, height: i32) -> Self {
        Self {
            canvas,
            width,
            height,
        }
    }

    pub fn execute(&mut self, list: &DisplayList) -> Result<()> {
        for command in &list.commands {
            self.execute_command(command)?;
        }
        Ok(())
    }

    fn execute_command(&mut self, command: &DisplayCommand) -> Result<()> {
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
            DisplayCommand::Transition { transition } => {
                skia_transition::draw_transition(self.canvas, transition, self.width, self.height)?;
            }
        }
        Ok(())
    }
}

fn draw_item(canvas: &Canvas, item: &DisplayItem) {
    match item {
        DisplayItem::Rect(rect) => draw_rect(canvas, rect),
        DisplayItem::Text(text) => draw_text(canvas, text),
        DisplayItem::Bitmap(bitmap) => draw_bitmap(canvas, bitmap),
    }
}

fn draw_rect(canvas: &Canvas, rect: &RectDisplayItem) {
    let style = &rect.paint;
    if style.background.is_none() && style.border_width.is_none() && style.shadow.is_none() {
        return;
    }

    let rect = layout_rect_to_skia(rect.bounds);

    // Draw shadow first (behind the rect)
    if let Some(shadow) = style.shadow {
        draw_shadow(canvas, rect, style.border_radius, shadow);
    }

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

fn draw_shadow(canvas: &Canvas, rect: Rect, radius: f32, shadow: ShadowStyle) {
    let (blur, offset_y) = match shadow {
        ShadowStyle::SM => (2.0, 1.0),
        ShadowStyle::MD => (4.0, 3.0),
        ShadowStyle::LG => (10.0, 6.0),
        ShadowStyle::XL => (20.0, 10.0),
    };

    let mut paint = Paint::default();
    paint.set_color(skia_safe::Color::from_argb(30, 0, 0, 0));
    paint.set_anti_alias(true);

    let shadow_rect = Rect::from_xywh(
        rect.left() - blur / 2.0,
        rect.top() + offset_y - blur / 2.0,
        rect.width() + blur,
        rect.height() + blur,
    );

    if radius > 0.0 {
        let rrect = RRect::new_rect_xy(shadow_rect, radius + blur / 2.0, radius + blur / 2.0);
        canvas.draw_rrect(rrect, &paint);
    } else {
        canvas.draw_rect(shadow_rect, &paint);
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

fn draw_bitmap(canvas: &Canvas, bitmap: &BitmapDisplayItem) {
    let info = ImageInfo::new(
        (bitmap.width as i32, bitmap.height as i32),
        ColorType::RGBA8888,
        AlphaType::Opaque,
        None,
    );

    let row_bytes = bitmap.width as usize * 4;
    let data = skia_safe::Data::new_copy(&bitmap.data);

    let image = skia_safe::images::raster_from_data(&info, data, row_bytes)
        .expect("failed to create image from bitmap data");

    let dst = layout_rect_to_skia(bitmap.bounds);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    match bitmap.object_fit {
        ObjectFit::Fill => {
            canvas.draw_image_rect(image, None, dst, &paint);
        }
        ObjectFit::Contain => {
            let fitted = fitted_rect(bitmap.width as f32, bitmap.height as f32, dst, false);
            canvas.draw_image_rect(image, None, fitted, &paint);
        }
        ObjectFit::Cover => {
            let src = cover_src_rect(bitmap.width as f32, bitmap.height as f32, dst);
            canvas.draw_image_rect(image, Some((&src, SrcRectConstraint::Strict)), dst, &paint);
        }
    }
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

fn fitted_rect(src_width: f32, src_height: f32, dst: Rect, cover: bool) -> Rect {
    let src_aspect = src_width / src_height;
    let dst_aspect = dst.width() / dst.height();

    let scale = if cover {
        if src_aspect > dst_aspect {
            dst.height() / src_height
        } else {
            dst.width() / src_width
        }
    } else if src_aspect > dst_aspect {
        dst.width() / src_width
    } else {
        dst.height() / src_height
    };

    let width = src_width * scale;
    let height = src_height * scale;
    let x = dst.left + (dst.width() - width) / 2.0;
    let y = dst.top + (dst.height() - height) / 2.0;

    Rect::from_xywh(x, y, width, height)
}

fn cover_src_rect(src_width: f32, src_height: f32, dst: Rect) -> Rect {
    let fitted = fitted_rect(src_width, src_height, dst, true);
    let scale = fitted.width() / src_width;
    let visible_width = dst.width() / scale;
    let visible_height = dst.height() / scale;
    let x = (src_width - visible_width) / 2.0;
    let y = (src_height - visible_height) / 2.0;

    Rect::from_xywh(x, y, visible_width, visible_height)
}
