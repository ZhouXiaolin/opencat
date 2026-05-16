#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::canvas::paint::{BlendMode, FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
use crate::canvas::{Canvas2D, ClipOp, Rect};
use crate::display::list::{DisplayItem, DisplayRect};
use crate::runtime::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle};
use crate::runtime::compositor::ordered_scene::{OrderedSceneOp, OrderedSceneProgram};
use crate::runtime::compositor::reuse::LiveNodeItemExecution;
use crate::scene::transition::{SlideDirection, TransitionKind, WipeDirection};
use crate::style::{BorderRadius, Transform};

use super::cache::CachedSubtreeSnapshot;
use super::{record_cache_pressure, RenderCache, RenderCtx, RenderError};

fn kurbo_rect(r: DisplayRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, (r.x + r.width) as f64, (r.y + r.height) as f64)
}

fn kurbo_rect_xywh(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64)
}

fn kurbo_rrect(rect: &Rect, radius: &BorderRadius) -> crate::canvas::RRect {
    let radii = effective_corner_radius(rect, radius);
    let r: (f64, f64, f64, f64) = (radii[0] as f64, radii[1] as f64, radii[2] as f64, radii[3] as f64);
    crate::canvas::RRect::new(rect.x0, rect.y0, rect.x1, rect.y1, r)
}

fn effective_corner_radius(rect: &Rect, radius: &BorderRadius) -> [f32; 4] {
    let w = rect.width() as f32;
    let h = rect.height() as f32;
    let clamp = |r: f32| {
        if r <= 0.0 { 0.0 } else { r.min(w / 2.0).min(h / 2.0) }
    };
    [clamp(radius.top_left), clamp(radius.top_right), clamp(radius.bottom_right), clamp(radius.bottom_left)]
}

pub fn render_display_tree<C: Canvas2D>(
    canvas: &mut C,
    tree: &AnnotatedDisplayTree,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    render_scene_op(canvas, &ctx.ordered_scene.root, tree, ctx, cache)
}

fn render_scene_op<C: Canvas2D>(
    canvas: &mut C,
    op: &OrderedSceneOp,
    tree: &AnnotatedDisplayTree,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    match op {
        OrderedSceneOp::CachedSubtree { handle } => {
            render_cached_subtree(canvas, *handle, tree, ctx, cache)
        }
        OrderedSceneOp::LiveSubtree { handle, item_execution, children } => {
            render_live_subtree(canvas, *handle, *item_execution, children, tree, ctx, cache)
        }
    }
}

