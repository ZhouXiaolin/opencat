use std::hash::{Hash, Hasher};

#[cfg(feature = "profile")]
use tracing::{Level, event, span};

#[cfg(feature = "profile")]
use crate::analyze::annotation::AnalyzeReuseState;
use crate::analyze::annotation::{
    AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, DrawCompositeSemantics,
};
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::analyze::fingerprint::{DisplayRecordedFingerprint, item_paint_fingerprint};
use crate::canvas::paint::{BlendMode, FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
use crate::display::list::{DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem};
use crate::ir::cache::{self as draw_cache, CachedDrawRange, CachedNodeOwnIr, SegmentKey};
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::{DrawOpRange, PathOp};
use crate::layout::tree::LayoutOutputFingerprint;
use crate::parse::transition::{SlideDirection, TransitionKind, WipeDirection};
use crate::render::builder::DrawOpBuilder;
use crate::style::{BorderRadius, Transform};

use super::{RenderCtx, RenderError, record_cache_pressure};

// ── Display Item dispatch ──────────────────────────────────────────────

fn should_cache_item_picture(item: &DisplayItem) -> bool {
    matches!(
        item,
        DisplayItem::Bitmap(_) | DisplayItem::DrawScript(_) | DisplayItem::SvgPath(_)
    )
}

pub fn render_display_item(
    ctx: &mut RenderCtx,
    item: &DisplayItem,
    layout_output_fingerprint: LayoutOutputFingerprint,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    if should_cache_item_picture(item) {
        let cache_key = item_paint_fingerprint(item, layout_output_fingerprint);
        return render_display_item_cached(ctx, item, cache_key, cache);
    }
    render_display_item_direct(ctx, item, cache)
}

fn render_display_item_cached(
    ctx: &mut RenderCtx,
    item: &DisplayItem,
    cache_key: u64,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    #[cfg(feature = "profile")]
    let _cached_span = span!(target: "render.backend", Level::TRACE, "draw_item_cached").entered();

    let semantics = item.picture_semantics();

    // Cache hit: import segment and replay with draw translation
    if let Some(cached_range) = cache.item_ranges.get_cloned(&cache_key)
        && let Some(segment) = cache.segments.get_cloned(&cached_range.segment_key)
    {
        #[cfg(feature = "profile")]
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "item_picture",
            result = "hit",
            amount = 1_u64
        );

        ctx.builder.push(DrawOp::Save);
        ctx.builder.push(DrawOp::Translate {
            x: semantics.draw_translation_x,
            y: semantics.draw_translation_y,
        });
        ctx.builder.import_segment(&segment);
        ctx.builder.push(DrawOp::Restore);
        return Ok(());
    }

    #[cfg(feature = "profile")]
    event!(
        target: "render.cache",
        Level::TRACE,
        kind = "cache",
        name = "item_picture",
        result = "miss",
        amount = 1_u64
    );

    // Cache miss: render directly with draw_translation, snapshot, store
    ctx.builder.push(DrawOp::Save);
    ctx.builder.push(DrawOp::Translate {
        x: semantics.draw_translation_x,
        y: semantics.draw_translation_y,
    });

    let marker = ctx.builder.begin_range();
    render_display_item_direct(ctx, item, cache)?;
    let range = ctx.builder.end_range(marker);

    ctx.builder.push(DrawOp::Restore);

    // Snapshot and store in cache
    let segment = ctx.builder.snapshot_range(range);
    let segment_key = SegmentKey::Item(cache_key);

    cache.segments.insert(segment_key, segment);
    cache.item_ranges.insert(
        cache_key,
        CachedDrawRange {
            segment_range: range,
            fingerprint: cache_key,
            bounds: semantics.record_bounds,
            segment_key,
        },
    );

    Ok(())
}

