use super::paint::paint_from_spec;
use super::path::path_from_encoded;
use super::{EngineDrawExecutor, EnginePreparedFrameMedia};
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_op::{DRRectSpec, Radii4};
use opencat_core::ir::draw_op::{DrawOp, LineCap as OpLineCap, LineJoin as OpLineJoin, PointMode};
use opencat_core::ir::draw_types::{DrawOpRange, PathOp, RuntimeEffectChildRef};
use opencat_core::platform::draw::DrawError;
use skia_safe::{
    Canvas, FilterMode, Paint, PathBuilder, Picture, PictureRecorder, Point, RRect, Rect, Shader,
    TileMode, Vector,
};

fn apply_global_alpha(paint: &mut Paint, alpha: f32) {
    let a = (paint.color().a() as f32 / 255.0) * alpha;
    paint.set_alpha((a * 255.0).round() as u8);
}

pub fn replay_frame(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
) -> Result<opencat_core::platform::draw::DrawStats, opencat_core::platform::draw::DrawError> {
    let stats = opencat_core::platform::draw::DrawStats {
        op_count: draw.ops.len() as u32,
        cache_hits: 0,
    };

    for op in &draw.ops {
        replay_op(exec, canvas, draw, media, op)?;
    }
    Ok(stats)
}

fn replay_range(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    range: DrawOpRange,
) -> Result<(), opencat_core::platform::draw::DrawError> {
    let start = range.start_op as usize;
    let end = start + range.op_len as usize;
    if end > draw.ops.len() {
        return Err(DrawError(format!(
            "ReplayRange out of bounds: {start}..{end} (len={})",
            draw.ops.len()
        )));
    }

    for op in &draw.ops[start..end] {
        replay_op(exec, canvas, draw, media, op)?;
    }
    Ok(())
}

fn rect_union(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.left.min(b.left),
        a.top.min(b.top),
        a.right.max(b.right),
        a.bottom.max(b.bottom),
    )
}

fn op_bounds(draw: &DrawOpFrame, op: &DrawOp) -> Option<Rect> {
    match op {
        DrawOp::SaveLayer {
            bounds: Some(rect), ..
        }
        | DrawOp::Rect { rect, .. }
        | DrawOp::RRect { rect, .. }
        | DrawOp::Oval { rect, .. }
        | DrawOp::Arc { rect, .. }
        | DrawOp::RuntimeEffect { dst: rect, .. }
        | DrawOp::ImageRect { dst: rect, .. } => Some(Rect::new(
            rect.x,
            rect.y,
            rect.x + rect.width,
            rect.y + rect.height,
        )),
        DrawOp::DRRect { outer, .. } => Some(Rect::new(
            outer.rect.x,
            outer.rect.y,
            outer.rect.x + outer.rect.width,
            outer.rect.y + outer.rect.height,
        )),
        DrawOp::Circle { cx, cy, radius, .. } => Some(Rect::new(
            cx - radius,
            cy - radius,
            cx + radius,
            cy + radius,
        )),
        DrawOp::Line { x0, y0, x1, y1, .. } => Some(Rect::new(
            x0.min(*x1),
            y0.min(*y1),
            x0.max(*x1),
            y0.max(*y1),
        )),
        DrawOp::ReplayRange { range } => range_bounds(draw, *range),
        _ => None,
    }
}

fn range_bounds(draw: &DrawOpFrame, range: DrawOpRange) -> Option<Rect> {
    let start = range.start_op as usize;
    let end = start.checked_add(range.op_len as usize)?;
    let ops = draw.ops.get(start..end)?;
    ops.iter()
        .filter_map(|op| op_bounds(draw, op))
        .reduce(rect_union)
}

fn picture_shader_for_range(
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    range: DrawOpRange,
    fallback_bounds: Rect,
) -> Result<Option<Shader>, opencat_core::platform::draw::DrawError> {
    let bounds = range_bounds(draw, range).unwrap_or(fallback_bounds);
    let mut recorder = PictureRecorder::new();
    let picture_canvas = recorder.begin_recording(bounds, false);
    let mut picture_exec = EngineDrawExecutor::new();
    picture_exec.begin_frame();
    replay_range(&mut picture_exec, picture_canvas, draw, media, range)?;
    let Some(picture): Option<Picture> = recorder.finish_recording_as_picture(Some(&bounds)) else {
        return Ok(None);
    };
    let shader = picture.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        FilterMode::Linear,
        None::<&skia_safe::Matrix>,
        Some(&bounds),
    );
    exec.compiled_pictures
        .insert(picture.unique_id() as u64, picture);
    Ok(Some(shader))
}