fn render_cached_subtree<C: Canvas2D>(
    canvas: &mut C,
    handle: AnnotatedNodeHandle,
    tree: &AnnotatedDisplayTree,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let draw = node.draw_composite_semantics();
    if draw.opacity <= 0.0 {
        return Ok(());
    }

    canvas.save();
    apply_transform(canvas, draw.transform);

    let opacity = draw.opacity;
    let backdrop_blur = draw.backdrop_blur_sigma;
    let layer_bounds = tree.layer_bounds(handle);
    let fingerprint = tree.analysis(handle).snapshot_fingerprint
        .expect("CachedSubtree node must have snapshot_fingerprint");
    let key = fingerprint.primary;

    let uses_layer = opacity < 1.0 || backdrop_blur.is_some();
    let mut has_backdrop_clip = false;
    if uses_layer {
        #[cfg(feature = "profile")]
        event!(
            target: "render.layer",
            Level::TRACE,
            kind = "layer",
            name = "save_layer",
            result = "count",
            amount = 1_u64
        );
        let bounds_rect = kurbo_rect(layer_bounds);
        if let Some(sigma) = backdrop_blur {
            if sigma > 0.0 {
                let paint = PaintSpec {
                    fill: FillSpec::Solid([1.0, 1.0, 1.0, opacity]),
                    style: PaintStyle::Fill,
                    stroke: None,
                    anti_alias: true,
                    blend_mode: BlendMode::SrcOver,
                    image_filter: Some(ImageFilterSpec::Blur {
                        sigma_x: sigma,
                        sigma_y: sigma,
                        crop_rect: None,
                    }),
                    color_filter: None,
                    mask_filter: None,
                    path_effect: None,
                };
                canvas.save();
                canvas.clip_rect(&bounds_rect, ClipOp::Intersect, false);
                has_backdrop_clip = true;
                canvas.save_layer_with(Some(bounds_rect), &paint);
            } else {
                canvas.save_layer(Some(bounds_rect), opacity);
            }
        } else {
            canvas.save_layer(Some(bounds_rect), opacity);
        }
    }

    {
        let mut lru = cache.subtree_snapshots.borrow_mut();
        if let Some(snapshot) = lru.get_cloned(&key) {
            if snapshot.secondary_fingerprint == fingerprint.secondary {
                #[cfg(feature = "profile")]
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "cache",
                    name = "subtree_snapshot",
                    result = "hit",
                    amount = 1_u64
                );
                #[cfg(feature = "profile")]
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "consecutive",
                    name = "subtree_snapshot",
                    result = "count",
                    amount = snapshot.consecutive_hits as u64
                );
                canvas.draw_picture(&snapshot.picture, None, None);
                let updated = CachedSubtreeSnapshot {
                    consecutive_hits: snapshot.consecutive_hits + 1,
                    ..snapshot
                };
                let report = lru.insert(key, updated);
                drop(lru);
                record_cache_pressure("subtree_snapshot", &report);
                if uses_layer {
                    canvas.restore();
                    if has_backdrop_clip {
                        canvas.restore();
                    }
                }
                canvas.restore();
                return Ok(());
            }
        }
    }

    #[cfg(feature = "profile")]
    let _record_span = span!(target: "render.backend", Level::TRACE, "subtree_snapshot_record").entered();
    let bounds = kurbo_rect(layer_bounds);
    let recorded = canvas.make_picture(&bounds, |rec_canvas| {
        let _ = render_live_cached_node(rec_canvas, handle, tree, ctx, cache);
    });
    #[cfg(feature = "profile")]
    drop(_record_span);

    let snapshot = CachedSubtreeSnapshot {
        picture: recorded.clone(),
        secondary_fingerprint: fingerprint.secondary,
        consecutive_hits: 0,
        recorded_bounds: layer_bounds,
    };
    {
        let report = cache.subtree_snapshots.borrow_mut().insert(key, snapshot);
        record_cache_pressure("subtree_snapshot", &report);
    }
    #[cfg(feature = "profile")]
    event!(
        target: "render.cache",
        Level::TRACE,
        kind = "cache",
        name = "subtree_snapshot",
        result = "miss",
        amount = 1_u64
    );

    #[cfg(feature = "profile")]
    let _draw_span = span!(target: "render.backend", Level::TRACE, "subtree_snapshot_draw").entered();
    canvas.draw_picture(&recorded, None, None);
    #[cfg(feature = "profile")]
    drop(_draw_span);

    if uses_layer {
        canvas.restore();
        if has_backdrop_clip {
            canvas.restore();
        }
    }
    canvas.restore();
    Ok(())
}

fn render_live_cached_node<C: Canvas2D>(
    canvas: &mut C,
    handle: AnnotatedNodeHandle,
    tree: &AnnotatedDisplayTree,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let subtree = OrderedSceneProgram::build_subtree(tree, handle);

    super::display_item::render_display_item(canvas, node.recorded_semantics().item, ctx, cache)?;

    if let Some(clip) = node.recorded_semantics().clip {
        canvas.save();
        let bounds = kurbo_rect(clip.bounds);
        clip_bounds_with_radius(canvas, &bounds, &clip.border_radius);
    }
    for child in &subtree.children {
        render_scene_op(canvas, child, tree, ctx, cache)?;
    }
    if node.recorded_semantics().clip.is_some() {
        canvas.restore();
    }
    Ok(())
}

