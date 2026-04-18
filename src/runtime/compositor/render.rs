use anyhow::{Result, anyhow};

use crate::{
    frame_ctx::FrameCtx,
    resource::{assets::AssetsMap, media::MediaContext},
    runtime::{
        annotation::AnnotatedDisplayTree,
        cache::CacheRegistry,
        compositor::record_layered_scene,
        frame_view::RenderFrameView,
        profile::{BackendCountMetric, backend_span, record_backend_count},
        render_engine::{SceneRenderContext, SceneSnapshot, SharedRenderEngine},
    },
};

use super::{SceneRenderPlan, SceneSlot, SceneSnapshotCache};

pub(crate) struct SceneRenderRuntime<'a> {
    pub assets: &'a AssetsMap,
    pub scene_snapshots: &'a mut SceneSnapshotCache,
    pub cache_registry: &'a CacheRegistry,
    pub media_ctx: &'a mut MediaContext,
    pub frame_ctx: &'a FrameCtx,
    pub render_engine: SharedRenderEngine,
    pub width: i32,
    pub height: i32,
}

impl<'a> SceneRenderRuntime<'a> {
    fn render_context(&mut self) -> SceneRenderContext<'_> {
        SceneRenderContext {
            assets: self.assets,
            cache_registry: self.cache_registry,
            media_ctx: &mut *self.media_ctx,
            frame_ctx: self.frame_ctx,
            width: self.width,
            height: self.height,
        }
    }
}

pub(crate) fn render_scene_slot(
    runtime: &mut SceneRenderRuntime<'_>,
    slot: SceneSlot,
    display_tree: &AnnotatedDisplayTree,
    plan: SceneRenderPlan,
    require_scene_snapshot: bool,
    frame_view: Option<RenderFrameView>,
) -> Result<Option<SceneSnapshot>> {
    let engine = runtime.render_engine.clone();
    if let Some(snapshot) =
        resolve_scene_snapshot_for_slot(runtime, slot, display_tree, plan, require_scene_snapshot)?
    {
        if let Some(frame_view) = frame_view {
            let _profile_span = backend_span("scene_snapshot_present");
            engine.draw_scene_snapshot(&snapshot, frame_view)?;
            return Ok(None);
        }
        return Ok(Some(snapshot));
    }

    if require_scene_snapshot {
        return Err(anyhow!(
            "scene snapshot is required for slot but no snapshot was produced"
        ));
    }

    let frame_view = frame_view.ok_or_else(|| {
        anyhow!("frame view is required when scene rendering falls back to direct draw")
    })?;
    if plan.renders_layered_scene() {
        let mut render_context = runtime.render_context();
        let layered_scene =
            record_layered_scene(&mut render_context, engine.clone(), display_tree)?;
        layered_scene.compose(&mut render_context, engine, display_tree, frame_view)?;
        return Ok(None);
    }

    let mut render_context = runtime.render_context();
    engine.draw_display_tree(&mut render_context, display_tree, frame_view)?;
    Ok(None)
}

fn resolve_scene_snapshot_for_slot(
    runtime: &mut SceneRenderRuntime<'_>,
    slot: SceneSlot,
    display_tree: &AnnotatedDisplayTree,
    plan: SceneRenderPlan,
    require_scene_snapshot: bool,
) -> Result<Option<SceneSnapshot>> {
    let engine = runtime.render_engine.clone();
    if plan.renders_layered_scene() {
        runtime.scene_snapshots.store_scene_snapshot(slot, None);
        if !require_scene_snapshot {
            return Ok(None);
        }
        let mut render_context = runtime.render_context();
        return engine
            .record_display_tree_snapshot(&mut render_context, display_tree)
            .map(Some);
    }

    if plan.allows_scene_snapshot_cache {
        if let Some(snapshot) = runtime.scene_snapshots.scene_snapshot(slot) {
            record_backend_count(BackendCountMetric::SceneSnapshotCacheHit, 1);
            return Ok(Some(snapshot));
        }

        let mut render_context = runtime.render_context();
        let snapshot = engine.record_display_tree_snapshot(&mut render_context, display_tree)?;
        record_backend_count(BackendCountMetric::SceneSnapshotCacheMiss, 1);
        runtime
            .scene_snapshots
            .store_scene_snapshot(slot, Some(snapshot.clone()));
        return Ok(Some(snapshot));
    }

    runtime.scene_snapshots.store_scene_snapshot(slot, None);
    if !require_scene_snapshot {
        return Ok(None);
    }

    let mut render_context = runtime.render_context();
    engine
        .record_display_tree_snapshot(&mut render_context, display_tree)
        .map(Some)
}
