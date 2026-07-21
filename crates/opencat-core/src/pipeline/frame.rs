//! Shared per-frame executor for pipeline implementations.

use std::sync::Arc;

use anyhow::Result;

#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::analyze::annotation::{
    AnalyzeFingerprintHistory, annotate_display_tree,
    compute_display_tree_fingerprints_with_history,
};
use crate::analyze::compositor::{OrderedSceneProgram, plan_for_scene};
use crate::analyze::invalidation::{CompositeHistory, mark_display_tree_apply_changed};
use crate::display::build::DisplayBuildSession;
use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::ir::cache::{RenderCache, SceneSnapshotEntry};
use crate::ir::{DrawOpFrame, FrameMediaPlan};
use crate::layout::LayoutSession;
use crate::parse::composition::Composition;
use crate::render::RenderCtx;
use crate::render::builder::DrawOpBuilder;
use crate::render::media_plan::build_media_plan;
use crate::resolve::resolve::resolve_ui_tree_with_script_cache;
use crate::resource::catalog::ResourceResolver;
use crate::script::ScriptHost;
use crate::text::DefaultFontProvider;

#[allow(clippy::too_many_arguments)]
pub fn render_frame_with_state(
    composition: &Composition,
    frame_index: u32,
    layout_session: &mut LayoutSession,
    display_build_session: &mut DisplayBuildSession,
    composite_history: &mut CompositeHistory,
    analyze_fingerprint_history: &mut AnalyzeFingerprintHistory,
    font_db: &Arc<fontdb::Database>,
    catalog: &mut dyn ResourceResolver,
    cache: &mut RenderCache,
    last_ordered_scene: &mut OrderedSceneProgram,
    script: &mut dyn ScriptHost,
    generated_images: &mut crate::ir::GeneratedImageTable,
) -> Result<(DrawOpFrame, FrameMediaPlan)> {
    #[cfg(feature = "profile")]
    let _frame_span = span!(
        target: "render.pipeline",
        Level::TRACE,
        "frame",
        frame = frame_index,
        width = composition.width as i64,
        height = composition.height as i64,
        fps = composition.fps as i64,
        mode = "scene"
    )
    .entered();

    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);

    #[cfg(feature = "profile")]
    let _resolve_span = span!(target: "render.scene", Level::TRACE, "resolve_ui_tree").entered();
    let root = composition.root_node(&frame_ctx);
    #[cfg(feature = "profile")]
    let _script_span = span!(target: "render.pipeline", Level::TRACE, "script").entered();
    let element_root = crate::text::scope_font_db(font_db, || {
        resolve_ui_tree_with_script_cache(
            &root,
            &frame_ctx,
            &script_frame_ctx,
            catalog,
            None,
            script,
        )
    })?;
    #[cfg(feature = "profile")]
    drop(_script_span);
    #[cfg(feature = "profile")]
    drop(_resolve_span);

    let provider = DefaultFontProvider::from_arc(font_db.clone());
    let (layout_tree, layout_pass) =
        layout_session.compute_layout_with_provider(&element_root, &frame_ctx, &provider)?;

    #[cfg(feature = "profile")]
    {
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "reused_nodes", result = "count", amount = layout_pass.reused_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "input_merkle_full_hit_subtrees", result = "count", amount = layout_pass.input_merkle_full_hit_subtrees as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "input_merkle_full_hit_nodes", result = "count", amount = layout_pass.input_merkle_full_hit_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "layout_merkle_skipped_subtrees", result = "count", amount = layout_pass.layout_merkle_skipped_subtrees as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "layout_merkle_skipped_nodes", result = "count", amount = layout_pass.layout_merkle_skipped_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "layout_dirty", result = "count", amount = layout_pass.layout_dirty_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "raster_dirty", result = "count", amount = layout_pass.raster_dirty_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "structure_rebuild", result = "count", amount = layout_pass.structure_rebuild as u64);
    }

    #[cfg(feature = "profile")]
    let _display_span = span!(target: "render.scene", Level::TRACE, "build_display_tree").entered();
    let (display_tree, display_stats) =
        display_build_session.build_with_cache(&element_root, &layout_tree, &frame_ctx)?;
    #[cfg(feature = "profile")]
    {
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_merkle_skipped_subtrees", result = "count", amount = display_stats.subtree_full_hit_subtrees as u64);
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_merkle_skipped_nodes", result = "count", amount = display_stats.subtree_full_hit_nodes as u64);
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_rebuilt_nodes", result = "count", amount = display_stats.rebuilt_nodes as u64);
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_apply_only_nodes", result = "count", amount = display_stats.apply_only_nodes as u64);
    }
    #[cfg(not(feature = "profile"))]
    let _ = display_stats;
    let mut annotated = annotate_display_tree(&display_tree);
    let apply_changed_stats = mark_display_tree_apply_changed(
        composite_history,
        &mut annotated,
        layout_pass.structure_rebuild,
    );
    #[cfg(not(feature = "profile"))]
    let _ = apply_changed_stats;
    #[cfg(feature = "profile")]
    let analyze_stats = compute_display_tree_fingerprints_with_history(
        &mut annotated,
        analyze_fingerprint_history,
        layout_pass.structure_rebuild,
    );
    #[cfg(not(feature = "profile"))]
    compute_display_tree_fingerprints_with_history(
        &mut annotated,
        analyze_fingerprint_history,
        layout_pass.structure_rebuild,
    );
    #[cfg(feature = "profile")]
    {
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_recorded_subtree_identical_subtrees", result = "count", amount = analyze_stats.recorded_hit_subtrees as u64);
        event!(target: "render.display", Level::TRACE, kind = "display", name = "display_recorded_subtree_identical_nodes", result = "count", amount = analyze_stats.recorded_hit_nodes as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_merkle_skipped_subtrees", result = "count", amount = analyze_stats.merkle_skipped_subtrees as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_merkle_skipped_nodes", result = "count", amount = analyze_stats.merkle_skipped_nodes as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_recorded_hit_subtrees", result = "count", amount = analyze_stats.recorded_hit_subtrees as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_recorded_hit_nodes", result = "count", amount = analyze_stats.recorded_hit_nodes as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_snapshot_eligibility_hit_subtrees", result = "count", amount = analyze_stats.snapshot_eligibility_hit_subtrees as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_snapshot_eligibility_hit_nodes", result = "count", amount = analyze_stats.snapshot_eligibility_hit_nodes as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_composite_blocked_subtrees", result = "count", amount = analyze_stats.composite_blocked_subtrees as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_composite_blocked_nodes", result = "count", amount = analyze_stats.composite_blocked_nodes as u64);
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_apply_changed_nodes", result = "count", amount = apply_changed_stats.apply_changed_nodes as u64);
    }
    #[cfg(feature = "profile")]
    drop(_display_span);

    let scene_plan = plan_for_scene(&layout_pass, apply_changed_stats.apply_changed_nodes);
    let root_fingerprint = annotated.root_node().recorded_subtree_fingerprint;

    let scene_snapshot_decision = scene_snapshot_cache_decision(
        &scene_plan,
        cache.last_scene_snapshot.as_ref(),
        &frame_ctx,
        root_fingerprint,
    );

    if scene_snapshot_decision == SceneSnapshotCacheDecision::Hit {
        #[cfg(feature = "profile")]
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "scene_snapshot",
            result = "hit",
            amount = 1_u64
        );
        let entry = cache
            .last_scene_snapshot
            .as_ref()
            .expect("scene snapshot hit requires cached entry");
        let frame = entry.frame.clone();
        // Scene snapshot reuses the DrawOpFrame; generated RGBA is still resolved
        // from the pipeline table so the host contract stays complete.
        let media_plan = build_media_plan(&frame, generated_images);
        return Ok((frame, media_plan));
    }

    #[cfg(feature = "profile")]
    event!(
        target: "render.cache",
        Level::TRACE,
        kind = "cache",
        name = "scene_snapshot",
        result = "miss",
        amount = 1_u64
    );
    #[cfg(feature = "profile")]
    if let SceneSnapshotCacheDecision::Miss(reason) = scene_snapshot_decision {
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "scene_snapshot_miss",
            result = reason.as_profile_result(),
            amount = 1_u64
        );
        if reason == SceneSnapshotMissReason::PlanBlocked {
            record_scene_snapshot_plan_block_reasons(&scene_plan);
        }
    }

    let ordered_scene = OrderedSceneProgram::build(&annotated);
    *last_ordered_scene = ordered_scene.clone();

    let mut builder = DrawOpBuilder::default();
    let mut ctx = RenderCtx {
        catalog: &*catalog,
        frame_ctx: &frame_ctx,
        display_tree: &annotated,
        ordered_scene: &ordered_scene,
        builder: &mut builder,
        font_db: font_db.as_ref(),
        hidden_picture_stack: Vec::new(),
        generated_images,
    };

    crate::text::scope_font_db(font_db, || {
        crate::render::dispatch::render_display_tree(&mut ctx, &annotated, cache)
    })?;

    let frame = builder.finish();
    let media_plan = build_media_plan(&frame, generated_images);
    cache.last_scene_snapshot = Some(SceneSnapshotEntry {
        frame: frame.clone(),
        width: frame_ctx.width,
        height: frame_ctx.height,
        root_fingerprint,
    });
    Ok((frame, media_plan))
}

