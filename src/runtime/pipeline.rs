use anyhow::Result;
use tracing::{Level, span};

use crate::{
    display::build::build_display_tree,
    element::resolve::resolve_ui_tree_with_script_cache,
    frame_ctx::{FrameCtx, ScriptFrameCtx},
    runtime::{
        annotation::{AnnotatedDisplayTree, annotate_display_tree},
        compositor::{SceneRenderRuntime, SceneSlot, plan_for_scene, render_scene_slot},
        frame_view::RenderFrameView,
        invalidation::mark_display_tree_composite_dirty,
        preflight::ensure_assets_preloaded,
        profile::SceneBuildStats,
        session::RenderSession,
    },
    scene::{composition::Composition, node::Node, script::StyleMutations},
};

pub(crate) fn render_frame_on_surface(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
    frame_view: RenderFrameView,
) -> Result<()> {
    ensure_assets_preloaded(composition, session)?;

    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };

    let _mutations: Option<StyleMutations> = None;

    let root_span = span!(
        target: "render.pipeline",
        Level::TRACE,
        "frame",
        frame = frame_index as u64,
        fps = composition.fps as i64,
        width = composition.width as i64,
        height = composition.height as i64,
        mode = tracing::field::Empty
    );
    let _root_guard = root_span.enter();

    let root = composition.root_node(&frame_ctx);
    let slot = SceneSlot::root_scene();
    let (annotated_display_tree, scene_stats) = build_scene_display_list_with_slot(
        &root,
        &frame_ctx,
        &ScriptFrameCtx::global(&frame_ctx),
        session,
        _mutations.as_ref(),
        slot.clone(),
    )?;
    let snapshot_plan = plan_for_scene(&scene_stats);

    let render_engine = session.render_engine_handle();
    let mut snapshot_runtime = SceneRenderRuntime {
        assets: &session.assets,
        scene_snapshots: &mut session.scene_snapshots,
        cache_registry: &session.cache_registry,
        media_ctx: &mut session.media_ctx,
        frame_ctx: &frame_ctx,
        render_engine,
        width: composition.width,
        height: composition.height,
    };
    render_scene_slot(
        &mut snapshot_runtime,
        &slot,
        &annotated_display_tree,
        snapshot_plan,
        false,
        Some(frame_view),
    )?;
    Ok(())
}

pub(crate) fn build_scene_display_list_with_slot(
    scene: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    session: &mut RenderSession,
    mutations: Option<&StyleMutations>,
    slot: SceneSlot,
) -> Result<(AnnotatedDisplayTree, SceneBuildStats)> {
    let mut stats = SceneBuildStats::default();

    let resolve_span = span!(target: "render.scene", Level::TRACE, "resolve_ui_tree");
    let element_root = {
        let _guard = resolve_span.enter();
        resolve_ui_tree_with_script_cache(
            scene,
            frame_ctx,
            script_frame_ctx,
            &mut session.media_ctx,
            &mut session.assets,
            mutations,
            &mut session.script_runtime,
        )?
    };

    let layout_span = span!(target: "render.scene", Level::TRACE, "compute_layout");
    let (layout_tree, layout_pass) = {
        let _guard = layout_span.enter();
        let text_engine = session.text_engine_handle();
        session
            .layout_session_mut(slot.clone())
            .compute_layout_with_text_engine(&element_root, frame_ctx, text_engine.as_ref())?
    };
    stats.layout_pass = layout_pass;

    let display_span = span!(target: "render.scene", Level::TRACE, "build_display_tree");
    let annotated_display_tree = {
        let _guard = display_span.enter();
        let display_tree = build_display_tree(&element_root, &layout_tree, &session.assets)?;
        let mut annotated = annotate_display_tree(&display_tree, &session.assets);
        mark_display_tree_composite_dirty(
            session.composite_history_mut(),
            slot,
            &mut annotated,
            stats.layout_pass.structure_rebuild,
        );
        annotated
    };
    stats.contains_time_variant_paint = annotated_display_tree.contains_time_variant();

    Ok((annotated_display_tree, stats))
}
