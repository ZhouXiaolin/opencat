#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::analyze::annotation::{
    AnnotatedDisplayTree, AnnotatedNodeHandle, DrawCompositeSemantics,
};
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::canvas::paint::{BlendMode, FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
use crate::display::list::{DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem};
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::{DrawOpRange, PathOp};
use crate::media::VideoFrameRequest;
use crate::parse::transition::{SlideDirection, TransitionKind, WipeDirection};
use crate::render::builder::DrawOpBuilder;
use crate::style::{BorderRadius, CssFilter, Transform};

use super::{RenderCtx, RenderError};

// ── Display Item dispatch ──────────────────────────────────────────────

pub fn render_display_item(ctx: &mut RenderCtx, item: &DisplayItem) -> Result<(), RenderError> {
    render_display_item_direct(ctx, item)
}

fn render_display_item_direct(ctx: &mut RenderCtx, item: &DisplayItem) -> Result<(), RenderError> {
    match item {
        DisplayItem::Rect(rect) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_rect").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "rect", result = "count", amount = 1_u64);
            super::helpers::render_rect_with_shadows(ctx, rect)
        }
        DisplayItem::Timeline(timeline) => {
            #[cfg(feature = "profile")]
            let _span =
                span!(target: "render.backend", Level::TRACE, "draw_item_timeline").entered();
            super::helpers::render_timeline(ctx, timeline)
        }
        DisplayItem::Text(text) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_text").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "text", result = "count", amount = 1_u64);
            super::text::render_text_with_shadows(ctx, text)
        }
        DisplayItem::Bitmap(bitmap) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_bitmap").entered();
            super::helpers::render_bitmap_with_shadows(ctx, bitmap)
        }
        DisplayItem::Lottie(lottie) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_lottie").entered();
            super::helpers::render_lottie_with_shadows(ctx, lottie)
        }
        DisplayItem::DrawScript(script) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_script").entered();
            #[cfg(feature = "profile")]
            event!(target: "render.draw", Level::TRACE, kind = "draw", name = "script", result = "count", amount = 1_u64);
            super::helpers::render_draw_script(ctx, script)
        }
        DisplayItem::SvgPath(svg) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_svg").entered();
            super::helpers::render_svg_path(ctx, svg)
        }
    }
}

// ── Display Tree render ───────────────────────────────────────────────

fn display_rect_to_rect4(r: DisplayRect) -> Rect4 {
    Rect4 {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}

pub(crate) struct BackdropLayerState {
    uses_layer: bool,
    has_backdrop_clip: bool,
}

pub(crate) fn save_composite_layer(
    builder: &mut DrawOpBuilder,
    opacity: f32,
    css_filter: &CssFilter,
    backdrop_blur: Option<f32>,
    bounds: DisplayRect,
) -> BackdropLayerState {
    let uses_css_filter = !css_filter.is_identity();
    let uses_layer = opacity < 1.0 || backdrop_blur.is_some() || uses_css_filter;
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
        let bounds_rect4 = display_rect_to_rect4(bounds);
        // The stored value carries the CSS `backdrop-filter: blur(R)` radius —
        // same Gaussian sigma = R semantics as `filter: blur()` (see
        // `css_filter_image_filter`); pass through to Skia unchanged.
        if let Some(sigma) = backdrop_blur.filter(|sigma| *sigma > 0.0) {
            let image_filter = match super::helpers::css_filter_image_filter(css_filter) {
                Some(css_image_filter) => ImageFilterSpec::Compose(
                    Box::new(css_image_filter),
                    Box::new(ImageFilterSpec::Blur {
                        sigma_x: sigma,
                        sigma_y: sigma,
                        crop_rect: None,
                        decal: false,
                    }),
                ),
                None => ImageFilterSpec::Blur {
                    sigma_x: sigma,
                    sigma_y: sigma,
                    crop_rect: None,
                    decal: false,
                },
            };
            let paint = PaintSpec {
                fill: FillSpec::Solid([1.0, 1.0, 1.0, opacity]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: true,
                blend_mode: BlendMode::SrcOver,
                image_filter: Some(image_filter),
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::Save);
            builder.push(DrawOp::BeginPath);
            builder.push(DrawOp::Path(PathOp::AddRect {
                x: bounds_rect4.x,
                y: bounds_rect4.y,
                width: bounds_rect4.width,
                height: bounds_rect4.height,
            }));
            builder.push(DrawOp::ClipPath { anti_alias: false });
            has_backdrop_clip = true;
            // Backdrop semantics: the blur applies to what's beneath this
            // node's bounds; the node's own subtree paints unfiltered inside.
            builder.push(DrawOp::SaveBackdropLayer {
                bounds: Some(bounds_rect4),
                paint: Some(paint_id),
            });
        } else if let Some(image_filter) = super::helpers::css_filter_image_filter(css_filter) {
            let paint = PaintSpec {
                fill: FillSpec::Solid([1.0, 1.0, 1.0, 1.0]),
                style: PaintStyle::Fill,
                stroke: None,
                anti_alias: true,
                blend_mode: BlendMode::SrcOver,
                image_filter: Some(image_filter),
                color_filter: None,
                mask_filter: None,
                path_effect: None,
            };
            let paint_id = builder.intern_paint(paint);
            builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: Some(paint_id),
                alpha: opacity,
                backdrop: false,
            });
        } else {
            builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: opacity,
                backdrop: false,
            });
        }
    }
    BackdropLayerState {
        uses_layer,
        has_backdrop_clip,
    }
}