fn replay_op(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    op: &DrawOp,
) -> Result<(), opencat_core::platform::draw::DrawError> {
    match op {
        DrawOp::Save => {
            canvas.save();
            Ok(())
        }
        DrawOp::Restore => {
            canvas.restore();
            Ok(())
        }
        DrawOp::SaveLayer {
            bounds,
            paint,
            alpha,
        } => {
            let sk_paint = paint
                .as_ref()
                .map(|pid| paint_from_spec(&draw.paints[pid.0 as usize]));
            let sk_rect = bounds.map(|r| Rect::new(r.x, r.y, r.x + r.width, r.y + r.height));
            let mut rec = skia_safe::canvas::SaveLayerRec::default();
            if let Some(ref p) = sk_paint {
                rec = rec.paint(p);
            }
            if let Some(ref r) = sk_rect {
                rec = rec.bounds(r);
            }
            if sk_paint.is_none() {
                canvas.save_layer_alpha(sk_rect, (*alpha * 255.0) as u32);
            } else {
                canvas.save_layer(&rec);
            }
            Ok(())
        }
        DrawOp::RestoreToCount { count } => {
            canvas.restore_to_count(*count as usize);
            Ok(())
        }

        DrawOp::Translate { x, y } => {
            canvas.translate((*x, *y));
            Ok(())
        }
        DrawOp::Scale { x, y } => {
            canvas.scale((*x, *y));
            Ok(())
        }
        DrawOp::Rotate { degrees, cx, cy } => {
            canvas.rotate(*degrees, Some(Point::new(*cx, *cy)));
            Ok(())
        }
        DrawOp::Skew { sx, sy } => {
            canvas.skew((*sx, *sy));
            Ok(())
        }
        DrawOp::Concat { matrix } => {
            canvas.concat(&skia_safe::Matrix::new_all(
                matrix[0], matrix[3], matrix[6], matrix[1], matrix[4], matrix[7], matrix[2],
                matrix[5], matrix[8],
            ));
            Ok(())
        }

        DrawOp::SetFillStyle { color } => {
            let c = skia_safe::Color::from_argb(color.a, color.r, color.g, color.b);
            exec.current_fill_paint.set_color(c);
            Ok(())
        }
        DrawOp::SetStrokeStyle { color } => {
            let c = skia_safe::Color::from_argb(color.a, color.r, color.g, color.b);
            exec.current_stroke_paint.set_color(c);
            Ok(())
        }
        DrawOp::SetLineWidth { width } => {
            exec.current_stroke_paint.set_stroke_width(*width);
            Ok(())
        }
        DrawOp::SetLineCap { cap } => {
            exec.current_stroke_paint.set_stroke_cap(match cap {
                OpLineCap::Butt => skia_safe::paint::Cap::Butt,
                OpLineCap::Round => skia_safe::paint::Cap::Round,
                OpLineCap::Square => skia_safe::paint::Cap::Square,
            });
            Ok(())
        }
        DrawOp::SetLineJoin { join } => {
            exec.current_stroke_paint.set_stroke_join(match join {
                OpLineJoin::Miter => skia_safe::paint::Join::Miter,
                OpLineJoin::Round => skia_safe::paint::Join::Round,
                OpLineJoin::Bevel => skia_safe::paint::Join::Bevel,
            });
            Ok(())
        }
        DrawOp::SetLineDash { intervals, phase } => {
            let start = intervals.start as usize;
            let end = start + intervals.len as usize;
            if end > draw.f32_pool.len() {
                return Err(DrawError(format!("SetLineDash f32_pool out of bounds")));
            }
            let dash: Vec<f32> = draw.f32_pool[start..end].to_vec();
            if let Some(pe) = skia_safe::PathEffect::dash(&dash, *phase) {
                exec.current_stroke_paint.set_path_effect(pe);
            }
            Ok(())
        }
        DrawOp::ClearLineDash => {
            exec.current_stroke_paint
                .set_path_effect(None::<skia_safe::PathEffect>);
            Ok(())
        }
        DrawOp::SetGlobalAlpha { alpha } => {
            exec.current_alpha = alpha.clamp(0.0, 1.0);
            Ok(())
        }
        DrawOp::SetAntiAlias { enabled } => {
            exec.current_fill_paint.set_anti_alias(*enabled);
            exec.current_stroke_paint.set_anti_alias(*enabled);
            Ok(())
        }

        DrawOp::BeginPath => {
            exec.current_path = Some(PathBuilder::new());
            Ok(())
        }
        DrawOp::Path(path_op) => {
            if let Some(ref mut pb) = exec.current_path {
                apply_path_op(pb, path_op);
            }
            Ok(())
        }
        DrawOp::FillPath => {
            if let Some(mut pb) = exec.current_path.take() {
                let path = pb.detach();
                let mut paint = exec.current_fill_paint.clone();
                apply_global_alpha(&mut paint, exec.current_alpha);
                canvas.draw_path(&path, &paint);
                exec.current_path = Some(PathBuilder::new());
            }
            Ok(())
        }
        DrawOp::StrokePath => {
            if let Some(mut pb) = exec.current_path.take() {
                let path = pb.detach();
                let mut paint = exec.current_stroke_paint.clone();
                apply_global_alpha(&mut paint, exec.current_alpha);
                canvas.draw_path(&path, &paint);
                exec.current_path = Some(PathBuilder::new());
            }
            Ok(())
        }
        DrawOp::ClipPath { anti_alias } => {
            if let Some(mut pb) = exec.current_path.take() {
                let path = pb.detach();
                canvas.clip_path(&path, skia_safe::ClipOp::Intersect, *anti_alias);
                exec.current_path = Some(PathBuilder::new());
            }
            Ok(())
        }

        DrawOp::Clear { color } => {
            canvas.clear(skia_safe::Color4f {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            });
            Ok(())
        }
        DrawOp::Paint { paint: pid } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_paint(&paint);
            Ok(())
        }
        DrawOp::Rect { rect, paint: pid } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_rect(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                &paint,
            );
            Ok(())
        }
        DrawOp::RRect {
            rect,
            radii,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            let r = RRect::new_rect_radii(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                &radii4_to_vectors(radii),
            );
            canvas.draw_rrect(&r, &paint);
            Ok(())
        }
        DrawOp::DRRect {
            outer,
            inner,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            let o = drrect_to_skia(outer);
            let i = drrect_to_skia(inner);
            canvas.draw_drrect(&o, &i, &paint);
            Ok(())
        }
        DrawOp::Oval { rect, paint: pid } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_oval(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                &paint,
            );
            Ok(())
        }
        DrawOp::Circle {
            cx,
            cy,
            radius,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_circle((*cx, *cy), *radius, &paint);
            Ok(())
        }
        DrawOp::Arc {
            rect,
            start,
            sweep,
            use_center,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_arc(
                Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height),
                *start,
                *sweep,
                *use_center,
                &paint,
            );
            Ok(())
        }
        DrawOp::Line {
            x0,
            y0,
            x1,
            y1,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_line((*x0, *y0), (*x1, *y1), &paint);
            Ok(())
        }
        DrawOp::Points {
            mode,
            points,
            paint: pid,
        } => {
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            let start = points.start as usize;
            let end = start + points.len as usize;
            if end > draw.f32_pool.len() {
                return Err(DrawError(format!("Points f32_pool out of bounds")));
            }
            let pts: Vec<Point> = draw.f32_pool[start..end]
                .chunks(2)
                .map(|c| Point::new(c[0], c[1]))
                .collect();
            let sk_mode = match mode {
                PointMode::Points => skia_safe::canvas::PointMode::Points,
                PointMode::Lines => skia_safe::canvas::PointMode::Lines,
                PointMode::Polygon => skia_safe::canvas::PointMode::Polygon,
            };
            canvas.draw_points(sk_mode, &pts, &paint);
            Ok(())
        }
        DrawOp::DrawPath { path, paint: pid } => {
            let sk_path = path_from_encoded(&draw.paths[path.0 as usize]);
            let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
            apply_global_alpha(&mut paint, exec.current_alpha);
            canvas.draw_path(&sk_path, &paint);
            Ok(())
        }
        DrawOp::Image {
            image,
            x,
            y,
            paint: pid,
        } => {
            if let Some(idx) = media.image_index.get(image) {
                let sk_image = &media.images[*idx];
                if let Some(pid) = pid {
                    let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image(sk_image, (*x, *y), Some(&paint));
                } else {
                    canvas.draw_image(sk_image, (*x, *y), None);
                }
            }
            Ok(())
        }
        DrawOp::ImageRect {
            image,
            src,
            dst,
            paint: pid,
        } => {
            if let Some(idx) = media.image_index.get(image) {
                let sk_image = &media.images[*idx];
                let src_rect = src.map(|r| Rect::new(r.x, r.y, r.x + r.width, r.y + r.height));
                let dst_rect = Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                let src_arg = src_rect
                    .as_ref()
                    .map(|r| (r, skia_safe::canvas::SrcRectConstraint::Fast));
                if let Some(pid) = pid {
                    let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image_rect(sk_image, src_arg, dst_rect, &paint);
                } else {
                    let mut paint = Paint::default();
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image_rect(sk_image, src_arg, dst_rect, &paint);
                }
            }
            Ok(())
        }

        DrawOp::RuntimeEffect {
            effect,
            uniforms,
            children,
            dst,
        } => {
            let effect_idx = effect.0 as usize;
            if effect_idx < media.runtime_effects.len() {
                let rt = &media.runtime_effects[effect_idx];
                let uniform_idx = uniforms.0 as usize;
                if uniform_idx >= draw.byte_ranges.len() {
                    return Err(DrawError(format!(
                        "RuntimeEffect bytes_range out of bounds"
                    )));
                }
                let uniform_bytes = {
                    let start = draw.byte_ranges[uniform_idx].start as usize;
                    let len = draw.byte_ranges[uniform_idx].len as usize;
                    if start + len > draw.bytes.len() {
                        return Err(DrawError(format!("RuntimeEffect bytes out of bounds")));
                    }
                    &draw.bytes[start..start + len]
                };
                let mut inputs: Vec<skia_safe::runtime_effect::ChildPtr> = Vec::new();
                let child_start = children.start as usize;
                let child_end = child_start + children.len as usize;
                if child_end > draw.children.len() {
                    return Err(DrawError(format!("RuntimeEffect children out of bounds")));
                }
                for child_ref in &draw.children[child_start..child_end] {
                    match child_ref {
                        RuntimeEffectChildRef::Image(img_ref) => {
                            if let Some(idx) = media.image_index.get(img_ref) {
                                if let Some(shader) = skia_safe::shaders::image(
                                    &media.images[*idx],
                                    (skia_safe::TileMode::Clamp, skia_safe::TileMode::Clamp),
                                    &skia_safe::SamplingOptions::default(),
                                    None::<&skia_safe::Matrix>,
                                ) {
                                    inputs.push(shader.into());
                                } else {
                                    inputs.push(skia_safe::shaders::empty().into());
                                }
                            } else {
                                inputs.push(skia_safe::shaders::empty().into());
                            }
                        }
                        RuntimeEffectChildRef::Picture(range) => {
                            let fallback_bounds =
                                Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                            let shader = picture_shader_for_range(
                                exec,
                                draw,
                                media,
                                *range,
                                fallback_bounds,
                            )?
                            .unwrap_or_else(skia_safe::shaders::empty);
                            inputs.push(shader.into());
                        }
                        RuntimeEffectChildRef::Shader(_) => {
                            inputs.push(skia_safe::shaders::empty().into());
                        }
                    }
                }
                let uniform_data = skia_safe::Data::new_copy(uniform_bytes);
                let shader = rt.make_shader(uniform_data, &inputs, None::<&skia_safe::Matrix>);
                if let Some(s) = shader {
                    let mut paint = Paint::default();
                    paint.set_shader(Some(s));
                    canvas.draw_rect(
                        Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height),
                        &paint,
                    );
                }
            }
            Ok(())
        }

        DrawOp::ReplayRange { range } => replay_range(exec, canvas, draw, media, *range),
    }
}

