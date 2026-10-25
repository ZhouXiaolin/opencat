#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::analyze::annotation::{
    AnnotatedDisplayTree, AnnotatedNodeHandle, DrawCompositeSemantics,
};
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::canvas::paint::{BlendMode, FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
use crate::display::list::{DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem};
use crate::geom::Mat4;
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
        DisplayItem::Svg(item) => {
            #[cfg(feature = "profile")]
            let _span = span!(target: "render.backend", Level::TRACE, "draw_item_svg_file").entered();
            super::helpers::render_svg(ctx, item)
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
    has_content_layer: bool,
}

pub(crate) fn save_composite_layer(
    builder: &mut DrawOpBuilder,
    opacity: f32,
    css_filter: &CssFilter,
    backdrop_filter: &CssFilter,
    backdrop_blur: Option<f32>,
    bounds: Option<DisplayRect>,
    backdrop_bounds: DisplayRect,
    backdrop_radius: BorderRadius,
) -> BackdropLayerState {
    // `backdrop-filter` 生效条件：blur sigma 或非 blur 算子任一存在。
    let backdrop_sigma = backdrop_blur.filter(|sigma| *sigma > 0.0);
    let uses_backdrop_filter = !backdrop_filter.is_identity();
    let uses_css_filter = !css_filter.is_identity();
    let uses_layer =
        opacity < 1.0 || backdrop_sigma.is_some() || uses_backdrop_filter || uses_css_filter;
    let mut has_backdrop_clip = false;
    let mut has_content_layer = false;
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
        // `None` means "let the layer cover whatever the subtree paints": a
        // 3-D subtree projects outside the element rect, so clipping to it
        // would cut off the parts nearer the camera.
        let bounds_rect4 = display_rect_to_rect4(bounds.unwrap_or(backdrop_bounds));
        let layer_bounds = bounds.map(display_rect_to_rect4);
        // The stored value carries the CSS `backdrop-filter: blur(R)` radius —
        // same Gaussian sigma = R semantics as `filter: blur()` (see
        // `css_filter_image_filter`); pass through to Skia unchanged.
        if backdrop_sigma.is_some() || uses_backdrop_filter {
            // CSS applies `backdrop-filter` functions in declaration order;
            // Skia's `Compose(outer, inner)` runs `inner` first, so blur is
            // the inner filter and the non-blur ops (folded in declaration
            // order by `css_filter_image_filter`) wrap around it. The
            // element's own `filter` must not touch the backdrop, so only
            // `backdrop_filter` feeds this chain.
            let blur_filter = backdrop_sigma.map(|sigma| ImageFilterSpec::Blur {
                sigma_x: sigma,
                sigma_y: sigma,
                crop_rect: None,
                decal: false,
            });
            let color_filter_chain = super::helpers::css_filter_image_filter(backdrop_filter);
            let image_filter = match (blur_filter, color_filter_chain) {
                (Some(blur), Some(chain)) => {
                    ImageFilterSpec::Compose(Box::new(chain), Box::new(blur))
                }
                (Some(blur), None) => blur,
                (None, Some(chain)) => chain,
                // 入口条件已排除两者皆空。
                (None, None) => unreachable!("backdrop layer without any backdrop filter"),
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
            builder.push(DrawOp::ClipPath { anti_alias: false, fill_type: Default::default() });
            has_backdrop_clip = true;
            // Backdrop semantics: the blur applies to what's beneath this
            // node's bounds; the node's own subtree paints unfiltered inside.
            // `bounds` carries the *element* rect — CSS backdrop-filter is
            // clipped to the border box, not to the shadow-padded layer
            // bounds the box-shadow needs.
            let backdrop_rect4 = display_rect_to_rect4(backdrop_bounds);
            builder.push(DrawOp::SaveBackdropLayer {
                bounds: Some(backdrop_rect4),
                radius: [
                    backdrop_radius.top_left,
                    backdrop_radius.top_right,
                    backdrop_radius.bottom_right,
                    backdrop_radius.bottom_left,
                ],
                paint: Some(paint_id),
            });
            // The node's own `filter` never touches the backdrop, but it still
            // applies to the subtree. Nest a content layer so both run, in the
            // CSS order (backdrop first, then the element's own filter).
            if let Some(image_filter) = super::helpers::css_filter_image_filter(css_filter) {
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
                    bounds: layer_bounds,
                    paint: Some(paint_id),
                    alpha: opacity,
                    backdrop: false,
                });
                has_content_layer = true;
            }
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
                bounds: layer_bounds,
                paint: Some(paint_id),
                alpha: opacity,
                backdrop: false,
            });
        } else {
            builder.push(DrawOp::SaveLayer {
                bounds: layer_bounds,
                paint: None,
                alpha: opacity,
                backdrop: false,
            });
        }
    }
    BackdropLayerState {
        uses_layer,
        has_backdrop_clip,
        has_content_layer,
    }
}

