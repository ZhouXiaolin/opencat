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
use crate::analyze::invalidation::{CompositeHistory, mark_display_tree_composite_dirty};
use crate::display::build::build_display_tree;
use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::ir::cache::RenderCache;
use crate::ir::{DrawOpFrame, FrameMediaPlan};
use crate::layout::LayoutSession;
use crate::parse::composition::Composition;
use crate::render::RenderCtx;
use crate::render::builder::DrawOpBuilder;
use crate::render::media_plan::build_media_plan;
use crate::resolve::path_bounds::DefaultPathBounds;
use crate::resolve::resolve::resolve_ui_tree_with_script_cache;
use crate::resource::blob_store::BlobStore;
use crate::resource::catalog::ResourceCatalog;
use crate::runtime::session::RenderSession;
use crate::script::ScriptHost;
use crate::text::DefaultFontProvider;

#[allow(clippy::too_many_arguments)]
pub fn render_frame_with_state(
    composition: &Composition,
    frame_index: u32,
    layout_session: &mut LayoutSession,
    composite_history: &mut CompositeHistory,
    analyze_fingerprint_history: &mut AnalyzeFingerprintHistory,
    font_db: &Arc<fontdb::Database>,
    catalog: &mut dyn ResourceCatalog,
    cache: &mut RenderCache,
    last_ordered_scene: &mut OrderedSceneProgram,
    script: &mut dyn ScriptHost,
    blob_store: Option<&dyn BlobStore>,
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

    let path_bounds = DefaultPathBounds;
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
    let element_root = resolve_ui_tree_with_script_cache(
        &root,
        &frame_ctx,
        &script_frame_ctx,
        catalog,
        None,
        script,
        &path_bounds,
    )?;
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
    let display_tree = build_display_tree(&element_root, &layout_tree, &frame_ctx)?;
    let mut annotated = annotate_display_tree(&display_tree);
    let composite_dirty_stats = mark_display_tree_composite_dirty(
        composite_history,
        &mut annotated,
        layout_pass.structure_rebuild,
    );
    #[cfg(not(feature = "profile"))]
    let _ = composite_dirty_stats;
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
        event!(target: "render.analyze", Level::TRACE, kind = "analyze", name = "analyze_composite_dirty_nodes", result = "count", amount = composite_dirty_stats.composite_dirty_nodes as u64);
    }
    #[cfg(feature = "profile")]
    drop(_display_span);

    let _scene_plan = plan_for_scene(&layout_pass, composite_dirty_stats.composite_dirty_nodes);
    cache.scene_snapshot = None;
    let ordered_scene = OrderedSceneProgram::build(&annotated);
    *last_ordered_scene = ordered_scene.clone();

    let mut builder = DrawOpBuilder::default();
    let mut ctx = RenderCtx {
        catalog: &*catalog,
        frame_ctx: &frame_ctx,
        display_tree: &annotated,
        ordered_scene: &ordered_scene,
        builder: &mut builder,
        blob_store,
        hidden_picture_stack: Vec::new(),
    };

    crate::render::dispatch::render_display_tree(&mut ctx, &annotated, cache)?;

    let frame = builder.finish();
    let media_plan = build_media_plan(&frame);
    Ok((frame, media_plan))
}

pub fn render_frame(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
    script: &mut dyn ScriptHost,
    blob_store: Option<&dyn BlobStore>,
) -> Result<(DrawOpFrame, FrameMediaPlan)> {
    let RenderSession {
        ref mut layout_session,
        ref mut composite_history,
        ref mut analyze_fingerprint_history,
        ref font_db,
        ref mut catalog,
        cache: ref mut cache_field,
        last_ordered_scene: ref mut last_ordered,
        ..
    } = *session;

    render_frame_with_state(
        composition,
        frame_index,
        layout_session,
        composite_history,
        analyze_fingerprint_history,
        font_db,
        catalog,
        cache_field,
        last_ordered,
        script,
        blob_store,
    )
}