fn render_display_item_direct(
    ctx: &mut RenderCtx,
    item: &DisplayItem,
    _cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
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

struct BackdropLayerState {
    uses_layer: bool,
    has_backdrop_clip: bool,
}

fn save_backdrop_blur_layer(
    builder: &mut DrawOpBuilder,
    opacity: f32,
    backdrop_blur: Option<f32>,
    bounds: DisplayRect,
) -> BackdropLayerState {
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
        let bounds_rect4 = display_rect_to_rect4(bounds);
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
                builder.push(DrawOp::SaveLayer {
                    bounds: Some(bounds_rect4),
                    paint: Some(paint_id),
                    alpha: 1.0,
                });
            } else {
                builder.push(DrawOp::SaveLayer {
                    bounds: Some(bounds_rect4),
                    paint: None,
                    alpha: opacity,
                });
            }
        } else {
            builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: opacity,
            });
        }
    }
    BackdropLayerState {
        uses_layer,
        has_backdrop_clip,
    }
}

fn restore_backdrop_blur_layer(builder: &mut DrawOpBuilder, state: &BackdropLayerState) {
    if state.uses_layer {
        builder.push(DrawOp::Restore);
        if state.has_backdrop_clip {
            builder.push(DrawOp::Restore);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ApplyPlan<'a> {
    transform: &'a DisplayTransform,
    opacity: f32,
    backdrop_blur_sigma: Option<f32>,
    layer_bounds: DisplayRect,
}

impl<'a> ApplyPlan<'a> {
    fn from_draw_composite(draw: DrawCompositeSemantics<'a>, layer_bounds: DisplayRect) -> Self {
        Self {
            transform: draw.transform,
            opacity: draw.opacity,
            backdrop_blur_sigma: draw.backdrop_blur_sigma,
            layer_bounds,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn apply_segment_key(plan: &ApplyPlan<'_>) -> u64 {
    let mut hasher = ahash::AHasher::default();
    plan.transform.translation_x.to_bits().hash(&mut hasher);
    plan.transform.translation_y.to_bits().hash(&mut hasher);
    plan.transform.bounds.width.to_bits().hash(&mut hasher);
    plan.transform.bounds.height.to_bits().hash(&mut hasher);
    plan.transform.transforms.len().hash(&mut hasher);
    for transform in &plan.transform.transforms {
        hash_transform(transform, &mut hasher);
    }
    plan.opacity.to_bits().hash(&mut hasher);
    plan.backdrop_blur_sigma.map(f32::to_bits).hash(&mut hasher);
    hash_display_rect(plan.layer_bounds, &mut hasher);
    hasher.finish()
}

#[cfg_attr(not(test), allow(dead_code))]
fn hash_display_rect(rect: DisplayRect, hasher: &mut impl Hasher) {
    rect.x.to_bits().hash(hasher);
    rect.y.to_bits().hash(hasher);
    rect.width.to_bits().hash(hasher);
    rect.height.to_bits().hash(hasher);
}

#[cfg_attr(not(test), allow(dead_code))]
fn hash_transform(transform: &Transform, hasher: &mut impl Hasher) {
    match *transform {
        Transform::TranslateX { value } => {
            0_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::TranslateY { value } => {
            1_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::Translate { x, y } => {
            2_u8.hash(hasher);
            x.to_bits().hash(hasher);
            y.to_bits().hash(hasher);
        }
        Transform::Scale { value } => {
            3_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::ScaleX { value } => {
            4_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::ScaleY { value } => {
            5_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::RotateDeg { value } => {
            6_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::SkewXDeg { value } => {
            7_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::SkewYDeg { value } => {
            8_u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        Transform::SkewDeg { x, y } => {
            9_u8.hash(hasher);
            x.to_bits().hash(hasher);
            y.to_bits().hash(hasher);
        }
    }
}

struct ApplyFrame {
    layer_state: BackdropLayerState,
}

fn emit_apply_prefix(builder: &mut DrawOpBuilder, plan: &ApplyPlan<'_>) -> ApplyFrame {
    builder.push(DrawOp::Save);
    apply_transform(builder, plan.transform);
    let layer_state = save_backdrop_blur_layer(
        builder,
        plan.opacity,
        plan.backdrop_blur_sigma,
        plan.layer_bounds,
    );
    ApplyFrame { layer_state }
}

fn emit_apply_suffix(builder: &mut DrawOpBuilder, frame: &ApplyFrame) {
    restore_backdrop_blur_layer(builder, &frame.layer_state);
    builder.push(DrawOp::Restore);
}

fn begin_apply_frame(builder: &mut DrawOpBuilder, plan: &ApplyPlan<'_>) -> ApplyFrame {
    emit_apply_prefix(builder, plan)
}

fn finish_apply_frame(builder: &mut DrawOpBuilder, frame: &ApplyFrame) {
    emit_apply_suffix(builder, frame);
}

pub fn render_display_tree(
    ctx: &mut RenderCtx,
    tree: &AnnotatedDisplayTree,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    render_scene_op(ctx, &ctx.ordered_scene.root, tree, cache)
}

fn render_scene_op(
    ctx: &mut RenderCtx,
    op: &OrderedSceneOp,
    tree: &AnnotatedDisplayTree,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    match op {
        OrderedSceneOp::ReusedSubtree { handle } => {
            render_reused_subtree(ctx, *handle, tree, cache)
        }
        OrderedSceneOp::LiveSubtree { handle, children } => {
            render_live_subtree(ctx, *handle, children, tree, cache)
        }
    }
}

fn render_reused_subtree(
    ctx: &mut RenderCtx,
    handle: AnnotatedNodeHandle,
    tree: &AnnotatedDisplayTree,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let draw = node.draw_composite_semantics();
    if draw.opacity <= 0.0 {
        return Ok(());
    }

    let layer_bounds = tree.layer_bounds(handle);
    let apply_plan = ApplyPlan::from_draw_composite(draw, layer_bounds);
    let apply_frame = begin_apply_frame(ctx.builder, &apply_plan);
    let fingerprint = tree.analysis(handle).snapshot_fingerprint.ok_or_else(|| {
        RenderError::InvalidArgument(
            "ReusedSubtree node must have snapshot_fingerprint".to_string(),
        )
    })?;
    let _snapshot_fingerprint = fingerprint;

    {
        #[cfg(feature = "profile")]
        {
            let result = match tree.analyze_reuse_state(handle) {
                AnalyzeReuseState::Fresh => "fresh",
                AnalyzeReuseState::ReusedFromHistory => "reused",
                AnalyzeReuseState::CompositeBlocked => "composite_blocked",
            };
            event!(
                target: "render.cache",
                Level::TRACE,
                kind = "cache",
                name = "subtree_snapshot_request_after_analyze",
                result = result,
                amount = 1_u64
            );
        }
    }

    let has_clip = node.clip.is_some();
    let subtree = OrderedSceneProgram::build_subtree(tree, handle);
    let own_key = node_own_segment_key(node);

    // First, try the node-own cache (keyed by this node's own content only,
    // independent of child fingerprints). This avoids unnecessary re-recording
    // of the node's item when only descendants have changed.
    {
        let own_hit = cache.node_own_segments.get_cloned(&own_key);
        if let Some(entry) = own_hit {
            if let Some(segment) = cache.segments.get_cloned(&entry.segment_key) {
                #[cfg(feature = "profile")]
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "cache",
                    name = "node_own_segment",
                    result = "hit",
                    amount = 1_u64
                );

                ctx.builder.import_segment(&segment);
                let updated = CachedNodeOwnIr {
                    consecutive_hits: entry.consecutive_hits + 1,
                    ..entry
                };
                let report = cache.node_own_segments.insert(own_key, updated);
                record_cache_pressure("node_own", &report);

                for child in &subtree.children {
                    render_scene_op(ctx, child, tree, cache)?;
                }
                if has_clip {
                    ctx.builder.push(DrawOp::Restore);
                }
                finish_apply_frame(ctx.builder, &apply_frame);
                return Ok(());
            }
        }
    }

    #[cfg(feature = "profile")]
    event!(
        target: "render.cache",
        Level::TRACE,
        kind = "cache",
        name = "node_own_segment",
        result = "record",
        amount = 1_u64
    );

    // Cache miss — record parent's own rendering as a segment
    #[cfg(feature = "profile")]
    let _record_span =
        span!(target: "render.backend", Level::TRACE, "node_own_segment_record").entered();

    let recorded = node.recorded_semantics();

    let parent_marker = ctx.builder.begin_range();
    render_display_item(
        ctx,
        recorded.item,
        recorded.layout_output_fingerprint,
        cache,
    )?;
    if let Some(clip) = &recorded.clip {
        ctx.builder.push(DrawOp::Save);
        let clip_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_rect4, &clip.border_radius);
    }
    let parent_range = ctx.builder.end_range(parent_marker);

    #[cfg(feature = "profile")]
    drop(_record_span);

    let segment = ctx.builder.snapshot_range(parent_range);
    let segment_key = SegmentKey::NodeOwn(own_key);

    cache.segments.insert(segment_key, segment);

    // Also store in node-own cache for reuse when children change but this node doesn't.
    {
        let own_snapshot = CachedNodeOwnIr {
            segment_key,
            consecutive_hits: 0,
            recorded_bounds: layer_bounds,
        };
        #[cfg(feature = "profile")]
        let own_report = cache.node_own_segments.insert(own_key, own_snapshot);
        #[cfg(not(feature = "profile"))]
        let _own_report = cache.node_own_segments.insert(own_key, own_snapshot);
        #[cfg(feature = "profile")]
        {
            if own_report.replaced {
                event!(
                    target: "render.cache",
                    Level::TRACE,
                    kind = "cache",
                    name = "node_own_segment",
                    result = "replaced",
                    amount = 1_u64
                );
            }
            record_cache_pressure("node_own", &own_report);
        }
    }

    // Render children dynamically (not baked into parent segment)
    for child in &subtree.children {
        render_scene_op(ctx, child, tree, cache)?;
    }
    if has_clip {
        ctx.builder.push(DrawOp::Restore);
    }

    finish_apply_frame(ctx.builder, &apply_frame);
    Ok(())
}

fn node_own_segment_key(node: &AnnotatedDisplayNode) -> u64 {
    DisplayRecordedFingerprint::from_recorded(&node.recorded_semantics()).0
}

fn render_live_subtree(
    ctx: &mut RenderCtx,
    handle: AnnotatedNodeHandle,
    children: &[OrderedSceneOp],
    tree: &AnnotatedDisplayTree,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let draw = node.draw_composite_semantics();
    if draw.opacity <= 0.0 {
        return Ok(());
    }

    let apply_plan = ApplyPlan::from_draw_composite(draw, tree.layer_bounds(handle));
    let apply_frame = begin_apply_frame(ctx.builder, &apply_plan);

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
        render_scene_op(ctx, &children[0], tree, cache)?;
        let from_range = ctx.builder.end_range(from_marker);

        let to_marker = ctx.builder.begin_range();
        render_scene_op(ctx, &children[1], tree, cache)?;
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
    render_display_item(ctx, &node.item, node.layout_output_fingerprint, cache)?;

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
        render_scene_op(ctx, child, tree, cache)?;
    }

    if node.clip.is_some() {
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
            });
            ctx.builder.push(DrawOp::ReplayRange { range: to_range });
            ctx.builder.push(DrawOp::Restore);
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
    let center_x = transform.bounds.width / 2.0;
    let center_y = transform.bounds.height / 2.0;

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
        style::{BackgroundFill, ColorToken},
    };

    fn rect_node(
        background: Option<BackgroundFill>,
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
            backdrop_blur_sigma: None,
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
                    blur_sigma: None,
                    box_shadow: None,
                    inset_shadow: None,
                    drop_shadow: None,
                    backdrop_blur_sigma: None,
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
        }
    }

    #[test]
    fn node_own_segment_key_tracks_parent_paint_not_children() {
        let parent_without_child = rect_node(
            Some(BackgroundFill::Solid(ColorToken::Custom(10, 20, 30, 255))),
            transform(0.0, 0.0),
            Vec::new(),
        );
        let parent_with_changed_child = rect_node(
            Some(BackgroundFill::Solid(ColorToken::Custom(10, 20, 30, 255))),
            transform(0.0, 0.0),
            vec![AnnotatedNodeHandle(1)],
        );
        let parent_with_changed_paint = rect_node(
            Some(BackgroundFill::Solid(ColorToken::Custom(30, 20, 10, 255))),
            transform(0.0, 0.0),
            vec![AnnotatedNodeHandle(1)],
        );

        assert_eq!(
            node_own_segment_key(&parent_without_child),
            node_own_segment_key(&parent_with_changed_child),
            "child changes must not invalidate the parent's own segment"
        );
        assert_ne!(
            node_own_segment_key(&parent_without_child),
            node_own_segment_key(&parent_with_changed_paint),
            "parent recorded paint changes must invalidate the parent's own segment"
        );
    }

    #[test]
    fn node_own_segment_key_ignores_apply_transform() {
        let stationary = rect_node(None, transform(0.0, 0.0), vec![AnnotatedNodeHandle(1)]);
        let moved = rect_node(None, transform(24.0, 12.0), vec![AnnotatedNodeHandle(1)]);

        assert_eq!(
            node_own_segment_key(&stationary),
            node_own_segment_key(&moved),
            "apply transform is replayed around the segment and must not invalidate it"
        );
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

        let plan = ApplyPlan {
            transform: &transform,
            opacity: 0.5,
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
            }
        );
        assert_eq!(ops[6], DrawOp::BeginPath);
        assert_eq!(ops[7], DrawOp::Restore);
        assert_eq!(ops[8], DrawOp::Restore);
        assert_eq!(ops.len(), 9);
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
        let plan = ApplyPlan {
            transform: &transform,
            opacity: 0.75,
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
            }
        );
        assert_eq!(ops[3], DrawOp::BeginPath);
        assert_eq!(ops[4], DrawOp::Restore);
        assert_eq!(ops[5], DrawOp::Restore);
        assert_eq!(ops.len(), 6);
    }

    #[test]
    fn apply_plan_is_built_from_draw_composite_semantics() {
        let node = rect_node(None, transform(3.0, 4.0), Vec::new());
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

    #[test]
    fn apply_plan_segment_key_tracks_draw_time_apply_only() {
        let mut transform_a = transform(1.0, 2.0);
        transform_a.transforms = vec![Transform::Scale { value: 1.25 }];
        let mut transform_b = transform(1.0, 2.0);
        transform_b.transforms = vec![Transform::Scale { value: 1.25 }];
        let bounds = DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 80.0,
        };
        transform_a.bounds = bounds;
        transform_b.bounds = bounds;
        let plan_a = ApplyPlan {
            transform: &transform_a,
            opacity: 0.75,
            backdrop_blur_sigma: None,
            layer_bounds: bounds,
        };
        let plan_b = ApplyPlan {
            transform: &transform_b,
            opacity: 0.75,
            backdrop_blur_sigma: None,
            layer_bounds: bounds,
        };

        assert_eq!(
            apply_segment_key(&plan_a),
            apply_segment_key(&plan_b),
            "same draw-time apply instructions should share an apply segment key"
        );

        let mut transform_c = transform_b.clone();
        transform_c.translation_x = 4.0;
        let plan_c = ApplyPlan {
            transform: &transform_c,
            opacity: 0.75,
            backdrop_blur_sigma: None,
            layer_bounds: bounds,
        };
        assert_ne!(
            apply_segment_key(&plan_a),
            apply_segment_key(&plan_c),
            "translation changes should invalidate only the apply segment"
        );
    }
}