/// Decide whether the cached whole-frame DrawOp recording can be reused this
/// frame. The cache hits only when:
///   1. the scene-level plan allows it (no structure/layout/raster/apply change),
///   2. the cached entry was recorded at the same viewport, and
///   3. the root subtree fingerprint matches — this catches per-frame item
///      content changes (transition progress, animated text, frame-bound
///      values) that aren't visible to the layout/composite signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SceneSnapshotCacheDecision {
    Hit,
    Miss(SceneSnapshotMissReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SceneSnapshotMissReason {
    PlanBlocked,
    Empty,
    ViewportChanged,
    RootFingerprintChanged,
}

impl SceneSnapshotMissReason {
    #[cfg(feature = "profile")]
    fn as_profile_result(self) -> &'static str {
        match self {
            SceneSnapshotMissReason::PlanBlocked => "plan_blocked",
            SceneSnapshotMissReason::Empty => "empty",
            SceneSnapshotMissReason::ViewportChanged => "viewport_changed",
            SceneSnapshotMissReason::RootFingerprintChanged => "root_fingerprint_changed",
        }
    }
}

fn scene_snapshot_cache_decision(
    plan: &crate::analyze::compositor::SceneRenderPlan,
    cached: Option<&crate::ir::cache::SceneSnapshotEntry>,
    frame_ctx: &FrameCtx,
    current_root_fingerprint: crate::display::tree::DisplayRecordedSubtreeFingerprint,
) -> SceneSnapshotCacheDecision {
    if !plan.allows_scene_snapshot_cache {
        return SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::PlanBlocked);
    }

    let Some(entry) = cached else {
        return SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::Empty);
    };

    if entry.width != frame_ctx.width || entry.height != frame_ctx.height {
        return SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::ViewportChanged);
    }

    if entry.root_fingerprint != current_root_fingerprint {
        return SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::RootFingerprintChanged);
    }

    SceneSnapshotCacheDecision::Hit
}