pub(crate) fn restore_backdrop_blur_layer(builder: &mut DrawOpBuilder, state: &BackdropLayerState) {
    if state.uses_layer {
        builder.push(DrawOp::Restore);
        if state.has_backdrop_clip {
            builder.push(DrawOp::Restore);
        }
    }
}

#[derive(Clone, Debug)]
struct ApplyPlan<'a> {
    transform: &'a DisplayTransform,
    opacity: f32,
    css_filter: &'a CssFilter,
    backdrop_blur_sigma: Option<f32>,
    layer_bounds: DisplayRect,
}

impl<'a> ApplyPlan<'a> {
    fn from_draw_composite(draw: DrawCompositeSemantics<'a>, layer_bounds: DisplayRect) -> Self {
        Self {
            transform: draw.transform,
            opacity: draw.opacity,
            css_filter: draw.css_filter,
            backdrop_blur_sigma: draw.backdrop_blur_sigma,
            layer_bounds,
        }
    }
}

struct ApplyFrame {
    layer_state: BackdropLayerState,
}

fn backdrop_layer_state_for_plan(plan: &ApplyPlan<'_>) -> BackdropLayerState {
    BackdropLayerState {
        uses_layer: plan.opacity < 1.0
            || plan.backdrop_blur_sigma.is_some()
            || !plan.css_filter.is_identity(),
        has_backdrop_clip: plan.backdrop_blur_sigma.is_some_and(|sigma| sigma > 0.0),
    }
}

fn emit_apply_prefix(builder: &mut DrawOpBuilder, plan: &ApplyPlan<'_>) -> ApplyFrame {
    builder.push(DrawOp::Save);
    apply_transform(builder, plan.transform);
    let layer_state = save_composite_layer(
        builder,
        plan.opacity,
        plan.css_filter,
        plan.backdrop_blur_sigma,
        plan.layer_bounds,
    );
    ApplyFrame { layer_state }
}

fn emit_apply_suffix(builder: &mut DrawOpBuilder, frame: &ApplyFrame) {
    restore_backdrop_blur_layer(builder, &frame.layer_state);
    builder.push(DrawOp::Restore);
}

#[cfg_attr(not(test), allow(dead_code))]
fn begin_apply_frame(builder: &mut DrawOpBuilder, plan: &ApplyPlan<'_>) -> ApplyFrame {
    emit_apply_prefix(builder, plan)
}

fn finish_apply_frame(builder: &mut DrawOpBuilder, frame: &ApplyFrame) {
    emit_apply_suffix(builder, frame);
}

pub fn render_display_tree(
    ctx: &mut RenderCtx,
    tree: &AnnotatedDisplayTree,
) -> Result<(), RenderError> {
    render_scene_op(ctx, &ctx.ordered_scene.root, tree)
}

fn item_visible_at_frame(ctx: &RenderCtx, item: &DisplayItem) -> bool {
    let DisplayItem::Bitmap(bitmap) = item else {
        return true;
    };
    let Some(timing) = bitmap.video_timing else {
        return true;
    };

    VideoFrameRequest {
        composition_time_secs: ctx.frame_ctx.frame as f64 / ctx.frame_ctx.fps.max(1) as f64,
        timing,
    }
    .is_visible()
}