fn render_live_subtree<C: Canvas2D>(
    canvas: &mut C,
    handle: AnnotatedNodeHandle,
    item_execution: LiveNodeItemExecution,
    children: &[OrderedSceneOp],
    tree: &AnnotatedDisplayTree,
    ctx: &RenderCtx<C>,
    cache: &mut RenderCache<C>,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let draw = node.draw_composite_semantics();
    if draw.opacity <= 0.0 {
        return Ok(());
    }

    canvas.save();
    apply_transform(canvas, draw.transform);

    let opacity = draw.opacity;
    let backdrop_blur = draw.backdrop_blur_sigma;
    let bounds = tree.layer_bounds(handle);

    let uses_layer = opacity < 1.0 || backdrop_blur.is_some();
    let mut has_backdrop_clip = false;
    if uses_layer {
        #[cfg(feature = "profile")]
        event!(
            target: "render.layer",
            Level::TRACE,
            kind = "layer",
            name = "save_layer",
            result = "count",
            amount = 1_u64
        );
        let bounds_rect = kurbo_rect(bounds);
        if let Some(sigma) = backdrop_blur {
            if sigma > 0.0 {
                let paint = PaintSpec {
                    fill: FillSpec::Solid([1.0, 1.0, 1.0, opacity]),
                    style: PaintStyle::Fill,
                    stroke: None,
                    anti_alias: true,
                    blend_mode: BlendMode::SrcOver,
                    image_filter: Some(ImageFilterSpec::Blur {
                        sigma_x: sigma,
                        sigma_y: sigma,
                        crop_rect: None,
                    }),
                    color_filter: None,
                    mask_filter: None,
                    path_effect: None,
                };
                canvas.save();
                canvas.clip_rect(&bounds_rect, ClipOp::Intersect, false);
                has_backdrop_clip = true;
                canvas.save_layer_with(Some(bounds_rect), &paint);
            } else {
                canvas.save_layer(Some(bounds_rect), opacity);
            }
        } else {
            canvas.save_layer(Some(bounds_rect), opacity);
        }
    }

    if let DisplayItem::Timeline(timeline) = &node.item {
        if let Some(ref transition) = timeline.transition {
            if children.len() == 2 {
                let tl_rect = super::rect::kurbo_rect(timeline.bounds);
                let rect_item = crate::display::list::RectDisplayItem {
                    bounds: timeline.bounds,
                    paint: timeline.paint.clone(),
                };
                super::rect::render_rect_with_shadows(canvas, &rect_item, ctx, cache)?;

                if let Some(clip) = &node.clip {
                    canvas.save();
                    let clip_bounds_rect = kurbo_rect(clip.bounds);
                    clip_bounds_with_radius(canvas, &clip_bounds_rect, &clip.border_radius);
                }

                let from_pic = canvas.make_picture(&tl_rect, |rec| {
                    let _ = render_scene_op(rec, &children[0], tree, ctx, cache);
                });
                let to_pic = canvas.make_picture(&tl_rect, |rec| {
                    let _ = render_scene_op(rec, &children[1], tree, ctx, cache);
                });

                #[cfg(feature = "profile")]
                let _trans_span = span!(
                    target: "render.transition",
                    Level::TRACE,
                    "draw_transition",
                    transition_kind = transition_kind_str(&transition.kind),
                )
                .entered();

                let p = transition.progress.clamp(0.0, 1.0);
                render_transition_composite(canvas, &from_pic, &to_pic, p, &transition.kind, timeline.bounds, cache);

                if node.clip.is_some() {
                    canvas.restore();
                }
                if uses_layer {
                    canvas.restore();
                    if has_backdrop_clip {
                        canvas.restore();
                    }
                }
                canvas.restore();
                return Ok(());
            }
        }
    }

    match item_execution {
        LiveNodeItemExecution::Direct => {
            #[cfg(feature = "profile")]
            let _item_span = span!(target: "render.backend", Level::TRACE, "draw_item").entered();
            super::display_item::render_display_item(canvas, &node.item, ctx, cache)?;
        }
        LiveNodeItemExecution::FrameLocalPicture => {
            #[cfg(feature = "profile")]
            let _item_span = span!(target: "render.backend", Level::TRACE, "draw_item_frame_local_picture").entered();
            let semantics = node.item.picture_semantics();
            let pic_bounds = kurbo_rect_xywh(
                0.0, 0.0,
                semantics.record_bounds.width, semantics.record_bounds.height,
            );
            let picture = canvas.make_picture(&pic_bounds, |rec_canvas| {
                rec_canvas.translate(
                    semantics.record_translation_x,
                    semantics.record_translation_y,
                );
                let _ = super::display_item::render_display_item(rec_canvas, &node.item, ctx, cache);
            });
            canvas.save();
            canvas.translate(semantics.draw_translation_x, semantics.draw_translation_y);
            canvas.draw_picture(&picture, None, None);
            canvas.restore();
        }
    }

    if let Some(clip) = &node.clip {
        canvas.save();
        let clip_bounds_rect = kurbo_rect(clip.bounds);
        clip_bounds_with_radius(canvas, &clip_bounds_rect, &clip.border_radius);
    }

    for child in children {
        render_scene_op(canvas, child, tree, ctx, cache)?;
    }

    if node.clip.is_some() {
        canvas.restore();
    }
    if uses_layer {
        canvas.restore();
        if has_backdrop_clip {
            canvas.restore();
        }
    }
    canvas.restore();
    Ok(())
}

