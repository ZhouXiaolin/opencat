use anyhow::Result;
use tracing::{Level, span};

use crate::{
    core::display::build::build_display_tree,
    core::element::resolve::resolve_ui_tree_with_script_cache,
    core::frame_ctx::{FrameCtx, ScriptFrameCtx},
    resource::catalog::ResourceCatalog,
    runtime::{
        annotation::{
            AnnotatedDisplayTree, annotate_display_tree, compute_display_tree_fingerprints,
        },
        compositor::{SceneRenderRuntime, plan_for_scene, render_scene},
        frame_view::RenderFrameView,
        invalidation::{CompositeHistory, mark_display_tree_composite_dirty},
        preflight::ensure_assets_preloaded,
        profile::SceneBuildStats,
        session::RenderSession,
    },
    core::scene::{
        composition::Composition,
        node::Node,
        script::{ScriptRuntimeCache, StyleMutations},
    },
    core::text::FontProvider,
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
    let (annotated_display_tree, scene_stats) = build_scene_display_list(
        &root,
        &frame_ctx,
        &ScriptFrameCtx::global(&frame_ctx),
        session,
        _mutations.as_ref(),
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
    render_scene(
        &mut snapshot_runtime,
        &annotated_display_tree,
        snapshot_plan,
        false,
        Some(frame_view),
    )?;
    Ok(())
}

pub fn build_frame_display_tree(
    scene: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    catalog: &mut dyn ResourceCatalog,
    fonts: &dyn FontProvider,
    layout_session: &mut crate::core::layout::LayoutSession,
    composite_history: &mut CompositeHistory,
    script_cache: &mut ScriptRuntimeCache,
    mutations: Option<&StyleMutations>,
) -> Result<(AnnotatedDisplayTree, SceneBuildStats)> {
    let mut stats = SceneBuildStats::default();

    let element_root = resolve_ui_tree_with_script_cache(
        scene,
        frame_ctx,
        script_frame_ctx,
        catalog,
        mutations,
        script_cache,
    )?;

    let (layout_tree, layout_pass) = layout_session
        .compute_layout_with_provider(&element_root, frame_ctx, fonts)?;
    stats.layout_pass = layout_pass;

    let display_tree = build_display_tree(&element_root, &layout_tree)?;
    let mut annotated = annotate_display_tree(&display_tree);
    mark_display_tree_composite_dirty(
        composite_history,
        &mut annotated,
        layout_pass.structure_rebuild,
    );
    compute_display_tree_fingerprints(&mut annotated);
    stats.contains_time_variant_paint = annotated.contains_time_variant();

    Ok((annotated, stats))
}

pub(crate) fn build_scene_display_list(
    scene: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    session: &mut RenderSession,
    mutations: Option<&StyleMutations>,
) -> Result<(AnnotatedDisplayTree, SceneBuildStats)> {
    let provider = crate::core::text::DefaultFontProvider::from_arc(session.font_db.clone());
    let RenderSession {
        assets,
        layout_session,
        composite_history,
        script_runtime,
        ..
    } = session;
    build_frame_display_tree(
        scene,
        frame_ctx,
        script_frame_ctx,
        assets,
        &provider,
        layout_session,
        composite_history,
        script_runtime,
        mutations,
    )
}
