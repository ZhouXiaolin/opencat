use opencat_core::draw::types::{EncodedPath, FillType, PathOp};
use skia_safe::{PathBuilder, PathFillType, RRect, Rect};

pub fn path_from_encoded(encoded: &EncodedPath) -> skia_safe::Path {
    let fill_type = match encoded.fill_type {
        FillType::Winding => PathFillType::Winding,
        FillType::EvenOdd => PathFillType::EvenOdd,
    };
    let mut builder = PathBuilder::new_with_fill_type(fill_type);

    for op in &encoded.ops {
        match *op {
            PathOp::MoveTo { x, y } => {
                builder.move_to((x, y));
            }
            PathOp::LineTo { x, y } => {
                builder.line_to((x, y));
            }
            PathOp::QuadTo { cx, cy, x, y } => {
                builder.quad_to((cx, cy), (x, y));
            }
            PathOp::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => {
                builder.cubic_to((c1x, c1y), (c2x, c2y), (x, y));
            }
            PathOp::Close => {
                builder.close();
            }
            PathOp::AddRect {
                x,
                y,
                width,
                height,
            } => {
                builder.add_rect(Rect::new(x, y, x + width, y + height), None, None);
            }
            PathOp::AddRRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                let r = RRect::new_rect_xy(Rect::new(x, y, x + width, y + height), radius, radius);
                builder.add_rrect(&r, None, None);
            }
            PathOp::AddOval {
                x,
                y,
                width,
                height,
            } => {
                builder.add_oval(Rect::new(x, y, x + width, y + height), None, None);
            }
            PathOp::AddArc {
                x,
                y,
                width,
                height,
                start_angle,
                sweep_angle,
            } => {
                builder.add_arc(
                    Rect::new(x, y, x + width, y + height),
                    start_angle,
                    sweep_angle,
                );
            }
        }
    }
    builder.detach()
}