fn render_transition_composite<C: Canvas2D>(
    canvas: &mut C,
    from_pic: &C::Picture,
    to_pic: &C::Picture,
    progress: f32,
    kind: &TransitionKind,
    bounds: DisplayRect,
    cache: &mut RenderCache<C>,
) {
    let rect = kurbo_rect(bounds);
    canvas.draw_picture(from_pic, None, None);

    match kind {
        TransitionKind::Fade => {
            canvas.save_layer(Some(rect), progress);
            canvas.draw_picture(to_pic, None, None);
            canvas.restore();
        }
        TransitionKind::Slide(dir) => {
            let (dx, dy) = match dir {
                SlideDirection::FromLeft => (-(1.0 - progress) * bounds.width, 0.0),
                SlideDirection::FromRight => ((1.0 - progress) * bounds.width, 0.0),
                SlideDirection::FromTop => (0.0, -(1.0 - progress) * bounds.height),
                SlideDirection::FromBottom => (0.0, (1.0 - progress) * bounds.height),
            };
            canvas.save();
            canvas.translate(dx, dy);
            canvas.draw_picture(to_pic, None, None);
            canvas.restore();
        }
        TransitionKind::Wipe(dir) => {
            let clip = match dir {
                WipeDirection::FromLeft => DisplayRect {
                    x: bounds.x + bounds.width * (1.0 - progress),
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height,
                },
                WipeDirection::FromRight => DisplayRect {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height,
                },
                WipeDirection::FromTop => {
                    let start = bounds.y + bounds.height * (1.0 - progress);
                    DisplayRect {
                        x: bounds.x,
                        y: start,
                        width: bounds.width,
                        height: bounds.height * progress,
                    }
                }
                WipeDirection::FromBottom => DisplayRect {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height * progress,
                },
                _ => DisplayRect {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height,
                },
            };
            canvas.save();
            canvas.clip_rect(&kurbo_rect(clip), ClipOp::Intersect, false);
            canvas.draw_picture(to_pic, None, None);
            canvas.restore();
        }
        TransitionKind::Iris => {
            let cx = bounds.x + bounds.width / 2.0;
            let cy = bounds.y + bounds.height / 2.0;
            let scale = progress.max(0.001);
            canvas.save();
            canvas.translate(cx, cy);
            canvas.scale(scale, scale);
            canvas.translate(-cx, -cy);
            canvas.draw_picture(to_pic, None, None);
            canvas.restore();
        }
        TransitionKind::LightLeak(params) => {
            super::transition::render_light_leak_transition(
                canvas, from_pic, to_pic, progress, params, bounds, cache,
            );
        }
        TransitionKind::Gl(effect) => {
            super::transition::render_gl_transition(
                canvas, from_pic, to_pic, progress, effect, bounds, cache,
            );
        }
        TransitionKind::ClockWipe => {
            canvas.save_layer(Some(rect), progress);
            canvas.draw_picture(to_pic, None, None);
            canvas.restore();
        }
    }
}

fn clip_bounds_with_radius<C: Canvas2D>(canvas: &mut C, rect: &Rect, radius: &BorderRadius) {
    let radii = effective_corner_radius(rect, radius);
    if radii.iter().any(|&r| r > 0.0) {
        let rrect = kurbo_rrect(rect, radius);
        canvas.clip_rrect(&rrect, ClipOp::Intersect, true);
    } else {
        canvas.clip_rect(rect, ClipOp::Intersect, true);
    }
}

#[cfg(feature = "profile")]
fn transition_kind_str(kind: &TransitionKind) -> &'static str {
    match kind {
        TransitionKind::Slide(_) => "slide",
        TransitionKind::LightLeak(_) => "light_leak",
        TransitionKind::Gl(_) => "gltransition",
        _ => "other",
    }
}

fn apply_transform<C: Canvas2D>(canvas: &mut C, transform: &crate::display::list::DisplayTransform) {
    canvas.translate(transform.translation_x, transform.translation_y);
    if transform.transforms.is_empty() {
        return;
    }
    let rect = kurbo_rect(transform.bounds);
    let center_x = rect.width() as f32 / 2.0;
    let center_y = rect.height() as f32 / 2.0;

    for t in transform.transforms.iter() {
        match *t {
            Transform::TranslateX { value } => canvas.translate(value, 0.0),
            Transform::TranslateY { value } => canvas.translate(0.0, value),
            Transform::Translate { x, y } => canvas.translate(x, y),
            Transform::Scale { value } => {
                canvas.translate(center_x, center_y);
                canvas.scale(value, value);
                canvas.translate(-center_x, -center_y);
            }
            Transform::ScaleX { value } => {
                canvas.translate(center_x, center_y);
                canvas.scale(value, 1.0);
                canvas.translate(-center_x, -center_y);
            }
            Transform::ScaleY { value } => {
                canvas.translate(center_x, center_y);
                canvas.scale(1.0, value);
                canvas.translate(-center_x, -center_y);
            }
            Transform::RotateDeg { value: deg } => canvas.rotate(deg, center_x, center_y),
            Transform::SkewXDeg { value: deg } => {
                canvas.translate(center_x, center_y);
                canvas.skew(deg.to_radians().tan(), 0.0);
                canvas.translate(-center_x, -center_y);
            }
            Transform::SkewYDeg { value: deg } => {
                canvas.translate(center_x, center_y);
                canvas.skew(0.0, deg.to_radians().tan());
                canvas.translate(-center_x, -center_y);
            }
            Transform::SkewDeg { x: x_deg, y: y_deg } => {
                canvas.translate(center_x, center_y);
                canvas.skew(x_deg.to_radians().tan(), y_deg.to_radians().tan());
                canvas.translate(-center_x, -center_y);
            }
        }
    }
}