pub(crate) fn restore_backdrop_blur_layer(builder: &mut DrawOpBuilder, state: &BackdropLayerState) {
    if state.uses_layer {
        if state.has_content_layer {
            builder.push(DrawOp::Restore);
        }
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
    backdrop_filter: &'a CssFilter,
    backdrop_blur_sigma: Option<f32>,
    backdrop_border_radius: BorderRadius,
    layer_bounds: DisplayRect,
}

impl<'a> ApplyPlan<'a> {
    fn from_draw_composite(draw: DrawCompositeSemantics<'a>, layer_bounds: DisplayRect) -> Self {
        Self {
            transform: draw.transform,
            opacity: draw.opacity,
            css_filter: draw.css_filter,
            backdrop_filter: draw.backdrop_filter,
            backdrop_blur_sigma: draw.backdrop_blur_sigma,
            backdrop_border_radius: draw.backdrop_border_radius,
            layer_bounds,
        }
    }
}

struct ApplyFrame {
    layer_state: BackdropLayerState,
}

fn backdrop_layer_state_for_plan(plan: &ApplyPlan<'_>) -> BackdropLayerState {
    let uses_backdrop_filter = !plan.backdrop_filter.is_identity();
    // Mirrors `save_composite_layer`: the content layer exists only on the
    // backdrop branch, and only when the node has its own non-identity filter.
    let backdrop_branch = plan.backdrop_blur_sigma.is_some() || uses_backdrop_filter;
    BackdropLayerState {
        uses_layer: plan.opacity < 1.0
            || plan.backdrop_blur_sigma.is_some()
            || uses_backdrop_filter
            || !plan.css_filter.is_identity(),
        has_backdrop_clip: plan.backdrop_blur_sigma.is_some_and(|sigma| sigma > 0.0)
            || uses_backdrop_filter,
        has_content_layer: backdrop_branch && !plan.css_filter.is_identity(),
    }
}

fn emit_apply_prefix(builder: &mut DrawOpBuilder, plan: &ApplyPlan<'_>) -> ApplyFrame {
    builder.push(DrawOp::Save);
    apply_transform(builder, plan.transform);
    // A node inside a 3-D context paints outside its own element rect (a
    // `translateZ` toward the camera magnifies it), so clipping its layer to
    // `layer_bounds` would cut that overflow off. Drop the bounds there.
    let layer_bounds = if is_3d(plan.transform) {
        None
    } else {
        Some(plan.layer_bounds)
    };
    let layer_state = save_composite_layer(
        builder,
        plan.opacity,
        plan.css_filter,
        plan.backdrop_filter,
        plan.backdrop_blur_sigma,
        layer_bounds,
        plan.transform.bounds,
        plan.backdrop_border_radius,
    );
    ApplyFrame { layer_state }
}

/// Whether this node opens a 3-D context of its own, or is transformed into
/// one its parent opened.
fn is_3d(transform: &DisplayTransform) -> bool {
    transform.local_m4.is_some() || transform.perspective.is_some() || transform.preserve_3d
}

/// The 4×4 every node in the current 3-D context inherits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Space3 {
    /// Maps this context's local space into the parent's, `z = 0` being the
    /// plane the enclosing draw-op transform already established.
    abs: Mat4,
    /// Homography `abs` projects to. `None` when it is exactly the identity,
    /// which is the common case and lets us skip the `Concat`.
    abs_h: Option<[f32; 9]>,
}

impl Space3 {
    pub(crate) fn root() -> Self {
        Self {
            abs: Mat4::IDENTITY,
            abs_h: None,
        }
    }

    fn compose(&self, local: Mat4) -> Self {
        let abs = self.abs.mul(&local);
        let h = abs.to_homography3();
        let is_identity = h == Mat4::IDENTITY.to_homography3();
        Self {
            abs,
            abs_h: if is_identity { None } else { Some(h) },
        }
    }
}

/// A node's accumulated 4×4 in the space its parent's children are drawn in.
/// The node's own `Translate` is part of it, because `apply_transform` has
/// already emitted it by the time this runs.
fn node_abs(space: Space3, transform: &DisplayTransform) -> Option<Space3> {
    let local = transform.local_m4?;
    Some(
        space
            .compose(Mat4::translate(
                transform.translation_x,
                transform.translation_y,
                0.0,
            ))
            .compose(local),
    )
}