#[cfg(feature = "profile")]
fn record_scene_snapshot_plan_block_reasons(plan: &crate::analyze::compositor::SceneRenderPlan) {
    if plan.scene_snapshot_blocked_by_structure {
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "scene_snapshot_plan_blocked",
            result = "structure",
            amount = 1_u64
        );
    }
    if plan.scene_snapshot_blocked_by_layout {
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "scene_snapshot_plan_blocked",
            result = "layout",
            amount = 1_u64
        );
    }
    if plan.scene_snapshot_blocked_by_raster {
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "scene_snapshot_plan_blocked",
            result = "raster",
            amount = 1_u64
        );
    }
    if plan.scene_snapshot_blocked_by_apply_change {
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "cache",
            name = "scene_snapshot_plan_blocked",
            result = "apply_change",
            amount = 1_u64
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::compositor::SceneRenderPlan;
    use crate::display::tree::DisplayRecordedSubtreeFingerprint;
    use crate::ir::DrawOpFrame;
    use crate::ir::cache::SceneSnapshotEntry;

    fn ctx(width: i32, height: i32) -> FrameCtx {
        FrameCtx {
            frame: 0,
            fps: 30,
            width,
            height,
            frames: 1,
        }
    }

    fn entry(width: i32, height: i32, fp: u64) -> SceneSnapshotEntry {
        SceneSnapshotEntry {
            frame: DrawOpFrame::default(),
            width,
            height,
            root_fingerprint: DisplayRecordedSubtreeFingerprint(fp),
        }
    }

    fn fp(value: u64) -> DisplayRecordedSubtreeFingerprint {
        DisplayRecordedSubtreeFingerprint(value)
    }

    fn scene_plan(allows_scene_snapshot_cache: bool) -> SceneRenderPlan {
        SceneRenderPlan {
            allows_scene_snapshot_cache,
            scene_snapshot_blocked_by_structure: !allows_scene_snapshot_cache,
            scene_snapshot_blocked_by_layout: false,
            scene_snapshot_blocked_by_raster: false,
            scene_snapshot_blocked_by_apply_change: false,
        }
    }

    #[test]
    fn reuses_when_plan_allows_cache_present_viewport_and_fingerprint_match() {
        let plan = scene_plan(true);
        let cached = entry(100, 50, 0xABCD);
        assert_eq!(
            scene_snapshot_cache_decision(&plan, Some(&cached), &ctx(100, 50), fp(0xABCD),),
            SceneSnapshotCacheDecision::Hit
        );
    }

    #[test]
    fn miss_reason_is_plan_blocked_when_plan_disallows_cache() {
        let plan = scene_plan(false);
        let cached = entry(100, 50, 0xABCD);
        assert_eq!(
            scene_snapshot_cache_decision(&plan, Some(&cached), &ctx(100, 50), fp(0xABCD),),
            SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::PlanBlocked)
        );
    }

    #[test]
    fn miss_reason_is_empty_on_first_frame() {
        let plan = scene_plan(true);
        assert_eq!(
            scene_snapshot_cache_decision(&plan, None, &ctx(100, 50), fp(0xABCD),),
            SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::Empty)
        );
    }

    #[test]
    fn miss_reason_is_viewport_changed_on_width_change() {
        let plan = scene_plan(true);
        let cached = entry(100, 50, 0xABCD);
        assert_eq!(
            scene_snapshot_cache_decision(&plan, Some(&cached), &ctx(200, 50), fp(0xABCD),),
            SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::ViewportChanged)
        );
    }

    #[test]
    fn miss_reason_is_viewport_changed_on_height_change() {
        let plan = scene_plan(true);
        let cached = entry(100, 50, 0xABCD);
        assert_eq!(
            scene_snapshot_cache_decision(&plan, Some(&cached), &ctx(100, 80), fp(0xABCD),),
            SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::ViewportChanged)
        );
    }

    #[test]
    fn miss_reason_is_root_fingerprint_changed_when_root_differs() {
        // Plan/viewport agree, but the root subtree fingerprint changed - a
        // per-frame item content change (e.g. transition progress) must
        // invalidate the whole-frame cache.
        let plan = scene_plan(true);
        let cached = entry(100, 50, 0xAAAA);
        assert_eq!(
            scene_snapshot_cache_decision(&plan, Some(&cached), &ctx(100, 50), fp(0xBBBB),),
            SceneSnapshotCacheDecision::Miss(SceneSnapshotMissReason::RootFingerprintChanged)
        );
    }
}
