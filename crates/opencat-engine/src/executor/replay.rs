use super::paint::paint_from_spec;
use super::path::path_from_encoded;
use super::{DrawError, DrawStats};
use super::{EngineDrawExecutor, EnginePreparedFrameMedia};
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_op::{DRRectSpec, Radii4};
use opencat_core::ir::draw_op::{DrawOp, LineCap as OpLineCap, LineJoin as OpLineJoin, PointMode};
use opencat_core::ir::draw_types::{DrawOpRange, ImageRef, PathOp, RuntimeEffectChildRef, SubtreeId};
use skia_safe::{
    Canvas, CubicResampler, FilterMode, MipmapMode, Paint, PathBuilder, Picture, PictureRecorder,
    Point, RRect, Rect, SamplingOptions, Shader, TileMode, Vector,
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
) -> Result<DrawStats, DrawError> {
    let stats = DrawStats {
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
) -> Result<(), DrawError> {
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
        DrawOp::ReplaySubtreePicture { subtree, .. } => subtree_bounds(draw, *subtree),
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

fn replay_subtree(
    exec: &mut EngineDrawExecutor,
    canvas: &Canvas,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    subtree: SubtreeId,
) -> Result<(), DrawError> {
    let Some(ops) = draw.subtrees.get(subtree.0 as usize) else {
        return Err(DrawError(format!(
            "Subtree out of bounds: {} (len={})",
            subtree.0,
            draw.subtrees.len()
        )));
    };
    for op in ops {
        replay_op(exec, canvas, draw, media, op)?;
    }
    Ok(())
}

fn subtree_bounds(draw: &DrawOpFrame, subtree: SubtreeId) -> Option<Rect> {
    let ops = draw.subtrees.get(subtree.0 as usize)?;
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
) -> Result<Option<Shader>, DrawError> {
    // RuntimeEffect picture children are sampled in the destination shader
    // space (`dst`/`xy`), not in a tight local content box. If we pass only
    // the recorded ops' minimal bounds here, Skia will clamp sampling to that
    // narrow strip and the rest of the destination will smear the edge pixel.
    // Keep the picture shader aligned to at least the destination bounds while
    // still expanding to include any recorded content that spills outside it.
    let bounds = range_bounds(draw, range)
        .map(|recorded| rect_union(recorded, fallback_bounds))
        .unwrap_or(fallback_bounds);
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

fn picture_shader_for_subtree(
    exec: &mut EngineDrawExecutor,
    draw: &DrawOpFrame,
    media: &EnginePreparedFrameMedia,
    subtree: SubtreeId,
    fallback_bounds: Rect,
) -> Result<Option<Shader>, DrawError> {
    let bounds = subtree_bounds(draw, subtree).unwrap_or(fallback_bounds);
    let mut recorder = PictureRecorder::new();
    let picture_canvas = recorder.begin_recording(bounds, false);
    let mut picture_exec = EngineDrawExecutor::new();
    picture_exec.begin_frame();
    replay_subtree(&mut picture_exec, picture_canvas, draw, media, subtree)?;
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
) -> Result<(), DrawError> {
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
            let sk_rect = bounds.map(|r| Rect::new(r.x, r.y, r.x + r.width, r.y + r.height));
            match paint {
                Some(pid) => {
                    // The layer alpha must compose with the paint (it used to
                    // be dropped whenever a paint was present, which made
                    // filtered layers render fully opaque).
                    let mut sk_paint = paint_from_spec(&draw.paints[pid.0 as usize]);
                    if *alpha < 1.0 {
                        apply_global_alpha(&mut sk_paint, *alpha);
                    }
                    let rec = skia_safe::canvas::SaveLayerRec::default().paint(&sk_paint);
                    let rec = match sk_rect {
                        Some(ref r) => rec.bounds(r),
                        None => rec,
                    };
                    canvas.save_layer(&rec);
                }
                None => {
                    canvas.save_layer_alpha(sk_rect, (*alpha * 255.0) as u32);
                }
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
                // r6: ImageRect 重采样质量。实测（examples/codex-five.xml 的 fig 1.2×
                // 放大 vs 参考视频）：Chrome 对 CSS 缩放的 video 元素用高质量 cubic
                // 重建，Mitchell(B=1/3,C=1/3) 把 fig 躯干窗 p8 从 10.0% 压到 3.9%、
                // p32 从 1.12% 压到 0.01%（catmull_rom 5.8%，nearest/旧默认 10%）。
                // 因此 VideoFrame（声明式 <video>，媒体路径）默认 = Mitchell cubic；
                // Static（JS canvas drawImageRect 的像素精确语义）保持 Skia 默认。
                // OPENCAT_IMG_SAMPLING=catmull|nearest|linear|legacy 仅供调试回退
                // （作用于 VideoFrame 路径）。
                let sampling = match std::env::var("OPENCAT_IMG_SAMPLING").as_deref() {
                    Ok("catmull") => SamplingOptions {
                        use_cubic: true,
                        cubic: CubicResampler::catmull_rom(),
                        ..SamplingOptions::default()
                    },
                    Ok("nearest") => SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
                    Ok("linear") => SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                    _ => match image {
                        ImageRef::VideoFrame { .. } => SamplingOptions {
                            use_cubic: true,
                            cubic: CubicResampler::mitchell(),
                            ..SamplingOptions::default()
                        },
                        ImageRef::Static { .. } | ImageRef::Generated { .. } => {
                            SamplingOptions::default()
                        }
                    },
                };
                if let Some(pid) = pid {
                    let mut paint = paint_from_spec(&draw.paints[pid.0 as usize]);
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image_rect_with_sampling_options(
                        sk_image,
                        src_arg,
                        dst_rect,
                        sampling,
                        &paint,
                    );
                } else {
                    let mut paint = Paint::default();
                    apply_global_alpha(&mut paint, exec.current_alpha);
                    canvas.draw_image_rect_with_sampling_options(
                        sk_image,
                        src_arg,
                        dst_rect,
                        sampling,
                        &paint,
                    );
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
                                // 约定（skill canvaskit §Effect lambda）：image
                                // child 的 tile 空间原点 = effect dst 本地原点，
                                // 与 `uv = xy - rect.xy` 对齐 —— 用本地矩阵平移
                                // dst.xy，使 child.eval(uv) 取到"该矩形自己的"
                                // 像素（与 lambda CPU 后端 ChildImage 采样一致）。
                                let mut local =
                                    skia_safe::Matrix::new_identity();
                                local.set_translate((-dst.x, -dst.y));
                                if let Some(shader) = skia_safe::shaders::image(
                                    &media.images[*idx],
                                    (skia_safe::TileMode::Clamp, skia_safe::TileMode::Clamp),
                                    &skia_safe::SamplingOptions::default(),
                                    Some(&local),
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
                        RuntimeEffectChildRef::SubtreePicture(subtree) => {
                            let fallback_bounds =
                                Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                            let shader = picture_shader_for_subtree(
                                exec,
                                draw,
                                media,
                                *subtree,
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

        DrawOp::ReplaySubtreePicture { subtree, x, y } => {
            canvas.save();
            canvas.translate(Vector::new(*x, *y));
            let result = replay_subtree(exec, canvas, draw, media, *subtree);
            canvas.restore();
            result
        }

        DrawOp::DrawSubtreePicture { .. } => Ok(()),

        DrawOp::ScriptRuntimeEffect { .. } => Ok(()),

        DrawOp::LottieRect {
            bundle_id,
            frame,
            dst,
        } => {
            #[cfg(not(target_os = "macos"))]
            if let Some(anim) = exec.lottie_cache.get(bundle_id) {
                anim.seek_frame(*frame as f64);
                let size = anim.size();
                let iw = size.width.max(1.0);
                let ih = size.height.max(1.0);
                canvas.save();
                let clip_rect = Rect::new(dst.x, dst.y, dst.x + dst.width, dst.y + dst.height);
                canvas.clip_rect(clip_rect, None, Some(false));
                canvas.translate((dst.x, dst.y));
                canvas.scale((dst.width / iw, dst.height / ih));
                anim.render(canvas, None);
                canvas.restore();
            }
            #[cfg(target_os = "macos")]
            {
                // No native Skottie renderer (feature off); skip the op.
                let _ = (frame, dst);
            }
            Ok(())
        }
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
    use opencat_core::canvas::paint::{FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
    use opencat_core::ir::draw_op::Rect4;
    use opencat_core::ir::draw_types::{
        BytesRangeId, ChildRange, DrawOpRange, EffectId, EffectRef,
    };
    use skia_safe::{
        AlphaType, ColorType, Data, ImageInfo, RuntimeEffect, images,
        image::CachingHint, surfaces,
    };

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
    fn canvas_draw_script_bounds_clip_then_script_clip_keeps_content() {
        // Mirror `render_draw_script`: it first clips the canvas to the element
        // bounds, then the script's own clip follows. Content drawn after both
        // clips must still land in the intersection.
        let mut frame = DrawOpFrame::default();
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        // 1) render_draw_script's own bounds clip (8x8 element)
        frame.ops.push(DrawOp::Save);
        frame.ops.push(DrawOp::BeginPath);
        frame.ops.push(DrawOp::Path(PathOp::AddRect {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 8.0,
        }));
        frame.ops.push(DrawOp::ClipPath { anti_alias: true });
        // 2) the script's own triangle clip (left half)
        frame.ops.push(DrawOp::Save);
        frame.ops.push(DrawOp::BeginPath);
        frame.ops.push(DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 8.0 }));
        frame.ops.push(DrawOp::Path(PathOp::Close));
        frame.ops.push(DrawOp::ClipPath { anti_alias: true });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });
        frame.ops.push(DrawOp::Restore);
        frame.ops.push(DrawOp::Restore);

        let media = EnginePreparedFrameMedia::default();
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface should create");
        let canvas = surface.canvas();
        for op in &frame.ops {
            replay_op(&mut exec, canvas, &frame, &media, op)
                .expect("clip ops should replay");
        }

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

        // (3,4) lies strictly inside the triangle (the hypotenuse passes
        // through x=2 at y=4, so sampling there would measure a correct
        // ~75%-coverage anti-aliased edge instead of full paint).
        let inside = pixel_rgba(&rgba, 8, 3, 4);
        let outside = pixel_rgba(&rgba, 8, 6, 4);
        assert!(
            inside[0] > 200 && inside[3] > 200,
            "content inside both clips must be painted, got {inside:?}"
        );
        assert_eq!(
            outside[3], 0,
            "content outside the script clip must be culled, got {outside:?}"
        );
    }

    #[test]
    fn canvas_clip_coverage_matrix_dump() {
        // Diagnostic: dump center-row pixel coverage for clip variants.
        let variants: Vec<(&str, bool, bool)> = vec![
            // (name, include_bounds_clip, triangle_clip_aa)
            ("clip-only-aa", false, true),
            ("clip-only-noaa", false, false),
            ("bounds+clip-aa", true, true),
            ("bounds+clip-noaa", true, false),
        ];
        for (name, with_bounds, tri_aa) in variants {
            let mut frame = DrawOpFrame::default();
            frame.paints.push(PaintSpec {
                fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
                style: PaintStyle::Fill,
                ..Default::default()
            });
            frame.ops.push(DrawOp::Save);
            if with_bounds {
                frame.ops.push(DrawOp::BeginPath);
                frame.ops.push(DrawOp::Path(PathOp::AddRect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                }));
                frame.ops.push(DrawOp::ClipPath { anti_alias: false });
            }
            frame.ops.push(DrawOp::BeginPath);
            frame.ops.push(DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 }));
            frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 0.0 }));
            frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 8.0 }));
            frame.ops.push(DrawOp::Path(PathOp::Close));
            frame.ops.push(DrawOp::ClipPath { anti_alias: tri_aa });
            frame.ops.push(DrawOp::Rect {
                rect: Rect4 { x: 0.0, y: 0.0, width: 8.0, height: 8.0 },
                paint: opencat_core::ir::draw_types::PaintId(0),
            });
            frame.ops.push(DrawOp::Restore);

            let media = EnginePreparedFrameMedia::default();
            let mut exec = EngineDrawExecutor::new();
            exec.begin_frame();
            let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface");
            let canvas = surface.canvas();
            for op in &frame.ops {
                replay_op(&mut exec, canvas, &frame, &media, op).unwrap();
            }
            let image = surface.image_snapshot();
            let info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
            let mut rgba = vec![0_u8; 8 * 8 * 4];
            assert!(image.read_pixels(&info, rgba.as_mut_slice(), 8 * 4, (0, 0), CachingHint::Allow));
            let row: Vec<u8> = (0..8).map(|x| pixel_rgba(&rgba, 8, x, 4)[3]).collect();
            eprintln!("clip-matrix {name}: alpha row4 = {row:?}");
        }
    }

    #[test]
    fn canvas_clip_path_intersects_subsequent_draws() {
        // Repro for the codex-five bootstrap: a script canvas that builds a
        // path, clips to it, then draws — the draw must land inside the clip.
        let mut frame = DrawOpFrame::default();
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        // Clip to the left half of the 8x8 canvas.
        frame.ops.push(DrawOp::BeginPath);
        frame.ops.push(DrawOp::Path(PathOp::MoveTo { x: 0.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 0.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 4.0, y: 8.0 }));
        frame.ops.push(DrawOp::Path(PathOp::LineTo { x: 0.0, y: 8.0 }));
        frame.ops.push(DrawOp::Path(PathOp::Close));
        frame.ops.push(DrawOp::ClipPath { anti_alias: true });
        // Fill the whole canvas red; only the left half may receive paint.
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 8.0,
                height: 8.0,
            },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });

        let media = EnginePreparedFrameMedia::default();
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface should create");
        let canvas = surface.canvas();
        for op in &frame.ops {
            replay_op(&mut exec, canvas, &frame, &media, op)
                .expect("clip ops should replay");
        }

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

        let inside = pixel_rgba(&rgba, 8, 2, 4);
        let outside = pixel_rgba(&rgba, 8, 6, 4);
        assert!(
            inside[0] > 200 && inside[3] > 200,
            "draw inside the clip must be painted, got {inside:?}"
        );
        assert_eq!(
            outside[3], 0,
            "draw outside the clip must be culled, got {outside:?}"
        );
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

    #[test]
    fn runtime_effect_picture_child_respects_translated_range_bounds() {
        let sksl = r#"
uniform shader child;

half4 main(float2 coord) {
    return child.eval(coord);
}
"#;
        let rt = RuntimeEffect::make_for_shader(sksl, None).expect("runtime effect should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: 0xBEEF,
            sksl: sksl.to_string(),
        });
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: 0 });
        frame
            .children
            .push(RuntimeEffectChildRef::Picture(DrawOpRange {
                start_op: 0,
                op_len: 2,
            }));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 1.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Translate { x: 4.0, y: 2.0 });
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
                width: 16.0,
                height: 16.0,
            },
        };

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((16, 16)).expect("surface should create");
        let canvas = surface.canvas();

        replay_op(&mut exec, canvas, &frame, &media, &effect_op)
            .expect("runtime effect op should replay");

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((16, 16), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 16 * 16 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            16 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        assert_eq!(
            pixel_rgba(&rgba, 16, 6, 4),
            [0, 255, 0, 255],
            "Picture child shader should preserve translated content bounds"
        );
        assert_eq!(
            pixel_rgba(&rgba, 16, 1, 1),
            [0, 0, 0, 0],
            "Pixels outside the translated picture should remain transparent"
        );
    }

    #[test]
    fn runtime_effect_picture_child_samples_content_translated_outside_local_op_bounds() {
        let sksl = r#"
uniform shader child;

half4 main(float2 coord) {
    return child.eval(coord);
}
"#;
        let rt = RuntimeEffect::make_for_shader(sksl, None).expect("runtime effect should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef {
            hash: 0xFACE,
            sksl: sksl.to_string(),
        });
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: 0 });
        frame
            .children
            .push(RuntimeEffectChildRef::Picture(DrawOpRange {
                start_op: 0,
                op_len: 2,
            }));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 1.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Translate { x: 12.0, y: 4.0 });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 4.0,
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
                width: 20.0,
                height: 12.0,
            },
        };

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((20, 12)).expect("surface should create");
        let canvas = surface.canvas();

        replay_op(&mut exec, canvas, &frame, &media, &effect_op)
            .expect("runtime effect op should replay");

        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((20, 12), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 20 * 12 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            20 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        assert_eq!(
            pixel_rgba(&rgba, 20, 13, 5),
            [0, 255, 0, 255],
            "Picture child shader should sample translated content even when it lies outside the local op bounds"
        );
        assert_eq!(
            pixel_rgba(&rgba, 20, 1, 1),
            [0, 0, 0, 0],
            "Pixels outside the translated content should remain transparent"
        );
    }

    /// CSS chained text-shadow semantics for the codex-five bootstrap: nested
    /// `SaveLayer`s with `keep_content: true` DropShadow filters must compound
    /// — the inner layer's output (shadow 1 + content) is the input of the
    /// outer shadow filter, so BOTH shadow lobes appear around the body. A
    /// regression that dropped the inner output (or kept `drop_shadow_only`)
    /// would lose one lobe.
    #[test]
    fn nested_keep_content_shadow_layers_compound() {
        use skia_safe::surfaces;

        fn keep_shadow_paint(dx: f32, dy: f32, sigma: f32) -> PaintSpec {
            PaintSpec {
                fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
                style: PaintStyle::Fill,
                anti_alias: true,
                blend_mode: opencat_core::canvas::paint::BlendMode::SrcOver,
                image_filter: Some(ImageFilterSpec::DropShadow {
                    dx,
                    dy,
                    sigma_x: sigma,
                    sigma_y: sigma,
                    color: [0.0, 0.0, 0.0, 1.0],
                    keep_content: true,
                }),
                ..Default::default()
            }
        }

        // 64x48 white canvas; content = small opaque square in the middle.
        // Shadow A offset (+24, 0), shadow B offset (-24, 0), sigma 4 each.
        let mut frame = DrawOpFrame::default();
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([1.0, 1.0, 1.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.paints.push(keep_shadow_paint(24.0, 0.0, 4.0));
        frame.paints.push(keep_shadow_paint(-24.0, 0.0, 4.0));
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.2, 0.2, 0.2, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 { x: 0.0, y: 0.0, width: 64.0, height: 48.0 },
            paint: opencat_core::ir::draw_types::PaintId(0),
        });
        frame.ops.push(DrawOp::SaveLayer {
            bounds: None,
            paint: Some(opencat_core::ir::draw_types::PaintId(1)),
            alpha: 1.0,
        });
        frame.ops.push(DrawOp::SaveLayer {
            bounds: None,
            paint: Some(opencat_core::ir::draw_types::PaintId(2)),
            alpha: 1.0,
        });
        frame.ops.push(DrawOp::Rect {
            rect: Rect4 { x: 28.0, y: 18.0, width: 8.0, height: 12.0 },
            paint: opencat_core::ir::draw_types::PaintId(3),
        });
        frame.ops.push(DrawOp::Restore);
        frame.ops.push(DrawOp::Restore);

        let media = EnginePreparedFrameMedia::default();
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((64, 48)).expect("surface");
        let canvas = surface.canvas();
        for op in &frame.ops {
            replay_op(&mut exec, canvas, &frame, &media, op).expect("shadow chain replay");
        }
        let image = surface.image_snapshot();
        let info = ImageInfo::new((64, 48), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 64 * 48 * 4];
        assert!(image.read_pixels(&info, rgba.as_mut_slice(), 64 * 4, (0, 0), CachingHint::Allow));

        fn luminance(rgba: &[u8], w: usize, x: usize, y: usize) -> u8 {
            let i = (y * w + x) * 4;
            rgba[i]
        }
        // Content square itself.
        let body = luminance(&rgba, 64, 32, 24);
        assert!(body < 80, "body square must be drawn, got {body}");
        // Right lobe (shadow A at +24): x≈58, y=24 must be darkened.
        let right = luminance(&rgba, 64, 57, 24);
        assert!(right < 220, "chained outer shadow lobe (+24) missing, got {right}");
        // Left lobe (shadow B at -24): x≈6, y=24 must be darkened — this only
        // happens when the inner keep_content output feeds the outer filter.
        let left = luminance(&rgba, 64, 6, 24);
        assert!(left < 220, "chained inner shadow lobe (-24) missing, got {left}");
    }

    /// lambda → SKSL → RuntimeEffect 的 raster 端到端：水平渐变亮度递增，
    /// 且 u_oc_rect 偏移生效（dst 从 (4,2) 起绘制，内部坐标仍从 0 数起）。
    #[test]
    fn lambda_sksl_gradient_raster_e2e() {
        let source = "(uv, u) => { const g = uv.x / u.w; return [g, g, g, 1]; }";
        let mut spec = opencat_core::script::effects_lambda::EffectSpec::default();
        spec.uniforms
            .push(("w".to_string(), opencat_core::script::effects_lambda::program::Ty::Float));
        let compiled =
            opencat_core::script::effects_lambda::compile_effect(source, &spec)
                .expect("lambda should compile");
        assert_eq!(
            compiled.backend,
            opencat_core::script::effects_lambda::Backend::Sksl
        );
        let sksl = compiled.sksl.expect("sksl backend carries code");

        let rt = RuntimeEffect::make_for_shader(&sksl, None).expect("generated SKSL should compile");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef { hash: compiled.hash, sksl: sksl.clone() });
        // uniforms: w=8.0 f32，随后 u_oc_rect = dst(0,0,8,8)
        let mut uniform_bytes = Vec::new();
        for v in [8.0f32, 0.0, 0.0, 8.0, 8.0] {
            uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        frame.bytes.extend_from_slice(&uniform_bytes);
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: uniform_bytes.len() as u32 });
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 { x: 0.0, y: 0.0, width: 8.0, height: 8.0 },
        });

        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface");
        let canvas = surface.canvas();
        replay_op(
            &mut exec,
            canvas,
            &frame,
            &media,
            &frame.ops[0],
        )
        .expect("lambda runtime effect should replay");

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
        // 水平亮度单调递增（premul 但 alpha=1）；byte index = col*4（row 0）
        let lum = |col: usize| rgba[col * 4];
        assert!(lum(0) < lum(3) && lum(3) < lum(6), "gradient not increasing: {} {} {}", lum(0), lum(3), lum(6));
    }

    /// 后端一致性：同一 float-only lambda，SKSL（skia raster）与 f64 解释器
    /// 输出逐通道容差比对。
    #[test]
    fn lambda_backend_consistency_sksl_vs_interpreter() {
        let source =
            "(uv, u) => { const g = smoothstep(0.0, u.w, uv.x); return [g, 0.5, 1 - g, 1]; }";
        let mut spec = opencat_core::script::effects_lambda::EffectSpec::default();
        spec.uniforms
            .push(("w".to_string(), opencat_core::script::effects_lambda::program::Ty::Float));

        // CPU（解释器）
        let mut cpu_spec = spec.clone();
        cpu_spec.backend_override =
            Some(opencat_core::script::effects_lambda::Backend::Cpu);
        let cpu = opencat_core::script::effects_lambda::compile_effect(source, &cpu_spec)
            .expect("cpu compile");
        let ctx = opencat_core::script::effects_lambda::interp::InterpCtx {
            uniforms: &[opencat_core::script::effects_lambda::program::Val::F(8.0)],
            rect: [0.0, 0.0, 8.0, 8.0],
            children: &[],
            scan: None,
        };
        let cpu_rgba =
            opencat_core::script::effects_lambda::interp::render(&cpu.program, 8, 8, &ctx);

        // SKSL（skia raster）
        let mut sksl_spec = spec.clone();
        sksl_spec.backend_override =
            Some(opencat_core::script::effects_lambda::Backend::Sksl);
        let sksl_c = opencat_core::script::effects_lambda::compile_effect(source, &sksl_spec)
            .expect("sksl compile");
        let rt = RuntimeEffect::make_for_shader(sksl_c.sksl.as_deref().unwrap(), None)
            .expect("generated SKSL should compile");
        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef { hash: sksl_c.hash, sksl: sksl_c.sksl.clone().unwrap() });
        let mut uniform_bytes = Vec::new();
        for v in [8.0f32, 0.0, 0.0, 8.0, 8.0] {
            uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        frame.bytes.extend_from_slice(&uniform_bytes);
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: uniform_bytes.len() as u32 });
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 0 },
            dst: Rect4 { x: 0.0, y: 0.0, width: 8.0, height: 8.0 },
        });
        let media = EnginePreparedFrameMedia {
            runtime_effects: vec![rt],
            ..Default::default()
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((8, 8)).expect("surface");
        replay_op(&mut exec, surface.canvas(), &frame, &media, &frame.ops[0]).expect("replay");
        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((8, 8), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut sksl_rgba = vec![0_u8; 8 * 8 * 4];
        assert!(image.read_pixels(
            &image_info,
            sksl_rgba.as_mut_slice(),
            8 * 4,
            (0, 0),
            CachingHint::Allow,
        ));

        for i in 0..8 * 8 * 4 {
            let (a, b) = (cpu_rgba[i], sksl_rgba[i]);
            assert!(
                (a as i32 - b as i32).abs() <= 3,
                "px {i}: cpu {a} vs sksl {b}"
            );
        }
    }

    /// lambda → SKSL 的 generated image child：child.eval(uv) 以 dst 本地
    /// 坐标取像素（tile 原点 = dst 原点）。
    #[test]
    fn lambda_sksl_generated_child_samples_local_space() {
        use opencat_core::ir::GeneratedImageId;

        let source =
            "(uv, tex, u) => { const c = tex.eval(uv); return [c.r, c.g, c.b, 1]; }";
        let mut spec = opencat_core::script::effects_lambda::EffectSpec::default();
        let compiled =
            opencat_core::script::effects_lambda::compile_effect(source, &spec)
                .expect("lambda should compile");
        let sksl = compiled.sksl.expect("sksl");
        let rt = RuntimeEffect::make_for_shader(&sksl, None).expect("compile");

        // 2×1 child：红、绿两像素（straight）
        let child_rgba: Vec<u8> = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let gid = GeneratedImageId(0x1A2B);
        let child_info =
            ImageInfo::new((2, 1), ColorType::RGBA8888, AlphaType::Unpremul, None);
        let child_img = images::raster_from_data(
            &child_info,
            Data::new_copy(&child_rgba),
            2 * 4,
        )
        .expect("child image should decode");

        let mut frame = DrawOpFrame::default();
        frame.effects.push(EffectRef { hash: compiled.hash, sksl });
        frame.children.push(RuntimeEffectChildRef::Image(ImageRef::Generated { id: gid }));
        // u_oc_rect = dst(0,0,2,1)（无用户 uniform）
        let mut uniform_bytes = Vec::new();
        for v in [0.0f32, 0.0, 2.0, 1.0] {
            uniform_bytes.extend_from_slice(&v.to_ne_bytes());
        }
        frame.bytes.extend_from_slice(&uniform_bytes);
        frame
            .byte_ranges
            .push(opencat_core::ir::draw_types::TableRange { start: 0, len: uniform_bytes.len() as u32 });
        frame.paints.push(PaintSpec {
            fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
            style: PaintStyle::Fill,
            ..Default::default()
        });
        frame.ops.push(DrawOp::RuntimeEffect {
            effect: EffectId(0),
            uniforms: BytesRangeId(0),
            children: ChildRange { start: 0, len: 1 },
            dst: Rect4 { x: 0.0, y: 0.0, width: 2.0, height: 1.0 },
        });

        let media = EnginePreparedFrameMedia {
            images: vec![child_img],
            image_index: {
                let mut m = std::collections::HashMap::new();
                m.insert(ImageRef::Generated { id: gid }, 0usize);
                m
            },
            runtime_effects: vec![rt],
        };
        let mut exec = EngineDrawExecutor::new();
        exec.begin_frame();
        let mut surface = surfaces::raster_n32_premul((2, 1)).expect("surface");
        replay_op(&mut exec, surface.canvas(), &frame, &media, &frame.ops[0]).expect("replay");
        let image = surface.image_snapshot();
        let image_info = ImageInfo::new((2, 1), ColorType::RGBA8888, AlphaType::Premul, None);
        let mut rgba = vec![0_u8; 2 * 1 * 4];
        assert!(image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            2 * 4,
            (0, 0),
            CachingHint::Allow,
        ));
        assert_eq!(rgba[0..3].to_vec(), vec![255, 0, 0], "left = red");
        assert_eq!(rgba[4..7].to_vec(), vec![0, 255, 0], "right = green");
    }
}