/// Enter a node's own 3-D space: the `Concat` that takes the enclosing draw-op
/// transform (which already includes this node's `Translate`) to its own.
fn enter_node_space(ctx: &mut RenderCtx, transform: &DisplayTransform) -> Space3 {
    let parent = ctx.space3;
    let Some(node) = node_abs(parent, transform) else {
        return parent;
    };
    let Some(node_h) = node.abs_h else {
        return parent;
    };
    // Incremental, so the enclosing `Save`/`Restore` still unwinds it and the
    // composite-layer bounds keep their meaning.
    let after_translate = parent.compose(Mat4::translate(
        transform.translation_x,
        transform.translation_y,
        0.0,
    ));
    // `Concat` pre-concatenates (CTM' = CTM · M), so M has to be
    // inv(CTM) · target, not target · inv(CTM).
    let matrix = match after_translate.abs_h {
        None => node_h,
        Some(parent_h) => match crate::geom::invert3(parent_h) {
            Some(inv) => crate::geom::mul3(inv, node_h),
            None => return parent,
        },
    };
    ctx.builder.push(DrawOp::Concat { matrix });
    ctx.space3 = node;
    node
}

/// The space a node's children are drawn in: its own, projected through its
/// CSS `perspective`, with the depth dropped when it is `transform-style: flat`.
///
/// The invariant `abs_h` must keep is *what the draw-op CTM currently holds*,
/// so every space change that is not already in the CTM has to be pushed here.
fn child_space(ctx: &mut RenderCtx, node: Space3, transform: &DisplayTransform) -> Space3 {
    let mut space = match transform.perspective {
        Some(distance) if distance > 0.0 => {
            let projected = node.compose(Mat4::perspective(distance));
            if let Some(projected_h) = projected.abs_h {
                let matrix = match node.abs_h {
                    None => projected_h,
                    Some(node_h) => match crate::geom::invert3(node_h) {
                        Some(inv) => crate::geom::mul3(inv, projected_h),
                        None => node_h,
                    },
                };
                ctx.builder.push(DrawOp::Concat { matrix });
            }
            projected
        }
        _ => node,
    };
    if !transform.preserve_3d {
        // Flattening only zeroes the z output row, which `to_homography3` never
        // reads, so the CTM still holds `space.abs_h` unchanged.
        space.abs = space.abs.flatten_depth();
    }
    space
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
    ctx.space3 = Space3::root();
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

/// Render a node's children in painter's order.
///
/// Inside a 3-D context the DOM order is not the visible order: a segment with
/// a larger projected z is nearer the viewer and must be drawn last. The depth
/// changes every frame, so the ordering lives here rather than in the build-time
/// `z_index` sort.
fn render_children(
    ctx: &mut RenderCtx,
    children: &[OrderedSceneOp],
    tree: &AnnotatedDisplayTree,
) -> Result<(), RenderError> {
    if ctx.space3.abs_h.is_none() {
        for child in children {
            render_scene_op(ctx, child, tree)?;
        }
        return Ok(());
    }
    let mut ordered: Vec<(f32, &OrderedSceneOp)> = children
        .iter()
        .map(|child| {
            let transform = &tree.node(child.handle()).transform;
            let depth = match transform.local_m4 {
                Some(_) => {
                    let m = node_abs(ctx.space3, transform)
                        .map(|space| space.abs)
                        .unwrap_or(ctx.space3.abs);
                    m.z_at(
                        transform.bounds.width * transform.origin.x,
                        transform.bounds.height * transform.origin.y,
                    )
                }
                None => ctx
                    .space3
                    .abs
                    .z_at(transform.translation_x, transform.translation_y),
            };
            (depth, child)
        })
        .collect();
    // Nearer last; ties keep DOM order so the common all-coplanar case is
    // untouched.
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, child) in ordered {
        render_scene_op(ctx, child, tree)?;
    }
    Ok(())
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
    let parent_space = ctx.space3;
    let apply_frame = emit_apply_prefix(ctx.builder, &apply_plan);
    let node_space = enter_node_space(ctx, apply_plan.transform);
    ctx.space3 = child_space(ctx, node_space, apply_plan.transform);
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
    render_children(ctx, &subtree.children, tree)?;
    if has_clip {
        ctx.builder.push(DrawOp::Restore);
    }
    if has_paint_clip {
        ctx.builder.push(DrawOp::Restore);
    }

    ctx.space3 = parent_space;
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
    let parent_space = ctx.space3;
    let apply_frame = emit_apply_prefix(ctx.builder, &apply_plan);
    let node_space = enter_node_space(ctx, apply_plan.transform);
    ctx.space3 = child_space(ctx, node_space, apply_plan.transform);

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
        ctx.space3 = parent_space;
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

    render_children(ctx, children, tree)?;

    if node.clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    if node.paint_clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    ctx.space3 = parent_space;
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
            ctx.builder.push(DrawOp::ClipPath { anti_alias: false, fill_type: Default::default() });
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
    builder.push(DrawOp::ClipPath { anti_alias: true, fill_type: Default::default() });
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
    // A chain carrying a 3-D operator is emitted by `enter_node_space` as one
    // 4x4 `Concat`; emitting the 2-D operators here as well would apply the
    // rotation twice.
    if transform.local_m4.is_some() {
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
            Transform::RotateXDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Concat {
                    matrix: crate::geom::Mat4::rotate_x_deg(deg).to_homography3(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            Transform::RotateYDeg { value: deg } => {
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Concat {
                    matrix: crate::geom::Mat4::rotate_y_deg(deg).to_homography3(),
                });
                builder.push(DrawOp::Translate {
                    x: -center_x,
                    y: -center_y,
                });
            }
            // A bare `translateZ` is a no-op on the node's own planar content;
            // it only moves the plane its children are drawn on, which the
            // 4×4 stack in `enter_node_space` already did.
            Transform::TranslateZ { .. } => {}
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
                // the reference raster matches Ry·Rx. `Mat4::rotate3` builds
                // exactly that matrix and folds the perspective row into `w`.
                // Emitted as a single column-major `DrawOp::Concat` about the
                // node's transform-origin (the pivot keeps z=0, so the planar
                // homography stays exact).
                builder.push(DrawOp::Translate {
                    x: center_x,
                    y: center_y,
                });
                builder.push(DrawOp::Concat {
                    matrix: crate::geom::Mat4::rotate3(rotation_x, rotation_y, perspective)
                        .to_homography3(),
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
            backdrop_filter: Default::default(),
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
            local_m4: None,
            perspective: None,
            preserve_3d: false,
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
        let backdrop_filter = CssFilter::default();

        let plan = ApplyPlan {
            transform: &transform,
            opacity: 0.5,
            css_filter: &css_filter,
            backdrop_filter: &backdrop_filter,
            backdrop_blur_sigma: None,
            backdrop_border_radius: Default::default(),
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
        let backdrop_filter = crate::style::CssFilter::default();
        let plan = ApplyPlan {
            transform: &transform,
            opacity: 1.0,
            backdrop_blur_sigma: None,
            css_filter: &css_filter,
            backdrop_filter: &backdrop_filter,
            backdrop_border_radius: Default::default(),
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
        let backdrop_filter = CssFilter::default();
        let plan = ApplyPlan {
            transform: &transform,
            opacity: 0.75,
            css_filter: &css_filter,
            backdrop_filter: &backdrop_filter,
            backdrop_blur_sigma: None,
            backdrop_border_radius: Default::default(),
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

    /// A torus segment: `rotateY(a) translateZ(400) rotateY(90)` pivoting on its
    /// own centre, exactly what `display/build.rs` folds into `local_m4`.
    fn segment(a: f32) -> DisplayTransform {
        let mut t = transform(0.0, 0.0);
        t.transforms = vec![
            Transform::RotateYDeg { value: a },
            Transform::TranslateZ { value: 400.0 },
            Transform::RotateYDeg { value: 90.0 },
        ];
        t.local_m4 = crate::display::build::local_m4(&t.transforms, t.origin, t.bounds);
        t
    }

    #[test]
    fn segment_local_m4_keeps_its_depth() {
        // The box centre is the pivot, so only the `translateZ` may move it.
        // A 3×3 chain would drop that depth and collapse the ring onto one
        // plane; the 4×4 keeps it.
        let t = segment(0.0);
        let m4 = t.local_m4.expect("3-D chain produces a 4×4");
        let (cx, cy) = (50.0, 50.0);
        let x = m4.0[12] + cx * m4.0[0] + cy * m4.0[4];
        let y = m4.0[13] + cx * m4.0[1] + cy * m4.0[5];
        let z = m4.0[14] + cx * m4.0[2] + cy * m4.0[6];
        assert!((x - 50.0).abs() < 1e-2, "x = {x}");
        assert!((y - 50.0).abs() < 1e-2, "y = {y}");
        assert!((z - 400.0).abs() < 1e-2, "z = {z}");

        // Turn the segment 22.5° and the same 400px now points off-axis, as
        // the ring placement requires.
        let t = segment(-22.5);
        let m4 = t.local_m4.unwrap();
        let z = m4.0[14] + cx * m4.0[2] + cy * m4.0[6];
        let expected = 400.0 * (-22.5f32).to_radians().cos();
        assert!((z - expected).abs() < 1e-2, "z = {z}, expected {expected}");
    }


}