fn render_scene_op(
    ctx: &mut RenderCtx,
    op: &OrderedSceneOp,
    tree: &AnnotatedDisplayTree,
) -> Result<(), RenderError> {
    match op {
        OrderedSceneOp::ReusedSubtree { handle } => render_reused_subtree(ctx, *handle, tree),
        OrderedSceneOp::LiveSubtree { handle, children } => {
            render_live_subtree(ctx, *handle, children, tree)
        }
    }
}

fn render_reused_subtree(
    ctx: &mut RenderCtx,
    handle: AnnotatedNodeHandle,
    tree: &AnnotatedDisplayTree,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let draw = node.draw_composite_semantics();
    if draw.opacity <= 0.0 {
        return Ok(());
    }
    if !item_visible_at_frame(ctx, &node.item) {
        return Ok(());
    }

    let layer_bounds = tree.layer_bounds(handle);
    let apply_plan = ApplyPlan::from_draw_composite(draw, layer_bounds);
    let apply_frame = emit_apply_prefix(ctx.builder, &apply_plan);
    let fingerprint = tree.analysis(handle).snapshot_fingerprint.ok_or_else(|| {
        RenderError::InvalidArgument(
            "ReusedSubtree node must have snapshot_fingerprint".to_string(),
        )
    })?;
    let _snapshot_fingerprint = fingerprint;

    let has_paint_clip = node.paint_clip.is_some();
    let has_clip = node.clip.is_some();
    let subtree = OrderedSceneProgram::build_subtree(tree, handle);

    let recorded = node.recorded_semantics();

    if let Some(clip) = &recorded.paint_clip {
        ctx.builder.push(DrawOp::Save);
        let clip_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_rect4, &clip.border_radius);
    }
    render_display_item(ctx, recorded.item)?;
    if let Some(clip) = &recorded.clip {
        ctx.builder.push(DrawOp::Save);
        let clip_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_rect4, &clip.border_radius);
    }

    // Render children dynamically.
    for child in &subtree.children {
        render_scene_op(ctx, child, tree)?;
    }
    if has_clip {
        ctx.builder.push(DrawOp::Restore);
    }
    if has_paint_clip {
        ctx.builder.push(DrawOp::Restore);
    }

    finish_apply_frame(ctx.builder, &apply_frame);
    Ok(())
}

fn render_live_subtree(
    ctx: &mut RenderCtx,
    handle: AnnotatedNodeHandle,
    children: &[OrderedSceneOp],
    tree: &AnnotatedDisplayTree,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let draw = node.draw_composite_semantics();
    if draw.opacity <= 0.0 {
        return Ok(());
    }
    if !item_visible_at_frame(ctx, &node.item) {
        return Ok(());
    }

    let apply_plan = ApplyPlan::from_draw_composite(draw, tree.layer_bounds(handle));
    let apply_frame = emit_apply_prefix(ctx.builder, &apply_plan);

    // Transition compositing: render from/to subtrees and blend them
    if let DisplayItem::Timeline(timeline) = &node.item
        && let Some(ref transition) = timeline.transition
        && children.len() == 2
    {
        let rect_item = RectDisplayItem {
            bounds: timeline.bounds,
            paint: timeline.paint.clone(),
        };
        super::helpers::render_rect_with_shadows(ctx, &rect_item)?;

        if let Some(clip) = &node.clip {
            ctx.builder.push(DrawOp::Save);
            let clip_bounds_rect4 = display_rect_to_rect4(clip.bounds);
            clip_bounds_with_radius(ctx.builder, clip_bounds_rect4, &clip.border_radius);
        }

        let from_marker = ctx.builder.begin_range();
        render_scene_op(ctx, &children[0], tree)?;
        let from_range = ctx.builder.end_range(from_marker);

        let to_marker = ctx.builder.begin_range();
        render_scene_op(ctx, &children[1], tree)?;
        let to_range = ctx.builder.end_range(to_marker);

        #[cfg(feature = "profile")]
        let _trans_span = span!(
            target: "render.transition",
            Level::TRACE,
            "draw_transition",
            transition_kind = transition_kind_str(&transition.kind),
        )
        .entered();

        let p = transition.progress.clamp(0.0, 1.0);
        render_transition_composite(
            ctx,
            from_range,
            to_range,
            p,
            &transition.kind,
            timeline.bounds,
        );

        if node.clip.is_some() {
            ctx.builder.push(DrawOp::Restore);
        }
        finish_apply_frame(ctx.builder, &apply_frame);
        return Ok(());
    }

    #[cfg(feature = "profile")]
    let _item_span = span!(target: "render.backend", Level::TRACE, "draw_item").entered();
    if let Some(clip) = &node.paint_clip {
        ctx.builder.push(DrawOp::Save);
        let clip_bounds_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_bounds_rect4, &clip.border_radius);
    }
    render_display_item(ctx, &node.item)?;

    if let Some(clip) = &node.clip {
        ctx.builder.push(DrawOp::Save);
        let clip_bounds_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_bounds_rect4, &clip.border_radius);
    }

    if let Some(ref slot) = node.draw_slot {
        if !slot.commands.is_empty() {
            super::helpers::render_draw_script(ctx, slot)?;
        }
    }

    for child in children {
        render_scene_op(ctx, child, tree)?;
    }

    if node.clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    if node.paint_clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    finish_apply_frame(ctx.builder, &apply_frame);
    Ok(())
}