fn apply_path_op(builder: &mut PathBuilder, op: &PathOp) {
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

fn radii4_to_vectors(r: &Radii4) -> [Vector; 4] {
    [
        Vector::new(r.top_left, r.top_left),
        Vector::new(r.top_right, r.top_right),
        Vector::new(r.bottom_right, r.bottom_right),
        Vector::new(r.bottom_left, r.bottom_left),
    ]
}

fn drrect_to_skia(spec: &DRRectSpec) -> RRect {
    let rect = Rect::new(
        spec.rect.x,
        spec.rect.y,
        spec.rect.x + spec.rect.width,
        spec.rect.y + spec.rect.height,
    );
    RRect::new_rect_radii(rect, &radii4_to_vectors(&spec.radii))
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencat_core::canvas::paint::{FillSpec, PaintSpec, PaintStyle};
    use opencat_core::ir::draw_op::Rect4;
    use opencat_core::ir::draw_types::{
        BytesRangeId, ChildRange, DrawOpRange, EffectId, EffectRef,
    };
    use skia_safe::{AlphaType, ColorType, ImageInfo, RuntimeEffect, image::CachingHint, surfaces};

    fn pixel_rgba(frame: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let index = (y * width + x) * 4;
        [
            frame[index],
            frame[index + 1],
            frame[index + 2],
            frame[index + 3],
        ]
    }

    #[test]
    fn runtime_effect_picture_child_samples_draw_op_range() {
        let sksl = r#"
uniform shader child;

half4 main(float2 coord) {
    return child.eval(coord);
}
"#;
        let rt = RuntimeEffect::make_for_shader(sksl, None).expect("runtime effect should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: 0xCAFE,
            sksl: sksl.to_string(),
        });
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: 0 });
        frame
            .children
            .push(RuntimeEffectChildRef::Picture(DrawOpRange {
                start_op: 0,
                op_len: 1,
            }));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 1.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });

        let effect_op = DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
        };

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface should create");
        let canvas = surface.canvas();

        replay_op(&mut exec, canvas, &frame, &media, &effect_op)
            .expect("runtime effect op should replay");

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 8 * 8 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            8 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        assert_eq!(
            pixel_rgba(&rgba, 8, 4, 4),
            [0, 255, 0, 255],
            "RuntimeEffect Picture child should sample the recorded draw range"
        );
    }
}
