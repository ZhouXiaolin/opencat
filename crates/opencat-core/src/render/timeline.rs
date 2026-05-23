use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};
use crate::display::list::{DisplayRect, TimelineDisplayItem, TimelineTransitionDisplay};
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::PathOp;
use crate::parse::transition::{SlideDirection, TransitionKind, WipeDirection};
use crate::render::builder::DrawOpBuilder;

use super::rect::{kurbo_rect, rect_to_rect4};
use super::{RenderCtx, RenderError};

fn render_transition_overlay(
    builder: &mut DrawOpBuilder,
    bounds: DisplayRect,
    transition: &TimelineTransitionDisplay,
) {
    let bounded_rect4 = rect_to_rect4(kurbo_rect(bounds));
    let p = transition.progress.clamp(0.0, 1.0);

    match &transition.kind {
        TransitionKind::Fade => {
            // Alpha blends into the SaveLayer created in render_timeline
        }
        TransitionKind::Slide(dir) => {
            let (dx, dy) = slide_offset(dir, bounds, 1.0 - p);
            builder.push(DrawOp::Translate { x: dx, y: dy });
        }
        TransitionKind::Wipe(dir) => {
            let clip_rect4 = wipe_clip_rect(dir, bounds, p);
            builder.push(DrawOp::BeginPath);
            builder.push(DrawOp::Path(PathOp::AddRect {
                x: clip_rect4.x,
                y: clip_rect4.y,
                width: clip_rect4.width,
                height: clip_rect4.height,
            }));
            builder.push(DrawOp::ClipPath { anti_alias: false });
        }
        TransitionKind::ClockWipe => {
            let sweep = (1.0 - p) * 360.0;
            if sweep > 0.0 {
                let overlay = PaintSpec {
                    fill: FillSpec::Solid([0.0, 0.0, 0.0, 1.0]),
                    style: PaintStyle::Fill,
                    stroke: None,
                    anti_alias: true,
                    blend_mode: BlendMode::SrcOver,
                    image_filter: None,
                    color_filter: None,
                    mask_filter: None,
                    path_effect: None,
                };
                let paint_id = builder.intern_paint(overlay);
                builder.push(DrawOp::Save);
                builder.push(DrawOp::BeginPath);
                builder.push(DrawOp::Path(PathOp::AddRect {
                    x: bounded_rect4.x,
                    y: bounded_rect4.y,
                    width: bounded_rect4.width,
                    height: bounded_rect4.height,
                }));
                builder.push(DrawOp::ClipPath { anti_alias: false });
                builder.push(DrawOp::Arc {
                    rect: bounded_rect4,
                    start: -90.0,
                    sweep,
                    use_center: true,
                    paint: paint_id,
                });
                builder.push(DrawOp::Restore);
            }
        }
        TransitionKind::Iris => {
            let cx = bounded_rect4.x + bounded_rect4.width / 2.0;
            let cy = bounded_rect4.y + bounded_rect4.height / 2.0;
            let scale = p.max(0.001);
            builder.push(DrawOp::Translate { x: cx, y: cy });
            builder.push(DrawOp::Scale { x: scale, y: scale });
            builder.push(DrawOp::Translate { x: -cx, y: -cy });
        }
        TransitionKind::LightLeak(leak) => {
            let r = sinusoid_noise(p, leak.seed);
            let g = sinusoid_noise(p, leak.seed + 1.0);
            let b = sinusoid_noise(p, leak.seed + 2.0);
            let alpha = (1.0 - p) * 0.3 * leak.mask_scale;
            let paint = PaintSpec {
                fill: FillSpec::Solid([r, g, b, alpha]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: false,
                blend_mode: BlendMode::SrcOver,
                image_filter: None,
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Rect {
                rect: bounded_rect4,
                paint: paint_id,
            });
        }
        TransitionKind::Gl(gl) => {
            log::warn!("GL transition '{}' not supported in render layer", gl.name);
        }
    }
}

fn slide_offset(dir: &SlideDirection, bounds: DisplayRect, amount: f32) -> (f32, f32) {
    match dir {
        SlideDirection::FromLeft => (bounds.width * -amount, 0.0),
        SlideDirection::FromRight => (bounds.width * amount, 0.0),
        SlideDirection::FromTop => (0.0, bounds.height * -amount),
        SlideDirection::FromBottom => (0.0, bounds.height * amount),
    }
}

fn wipe_clip_rect(dir: &WipeDirection, bounds: DisplayRect, progress: f32) -> Rect4 {
    let p = progress;
    let x = bounds.x;
    let y = bounds.y;
    let w = bounds.width;
    let h = bounds.height;
    match dir {
        WipeDirection::FromLeft => Rect4 {
            x,
            y,
            width: w * p,
            height: h,
        },
        WipeDirection::FromRight => Rect4 {
            x: x + w * (1.0 - p),
            y,
            width: w * p,
            height: h,
        },
        WipeDirection::FromTop => Rect4 {
            x,
            y,
            width: w,
            height: h * p,
        },
        WipeDirection::FromBottom => Rect4 {
            x,
            y: y + h * (1.0 - p),
            width: w,
            height: h * p,
        },
        WipeDirection::FromTopLeft => Rect4 {
            x,
            y,
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromTopRight => Rect4 {
            x: x + w * (1.0 - p),
            y,
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromBottomLeft => Rect4 {
            x,
            y: y + h * (1.0 - p),
            width: w * p,
            height: h * p,
        },
        WipeDirection::FromBottomRight => Rect4 {
            x: x + w * (1.0 - p),
            y: y + h * (1.0 - p),
            width: w * p,
            height: h * p,
        },
    }
}

fn sinusoid_noise(x: f32, seed: f32) -> f32 {
    ((x * std::f32::consts::TAU + seed * 12.9898).sin() * 43_758.547).fract()
}

pub fn render_timeline(ctx: &mut RenderCtx, item: &TimelineDisplayItem) -> Result<(), RenderError> {
    let rect_item = crate::display::list::RectDisplayItem {
        bounds: item.bounds,
        paint: item.paint.clone(),
    };

    if let Some(ref transition) = item.transition {
        let bounds = item.bounds;
        let rect4 = rect_to_rect4(kurbo_rect(bounds));

        // Solid white backdrop for transition compositing
        let paint = PaintSpec {
            fill: FillSpec::Solid([1.0; 4]),
            style: PaintStyle::Fill,
            stroke: None,
            anti_alias: true,
            blend_mode: BlendMode::SrcOver,
            image_filter: None,
            color_filter: None,
            mask_filter: None,
            path_effect: None,
        };
        let paint_id = ctx.builder.intern_paint(paint);

        let layer_alpha = match &transition.kind {
            TransitionKind::Fade => transition.progress.clamp(0.0, 1.0),
            _ => 1.0,
        };

        ctx.builder.push(DrawOp::Save);
        ctx.builder.push(DrawOp::BeginPath);
        ctx.builder.push(DrawOp::Path(PathOp::AddRect {
            x: rect4.x,
            y: rect4.y,
            width: rect4.width,
            height: rect4.height,
        }));
        ctx.builder.push(DrawOp::ClipPath { anti_alias: false });

        ctx.builder.push(DrawOp::SaveLayer {
            bounds: Some(rect4),
            paint: Some(paint_id),
            alpha: layer_alpha,
        });

        render_transition_overlay(ctx.builder, bounds, transition);
        super::rect::render_rect_with_shadows(ctx, &rect_item)?;

        ctx.builder.push(DrawOp::Restore);
        ctx.builder.push(DrawOp::Restore);
    } else {
        super::rect::render_rect_with_shadows(ctx, &rect_item)?;
    }

    Ok(())
}