fn render_transition_composite(
    ctx: &mut RenderCtx,
    from_range: DrawOpRange,
    to_range: DrawOpRange,
    progress: f32,
    kind: &TransitionKind,
    bounds: DisplayRect,
) {
    ctx.builder.push(DrawOp::ReplayRange { range: from_range });

    match kind {
        TransitionKind::Fade => {
            let bounds_rect4 = display_rect_to_rect4(bounds);
            ctx.builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: progress,
                backdrop: false,
            });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::Slide(dir) => {
            let (dx, dy) = match dir {
                SlideDirection::FromLeft => (-(1.0 - progress) * bounds.width, 0.0),
                SlideDirection::FromRight => ((1.0 - progress) * bounds.width, 0.0),
                SlideDirection::FromTop => (0.0, -(1.0 - progress) * bounds.height),
                SlideDirection::FromBottom => (0.0, (1.0 - progress) * bounds.height),
            };
            ctx.builder.push(DrawOp::Save);
            ctx.builder.push(DrawOp::Translate { x: dx, y: dy });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::Wipe(dir) => {
            let clip = match dir {
                WipeDirection::FromLeft => Rect4 {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height,
                },
                WipeDirection::FromRight => Rect4 {
                    x: bounds.x + bounds.width * (1.0 - progress),
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height,
                },
                WipeDirection::FromTop => Rect4 {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height * progress,
                },
                WipeDirection::FromBottom => Rect4 {
                    x: bounds.x,
                    y: bounds.y + bounds.height * (1.0 - progress),
                    width: bounds.width,
                    height: bounds.height * progress,
                },
                WipeDirection::FromTopLeft => Rect4 {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
                WipeDirection::FromTopRight => Rect4 {
                    x: bounds.x + bounds.width * (1.0 - progress),
                    y: bounds.y,
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
                WipeDirection::FromBottomLeft => Rect4 {
                    x: bounds.x,
                    y: bounds.y + bounds.height * (1.0 - progress),
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
                WipeDirection::FromBottomRight => Rect4 {
                    x: bounds.x + bounds.width * (1.0 - progress),
                    y: bounds.y + bounds.height * (1.0 - progress),
                    width: bounds.width * progress,
                    height: bounds.height * progress,
                },
            };
            ctx.builder.push(DrawOp::Save);
            ctx.builder.push(DrawOp::BeginPath);
            ctx.builder.push(DrawOp::Path(PathOp::AddRect {
                x: clip.x,
                y: clip.y,
                width: clip.width,
                height: clip.height,
            }));
            ctx.builder.push(DrawOp::ClipPath { anti_alias: false });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::Iris => {
            let cx = bounds.x + bounds.width / 2.0;
            let cy = bounds.y + bounds.height / 2.0;
            let scale = progress.max(0.001);
            ctx.builder.push(DrawOp::Save);
            ctx.builder.push(DrawOp::Translate { x: cx, y: cy });
            ctx.builder.push(DrawOp::Scale { x: scale, y: scale });
            ctx.builder.push(DrawOp::Translate { x: -cx, y: -cy });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        TransitionKind::LightLeak(params) => {
            super::helpers::render_light_leak_transition(
                ctx, from_range, to_range, progress, params, bounds,
            );
        }
        TransitionKind::Gl(effect) => {
            super::helpers::render_gl_transition(
                ctx, from_range, to_range, progress, effect, bounds,
            );
        }
        TransitionKind::ClockWipe => {
            let bounds_rect4 = display_rect_to_rect4(bounds);
            ctx.builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: progress,
                backdrop: false,
            });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
        }
        // No effect: the incoming scene covers the outgoing one outright.
        TransitionKind::Cut => {
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
        }
    }
}

pub(crate) fn clip_bounds_with_radius(
    builder: &mut DrawOpBuilder,
    rect: Rect4,
    radius: &BorderRadius,
) {
    let w = rect.width;
    let h = rect.height;
    let clamp = |r: f32| {
        if r <= 0.0 {
            0.0
        } else {
            r.min(w / 2.0).min(h / 2.0)
        }
    };
    let tl = clamp(radius.top_left);
    let tr = clamp(radius.top_right);
    let br = clamp(radius.bottom_right);
    let bl = clamp(radius.bottom_left);

    let has_radius = tl > 0.0 || tr > 0.0 || br > 0.0 || bl > 0.0;

    builder.push(DrawOp::BeginPath);
    if has_radius {
        let x = rect.x;
        let y = rect.y;
        let x1 = x + rect.width;
        let y1 = y + rect.height;
        builder.push(DrawOp::Path(PathOp::MoveTo { x: x + tl, y }));
        builder.push(DrawOp::Path(PathOp::LineTo { x: x1 - tr, y }));
        if tr > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x1,
                cy: y,
                x: x1,
                y: y + tr,
            }));
        }
        builder.push(DrawOp::Path(PathOp::LineTo { x: x1, y: y1 - br }));
        if br > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x1,
                cy: y1,
                x: x1 - br,
                y: y1,
            }));
        }
        builder.push(DrawOp::Path(PathOp::LineTo { x: x + bl, y: y1 }));
        if bl > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x,
                cy: y1,
                x,
                y: y1 - bl,
            }));
        }
        builder.push(DrawOp::Path(PathOp::LineTo { x, y: y + tl }));
        if tl > 0.0 {
            builder.push(DrawOp::Path(PathOp::QuadTo {
                cx: x,
                cy: y,
                x: x + tl,
                y,
            }));
        }
        builder.push(DrawOp::Path(PathOp::Close));
    } else {
        builder.push(DrawOp::Path(PathOp::AddRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }));
    }
    builder.push(DrawOp::ClipPath { anti_alias: true });
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

