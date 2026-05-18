//! Per-frame pipeline — core manages the entire chain from element resolve to canvas draw.

use std::cell::RefCell;

use anyhow::Result;

#[cfg(feature = "profile")]
use tracing::{Level, event, span};

use crate::canvas::Canvas2D;
use crate::display::build::build_display_tree;
use crate::element::resolve::resolve_ui_tree_with_script_cache;
use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::platform::platform::Platform;
use crate::platform::video::VideoFrameProvider;
use crate::render::{RenderCache, RenderCtx};
use std::marker::PhantomData;
use crate::render::display_tree::render_display_tree;
use crate::resource::blob_store::BlobStore;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::runtime::annotation::{annotate_display_tree, compute_display_tree_fingerprints};
use crate::runtime::compositor::{OrderedSceneProgram, plan_for_scene};
use crate::runtime::invalidation::mark_display_tree_composite_dirty;
use crate::runtime::session::RenderSession;
use crate::scene::composition::Composition;
use crate::scene::path_bounds::DefaultPathBounds;
use crate::scene::script::ScriptHost;
use crate::layout::LayoutSession;
use crate::runtime::invalidation::CompositeHistory;
use crate::text::DefaultFontProvider;

/// Generic per-frame pipeline: resolve → layout → display tree → annotate → plan → render.
///
/// Takes a `Canvas2D` directly, along with all the individual components that
/// would normally be unpacked from a `RenderSession`.
pub fn render_frame_inner<C: Canvas2D>(
    composition: &Composition,
    frame_index: u32,
    canvas: &mut C,
    layout_session: &mut LayoutSession,
    composite_history: &mut CompositeHistory,
    font_db: &std::sync::Arc<fontdb::Database>,
    catalog: &mut HashMapResourceCatalog,
    cache: &mut RenderCache<C>,
    last_ordered_scene: &mut OrderedSceneProgram,
    script: &mut dyn ScriptHost,
    video: &mut dyn VideoFrameProvider,
    blob_store: Option<&dyn BlobStore>,
) -> Result<()> {
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

    // 1. element resolve (with script)
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

    // 2. layout (sub-spans emitted inside compute_layout_with_font_db)
    let provider = DefaultFontProvider::from_arc(font_db.clone());
    let (layout_tree, layout_pass) = layout_session.compute_layout_with_provider(
        &element_root,
        &frame_ctx,
        &provider,
    )?;

    #[cfg(feature = "profile")]
    {
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "reused_nodes", result = "count", amount = layout_pass.reused_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "layout_dirty", result = "count", amount = layout_pass.layout_dirty_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "raster_dirty", result = "count", amount = layout_pass.raster_dirty_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "composite_dirty", result = "count", amount = layout_pass.composite_dirty_nodes as u64);
        event!(target: "render.layout", Level::TRACE, kind = "layout", name = "structure_rebuild", result = "count", amount = layout_pass.structure_rebuild as u64);
    }

    // 3. display tree + annotation + fingerprint
    #[cfg(feature = "profile")]
    let _display_span = span!(target: "render.scene", Level::TRACE, "build_display_tree").entered();
    let display_tree = build_display_tree(&element_root, &layout_tree)?;
    let mut annotated = annotate_display_tree(&display_tree);
    mark_display_tree_composite_dirty(
        composite_history,
        &mut annotated,
        layout_pass.structure_rebuild,
    );
    compute_display_tree_fingerprints(&mut annotated);
    #[cfg(feature = "profile")]
    drop(_display_span);

    // 4. plan
    let scene_plan = plan_for_scene(&layout_pass, annotated.contains_time_variant());

    // 5/6. snapshot cache decision
    if scene_plan.allows_scene_snapshot_cache {
        if let Some((_, ref snapshot)) = cache.scene_snapshot {
            #[cfg(feature = "profile")]
            event!(
                target: "render.cache",
                Level::TRACE,
                kind = "cache",
                name = "scene_snapshot",
                result = "hit",
                amount = 1_u64
            );
            canvas.draw_picture(snapshot, None, None);
            return Ok(());
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
        let ordered_scene = OrderedSceneProgram::build(&annotated);
        let bounds = crate::canvas::Rect::new(
            0.0, 0.0,
            composition.width as f64,
            composition.height as f64,
        );
        let snapshot = canvas.make_picture(&bounds, |rec_canvas| {
            let snapshot_ctx = RenderCtx {
                catalog,
                frame_ctx: &frame_ctx,
                display_tree: &annotated,
                ordered_scene: &ordered_scene,
                video: RefCell::new(video),
                blob_store,
                platform_data: &mut (),
                _phantom: PhantomData,
            };
            let _ = render_display_tree(rec_canvas, &annotated, &snapshot_ctx, cache);
        });
        cache.scene_snapshot = Some((0, snapshot.clone()));
        canvas.draw_picture(&snapshot, None, None);
        return Ok(());
    }

    // 6. ordered scene direct draw
    cache.scene_snapshot = None;
    let ordered_scene = OrderedSceneProgram::build(&annotated);
    *last_ordered_scene = ordered_scene.clone();

    let ctx = RenderCtx {
        catalog,
        frame_ctx: &frame_ctx,
        display_tree: &annotated,
        ordered_scene: &ordered_scene,
        video: RefCell::new(video),
        blob_store,
        platform_data: &mut (),
        _phantom: PhantomData,
    };
    render_display_tree(canvas, &annotated, &ctx, cache)?;
    Ok(())
}

/// Session-based wrapper: deconstructs `RenderSession<P, C>` and calls
/// `render_frame_inner`.
pub fn render_frame<P: Platform, C: Canvas2D>(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession<P, C>,
    canvas: &mut C,
    blob_store: Option<&dyn BlobStore>,
) -> Result<()> {
    let RenderSession {
        ref mut layout_session,
        ref mut composite_history,
        ref font_db,
        ref mut catalog,
        cache: ref mut cache_field,
        last_ordered_scene: ref mut last_ordered,
        ref mut platform,
        ..
    } = *session;

    // SAFETY: script_host() and video_source() access disjoint fields
    // on every Platform implementation. The borrow checker cannot see this
    // through trait method calls, so we use a raw pointer to split the borrow.
    let platform_ptr: *mut P = platform;
    let script = unsafe { (*platform_ptr).script_host() };
    let video = unsafe { (*platform_ptr).video_source() };

    render_frame_inner(
        composition,
        frame_index,
        canvas,
        layout_session,
        composite_history,
        font_db,
        catalog,
        cache_field,
        last_ordered,
        script,
        video,
        blob_store,
    )
}
