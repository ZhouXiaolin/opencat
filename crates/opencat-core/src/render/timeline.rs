use crate::canvas::paint::{BlendMode, FillSpec, PaintSpec, PaintStyle};
use crate::canvas::{Canvas2D, Rect};
use crate::display::list::{DisplayRect, TimelineDisplayItem, TimelineTransitionDisplay};
use crate::scene::transition::{SlideDirection, TransitionKind, WipeDirection};

use super::rect::kurbo_rect;
use super::{RenderCache, RenderCtx, RenderError};

fn render_transition_overlay<C: Canvas2D>(
    canvas: &mut C,
    bounds: DisplayRect,
    transition: &TimelineTransitionDisplay,
) {
    let rect = kurbo_rect(bounds);
    let p = transition.progress.clamp(0.0, 1.0);

    match &transition.kind {
        TransitionKind::Fade => {
            canvas.save_layer(Some(rect), p);
        }
        TransitionKind::Slide(dir) => {
            let (dx, dy) = slide_offset(dir, bounds, 1.0 - p);
            canvas.translate(dx, dy);
        }
        TransitionKind::Wipe(dir) => {
            let clip_rect = wipe_clip_rect(dir, bounds, p);
            canvas.clip_rect(&clip_rect, crate::canvas::ClipOp::Intersect, false);
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
                canvas.save();
                canvas.clip_rect(&rect, crate::canvas::ClipOp::Intersect, false);
                canvas.draw_arc(&rect, -90.0, sweep, true, &overlay);
                canvas.restore();
            }
        }
        TransitionKind::Iris => {
            let cx = (bounds.x + bounds.width / 2.0) as f32;
            let cy = (bounds.y + bounds.height / 2.0) as f32;
            let scale = p.max(0.001);
            canvas.translate(cx, cy);
            canvas.scale(scale, scale);
            canvas.translate(-cx, -cy);
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
            canvas.draw_rect(&rect, &paint);
        }
        TransitionKind::Gl(_gl) => {}
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

fn wipe_clip_rect(dir: &WipeDirection, bounds: DisplayRect, progress: f32) -> Rect {
    let p = progress;
    match dir {
        WipeDirection::FromLeft => {
            let w = bounds.width * p;
            Rect::new(bounds.x as f64, bounds.y as f64,
                (bounds.x + w) as f64, (bounds.y + bounds.height) as f64)
        }
        WipeDirection::FromRight => {
            let w = bounds.width * p;
            Rect::new((bounds.x + bounds.width - w) as f64, bounds.y as f64,
                (bounds.x + bounds.width) as f64, (bounds.y + bounds.height) as f64)
        }
        WipeDirection::FromTop => {
            let h = bounds.height * p;
            Rect::new(bounds.x as f64, bounds.y as f64,
                (bounds.x + bounds.width) as f64, (bounds.y + h) as f64)
        }
        WipeDirection::FromBottom => {
            let h = bounds.height * p;
            Rect::new(bounds.x as f64, (bounds.y + bounds.height - h) as f64,
                (bounds.x + bounds.width) as f64, (bounds.y + bounds.height) as f64)
        }
        WipeDirection::FromTopLeft => {
            let w = bounds.width * p;
            let h = bounds.height * p;
            Rect::new(bounds.x as f64, bounds.y as f64,
                (bounds.x + w) as f64, (bounds.y + h) as f64)
        }
        WipeDirection::FromTopRight => {
            let w = bounds.width * p;
            let h = bounds.height * p;
            Rect::new((bounds.x + bounds.width - w) as f64, bounds.y as f64,
                (bounds.x + bounds.width) as f64, (bounds.y + h) as f64)
        }
        WipeDirection::FromBottomLeft => {
            let w = bounds.width * p;
            let h = bounds.height * p;
            Rect::new(bounds.x as f64, (bounds.y + bounds.height - h) as f64,
                (bounds.x + w) as f64, (bounds.y + bounds.height) as f64)
        }
        WipeDirection::FromBottomRight => {
            let w = bounds.width * p;
            let h = bounds.height * p;
            Rect::new((bounds.x + bounds.width - w) as f64,
                (bounds.y + bounds.height - h) as f64,
                (bounds.x + bounds.width) as f64, (bounds.y + bounds.height) as f64)
        }
    }
}

fn sinusoid_noise(x: f32, seed: f32) -> f32 {
    ((x * 6.283185 + seed * 12.9898).sin() * 43758.5453).fract()
}

pub fn render_timeline<C: Canvas2D>(
    canvas: &mut C,
    item: &TimelineDisplayItem,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let rect_item = crate::display::list::RectDisplayItem {
        bounds: item.bounds,
        paint: item.paint.clone(),
    };

    if let Some(ref transition) = item.transition {
        let bounds = item.bounds;
        let rect = kurbo_rect(bounds);

        canvas.save();
        canvas.clip_rect(&rect, crate::canvas::ClipOp::Intersect, false);

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
        canvas.save_layer_with(Some(rect), &paint);

        render_transition_overlay(canvas, bounds, transition);

        super::rect::render_rect_with_shadows(canvas, &rect_item, ctx, cache)?;

        canvas.restore();
        canvas.restore();
    } else {
        super::rect::render_rect_with_shadows(canvas, &rect_item, ctx, cache)?;
    }

    Ok(())
}