pub(crate) fn apply_transform(builder: &mut DrawOpBuilder, transform: &DisplayTransform) {
    builder.push(DrawOp::Translate {
        x: transform.translation_x,
        y: transform.translation_y,
    });
    if transform.transforms.is_empty() {
        return;
    }
    // The pivot is the node's static `transform-origin` (default centre). Scale/
    // rotate/skew anchor here; translations are unaffected.
    let center_x = transform.bounds.width * transform.origin.x;
    let center_y = transform.bounds.height * transform.origin.y;

    for t in transform.transforms.iter() {
        match *t {
            Transform::TranslateX { value } => builder.push(DrawOp::Translate { x: value, y: 0.0 }),
            Transform::TranslateY { value } => builder.push(DrawOp::Translate { x: 0.0, y: value }),
            Transform::Translate { x, y } => builder.push(DrawOp::Translate { x, y }),
            Transform::Scale { value } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Scale { x: value, y: value });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::ScaleX { value } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Scale { x: value, y: 1.0 });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::ScaleY { value } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Scale { x: 1.0, y: value });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::RotateDeg { value: deg } => builder.push(DrawOp::Rotate {
                degrees: deg,
                cx: center_x,
                cy: center_y,
            }),
            Transform::SkewXDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Skew {
                    sx: deg.to_radians().tan(),
                    sy: 0.0,
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::SkewYDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Skew {
                    sx: 0.0,
                    sy: deg.to_radians().tan(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::SkewDeg { x: x_deg, y: y_deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Skew {
                    sx: x_deg.to_radians().tan(),
                    sy: y_deg.to_radians().tan(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::Rotate3 {
                rotation_x,
                rotation_y,
                perspective,
            } => {
                // GSAP `rotationX`/`rotationY` + `transformPerspective`: GSAP does
                // NOT emit the CSS chain `perspective(P) rotateX(rx) rotateY(ry)`
                // — it composes its own matrix3d in which the point hits
                // rotateX FIRST (M = P·Ry·Rx, the transpose-order of the CSS
                // list). Verified against the reference render: with rx=6/ry=-7
                // the two orders diverge by ~5px at ±400px from the pivot, and
                // the reference raster matches Ry·Rx. On the z=0 plane that is
                // ONE projective homography:
                //   X' = ( cos_ry·x + sin_rx·sin_ry·y )  / w
                //   Y' = ( cos_rx·y )                    / w
                //   w  = 1 + (sin_ry·x − cos_ry·sin_rx·y) / P
                // Emitted as a single column-major `DrawOp::Concat` about the
                // node's transform-origin (the pivot keeps z=0, so the planar
                // homography stays exact).
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                let sin_rx = rotation_x.to_radians().sin();
                let cos_rx = rotation_x.to_radians().cos();
                let sin_ry = rotation_y.to_radians().sin();
                let cos_ry = rotation_y.to_radians().cos();
                let inv_p = if perspective > 0.0 { 1.0 / perspective } else { 0.0 };
                builder.push(DrawOp::Concat {
                    matrix: [
                        cos_ry, sin_rx * sin_ry, sin_ry * inv_p, // column 0 (a, d, g)
                        0.0, cos_rx, -cos_ry * sin_rx * inv_p, // column 1 (b, e, h)
                        0.0, 0.0, 1.0, // column 2 (c, f, i)
                    ],
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyze::annotation::AnnotatedDisplayNode,
        display::{
            list::{DisplayItem, RectPaintStyle},
            tree::DisplayRecordedSubtreeFingerprint,
        },
        layout::tree::LayoutOutputFingerprint,
        semantic::fingerprint::ElementInputFingerprints,
        style::BackgroundFill,
    };

    fn rect_node(
        background: Vec<BackgroundFill>,
        transform: DisplayTransform,
        children: Vec<AnnotatedNodeHandle>,
    ) -> AnnotatedDisplayNode {
        let bounds = DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        AnnotatedDisplayNode {
            input_fingerprints: ElementInputFingerprints::default(),
            layout_output_fingerprint: LayoutOutputFingerprint::default(),
            recorded_subtree_fingerprint: DisplayRecordedSubtreeFingerprint::default(),
            transform,
            opacity: 1.0,
            css_filter: Default::default(),
            backdrop_blur_sigma: None,
            paint_clip: None,
            clip: None,
            item: DisplayItem::Rect(RectDisplayItem {
                bounds,
                paint: RectPaintStyle {
                    background,
                    border_radius: BorderRadius::default(),
                    border_width: None,
                    border_top_width: None,
                    border_right_width: None,
                    border_bottom_width: None,
                    border_left_width: None,
                    border_color: None,
                    border_style: None,
                    box_shadow: Vec::new(),
                    inset_shadow: Vec::new(),
                    drop_shadow: Vec::new(),
                    backdrop_blur_sigma: None,
                    blend_mode: Default::default(),
                },
            }),
            children,
            draw_slot: None,
            hidden_subtree: Vec::new(),
        }
    }

    fn transform(translation_x: f32, translation_y: f32) -> DisplayTransform {
        DisplayTransform {
            translation_x,
            translation_y,
            bounds: DisplayRect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            transforms: Vec::new(),
            origin: Default::default(),
        }
    }

    #[test]
    fn apply_frame_wraps_body_with_transform_and_layer() {
        let mut builder = DrawOpBuilder::default();
        let mut transform = transform(10.0, 20.0);
        transform.transforms = vec![Transform::Scale { value: 2.0 }];
        let bounds = DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 80.0,
        };
        transform.bounds = bounds;
        let css_filter = CssFilter::default();

        let plan = ApplyPlan {
            transform: &transform,
            opacity: 0.5,
            css_filter: &css_filter,
            backdrop_blur_sigma: None,
            layer_bounds: bounds,
        };
        let frame = begin_apply_frame(&mut builder, &plan);
        builder.push(DrawOp::BeginPath);
        finish_apply_frame(&mut builder, &frame);
        let ops = builder.finish().ops;

        assert_eq!(ops[0], DrawOp::Save);
        assert_eq!(ops[1], DrawOp::Translate { x: 10.0, y: 20.0 });
        assert_eq!(ops[2], DrawOp::Translate { x: 50.0, y: 40.0 });
        assert_eq!(ops[3], DrawOp::Scale { x: 2.0, y: 2.0 });
        assert_eq!(ops[4], DrawOp::Translate { x: -50.0, y: -40.0 });
        assert_eq!(
            ops[5],
            DrawOp::SaveLayer {
                bounds: Some(display_rect_to_rect4(bounds)),
                paint: None,
                alpha: 0.5,
                backdrop: false,
            }
        );
        assert_eq!(ops[6], DrawOp::BeginPath);
        assert_eq!(ops[7], DrawOp::Restore);
        assert_eq!(ops[8], DrawOp::Restore);
        assert_eq!(ops.len(), 9);
    }

    #[test]
    fn apply_frame_wraps_body_with_css_filter_layer() {
        let mut builder = DrawOpBuilder::default();
        let transform = transform(0.0, 0.0);
        let bounds = DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 80.0,
        };
        let mut css_filter = crate::style::CssFilter::default();
        css_filter.set_property("brightness", 0.5);
        let plan = ApplyPlan {
            transform: &transform,
            opacity: 1.0,
            backdrop_blur_sigma: None,
            css_filter: &css_filter,
            layer_bounds: bounds,
        };

        let frame = begin_apply_frame(&mut builder, &plan);
        builder.push(DrawOp::BeginPath);
        finish_apply_frame(&mut builder, &frame);
        let program = builder.finish();
        let ops = program.ops;

        let DrawOp::SaveLayer {
            bounds: Some(layer_bounds),
            paint: Some(paint_id),
            alpha,
            ..
        } = ops[2]
        else {
            panic!("expected CSS filter SaveLayer, got {:?}", ops[2]);
        };
        assert_eq!(layer_bounds, display_rect_to_rect4(bounds));
        assert_eq!(alpha, 1.0);
        let paint = &program.paints[paint_id.0 as usize];
        assert!(
            matches!(paint.image_filter, Some(ImageFilterSpec::ColorFilter(_))),
            "CSS color filters must be applied to a whole-node layer"
        );
        assert_eq!(ops[3], DrawOp::BeginPath);
        assert_eq!(ops[4], DrawOp::Restore);
        assert_eq!(ops[5], DrawOp::Restore);
        assert_eq!(ops.len(), 6);
    }

    #[test]
    fn apply_plan_emits_prefix_and_suffix_around_dynamic_body() {
        let mut builder = DrawOpBuilder::default();
        let transform = transform(7.0, 9.0);
        let bounds = DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 10.0,
        };
        let css_filter = CssFilter::default();
        let plan = ApplyPlan {
            transform: &transform,
            opacity: 0.75,
            css_filter: &css_filter,
            backdrop_blur_sigma: None,
            layer_bounds: bounds,
        };

        let frame = emit_apply_prefix(&mut builder, &plan);
        builder.push(DrawOp::BeginPath);
        emit_apply_suffix(&mut builder, &frame);
        let ops = builder.finish().ops;

        assert_eq!(ops[0], DrawOp::Save);
        assert_eq!(ops[1], DrawOp::Translate { x: 7.0, y: 9.0 });
        assert_eq!(
            ops[2],
            DrawOp::SaveLayer {
                bounds: Some(display_rect_to_rect4(bounds)),
                paint: None,
                alpha: 0.75,
                backdrop: false,
            }
        );
        assert_eq!(ops[3], DrawOp::BeginPath);
        assert_eq!(ops[4], DrawOp::Restore);
        assert_eq!(ops[5], DrawOp::Restore);
        assert_eq!(ops.len(), 6);
    }

    #[test]
    fn apply_plan_is_built_from_draw_composite_semantics() {
        let node = rect_node(Vec::new(), transform(3.0, 4.0), Vec::new());
        let layer_bounds = DisplayRect {
            x: 1.0,
            y: 2.0,
            width: 30.0,
            height: 40.0,
        };

        let plan = ApplyPlan::from_draw_composite(node.draw_composite_semantics(), layer_bounds);

        assert_eq!(plan.opacity, 1.0);
        assert_eq!(plan.backdrop_blur_sigma, None);
        assert_eq!(plan.layer_bounds.x, layer_bounds.x);
        assert_eq!(plan.layer_bounds.y, layer_bounds.y);
        assert_eq!(plan.layer_bounds.width, layer_bounds.width);
        assert_eq!(plan.layer_bounds.height, layer_bounds.height);
        assert_eq!(plan.transform.translation_x, 3.0);
        assert_eq!(plan.transform.translation_y, 4.0);
    }

}
