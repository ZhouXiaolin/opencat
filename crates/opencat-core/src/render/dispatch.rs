#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::analyze::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle};
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::analyze::fingerprint::item_paint_fingerprint;
use crate::canvas::paint::{BlendMode, FillSpec, ImageFilterSpec, PaintSpec, PaintStyle};
use crate::display::list::{DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem};
use crate::ir::cache::{self as draw_cache, CachedDrawRange, CachedSubtreeIr};
use crate::ir::draw_op::{DrawOp, Rect4};
use crate::ir::draw_types::{DrawOpRange, PathOp};
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
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    if should_cache_item_picture(item)
        && let Some(cache_key) = item_paint_fingerprint(item)
    {
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
    let segment_key = cache_key;

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
        OrderedSceneOp::CachedSubtree { handle } => {
            render_cached_subtree(ctx, *handle, tree, cache)
        }
        OrderedSceneOp::LiveSubtree { handle, children } => {
            render_live_subtree(ctx, *handle, children, tree, cache)
        }
    }
}

fn render_cached_subtree(
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

    ctx.builder.push(DrawOp::Save);
    apply_transform(ctx.builder, draw.transform);

    let opacity = draw.opacity;
    let backdrop_blur = draw.backdrop_blur_sigma;
    let layer_bounds = tree.layer_bounds(handle);
    let fingerprint = tree
        .analysis(handle)
        .snapshot_fingerprint
        .expect("CachedSubtree node must have snapshot_fingerprint");
    let key = fingerprint.0;

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
        let bounds_rect4 = display_rect_to_rect4(layer_bounds);
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
                let paint_id = ctx.builder.intern_paint(paint);
                ctx.builder.push(DrawOp::Save);
                ctx.builder.push(DrawOp::BeginPath);
                ctx.builder.push(DrawOp::Path(PathOp::AddRect {
                    x: bounds_rect4.x,
                    y: bounds_rect4.y,
                    width: bounds_rect4.width,
                    height: bounds_rect4.height,
                }));
                ctx.builder.push(DrawOp::ClipPath { anti_alias: false });
                has_backdrop_clip = true;
                ctx.builder.push(DrawOp::SaveLayer {
                    bounds: Some(bounds_rect4),
                    paint: Some(paint_id),
                    alpha: 1.0,
                });
            } else {
                ctx.builder.push(DrawOp::SaveLayer {
                    bounds: Some(bounds_rect4),
                    paint: None,
                    alpha: opacity,
                });
            }
        } else {
            ctx.builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: opacity,
            });
        }
    }

    // Check cache
    {
        let hit_entry = cache.subtree_snapshots.get_cloned(&key);
        if let Some(entry) = hit_entry {
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
                amount = entry.consecutive_hits as u64
            );
            if let Some(segment) = cache.segments.get_cloned(&entry.segment_key) {
                ctx.builder.import_segment(&segment);
                let updated = CachedSubtreeIr {
                    consecutive_hits: entry.consecutive_hits + 1,
                    ..entry
                };
                let report = cache.subtree_snapshots.insert(key, updated);
                record_cache_pressure("subtree_snapshot", &report);
                if uses_layer {
                    ctx.builder.push(DrawOp::Restore);
                    if has_backdrop_clip {
                        ctx.builder.push(DrawOp::Restore);
                    }
                }
                ctx.builder.push(DrawOp::Restore);
                return Ok(());
            }
        }
    }

    // Cache miss — record subtree into IR range
    #[cfg(feature = "profile")]
    let _record_span =
        span!(target: "render.backend", Level::TRACE, "subtree_snapshot_record").entered();

    let range_marker = ctx.builder.begin_range();
    render_live_cached_node(ctx, handle, tree, cache)?;
    let range = ctx.builder.end_range(range_marker);

    #[cfg(feature = "profile")]
    drop(_record_span);

    let segment = ctx.builder.snapshot_range(range);
    let segment_key = key;

    cache.segments.insert(segment_key, segment);

    let snapshot = CachedSubtreeIr {
        segment_key,
        consecutive_hits: 0,
        recorded_bounds: layer_bounds,
    };
    {
        let report = cache.subtree_snapshots.insert(key, snapshot);
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

    if uses_layer {
        ctx.builder.push(DrawOp::Restore);
        if has_backdrop_clip {
            ctx.builder.push(DrawOp::Restore);
        }
    }
    ctx.builder.push(DrawOp::Restore);
    Ok(())
}

fn render_live_cached_node(
    ctx: &mut RenderCtx,
    handle: AnnotatedNodeHandle,
    tree: &AnnotatedDisplayTree,
    cache: &mut draw_cache::RenderCache,
) -> Result<(), RenderError> {
    let node = tree.node(handle);
    let subtree = OrderedSceneProgram::build_subtree(tree, handle);

    render_display_item(ctx, node.recorded_semantics().item, cache)?;

    if let Some(clip) = node.recorded_semantics().clip {
        ctx.builder.push(DrawOp::Save);
        let clip_rect4 = display_rect_to_rect4(clip.bounds);
        clip_bounds_with_radius(ctx.builder, clip_rect4, &clip.border_radius);
    }
    for child in &subtree.children {
        render_scene_op(ctx, child, tree, cache)?;
    }
    if node.recorded_semantics().clip.is_some() {
        ctx.builder.push(DrawOp::Restore);
    }
    Ok(())
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

    ctx.builder.push(DrawOp::Save);
    apply_transform(ctx.builder, draw.transform);

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
                let paint_id = ctx.builder.intern_paint(paint);
                ctx.builder.push(DrawOp::Save);
                ctx.builder.push(DrawOp::BeginPath);
                ctx.builder.push(DrawOp::Path(PathOp::AddRect {
                    x: bounds_rect4.x,
                    y: bounds_rect4.y,
                    width: bounds_rect4.width,
                    height: bounds_rect4.height,
                }));
                ctx.builder.push(DrawOp::ClipPath { anti_alias: false });
                has_backdrop_clip = true;
                ctx.builder.push(DrawOp::SaveLayer {
                    bounds: Some(bounds_rect4),
                    paint: Some(paint_id),
                    alpha: 1.0,
                });
            } else {
                ctx.builder.push(DrawOp::SaveLayer {
                    bounds: Some(bounds_rect4),
                    paint: None,
                    alpha: opacity,
                });
            }
        } else {
            ctx.builder.push(DrawOp::SaveLayer {
                bounds: Some(bounds_rect4),
                paint: None,
                alpha: opacity,
            });
        }
    }

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
        if uses_layer {
            ctx.builder.push(DrawOp::Restore);
            if has_backdrop_clip {
                ctx.builder.push(DrawOp::Restore);
            }
        }
        ctx.builder.push(DrawOp::Restore);
        return Ok(());
    }

    #[cfg(feature = "profile")]
    let _item_span = span!(target: "render.backend", Level::TRACE, "draw_item").entered();
    render_display_item(ctx, &node.item, cache)?;

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
    if uses_layer {
        ctx.builder.push(DrawOp::Restore);
        if has_backdrop_clip {
            ctx.builder.push(DrawOp::Restore);
        }
    }
    ctx.builder.push(DrawOp::Restore);
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
