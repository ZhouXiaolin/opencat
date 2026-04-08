use anyhow::{Result, anyhow};

use crate::{
    backend::resource_cache::BackendResourceCache,
    display::list::DisplayList,
    display::tree::DisplayTree,
    frame_ctx::FrameCtx,
    resource::{assets::AssetsMap, media::MediaContext},
    runtime::{
        frame_view::RenderFrameView,
        policy::{
            cache::{SceneSlot, SceneSnapshotCache},
            invalidation::{SceneInvalidation, invalidation_for_scene},
        },
        profile::{BackendProfile, SceneBuildStats},
        render_engine::{SceneRenderContext, SceneSnapshot, SharedRenderEngine},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneSnapshotStrategy {
    DisplayList,
    DisplayTreeWithSubtreeCache,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SceneSnapshotPlan {
    pub strategy: SceneSnapshotStrategy,
    pub invalidation: SceneInvalidation,
    pub contains_video: bool,
}

impl SceneSnapshotPlan {
    pub(crate) fn from_scene(invalidation: SceneInvalidation, contains_video: bool) -> Self {
        let strategy = if contains_video || invalidation.prefers_subtree_cache() {
            SceneSnapshotStrategy::DisplayTreeWithSubtreeCache
        } else {
            SceneSnapshotStrategy::DisplayList
        };
        Self {
            strategy,
            invalidation,
            contains_video,
        }
    }

    pub(crate) fn allows_cache_reuse(self) -> bool {
        self.invalidation.allows_picture_reuse()
    }
}

pub(crate) struct SceneSnapshotRuntime<'a> {
    pub assets: &'a AssetsMap,
    pub scene_snapshots: &'a mut SceneSnapshotCache,
    pub backend_resources: &'a BackendResourceCache,
    pub media_ctx: &'a mut MediaContext,
    pub frame_ctx: &'a FrameCtx,
    pub backend_profile: &'a mut BackendProfile,
    pub render_engine: SharedRenderEngine,
    pub width: i32,
    pub height: i32,
}

impl<'a> SceneSnapshotRuntime<'a> {
    fn render_context(&mut self) -> SceneRenderContext<'_> {
        SceneRenderContext {
            assets: self.assets,
            backend_resources: self.backend_resources,
            media_ctx: &mut *self.media_ctx,
            frame_ctx: self.frame_ctx,
            backend_profile: &mut *self.backend_profile,
            width: self.width,
            height: self.height,
        }
    }
}

pub(crate) fn plan_for_scene(scene_stats: &SceneBuildStats) -> SceneSnapshotPlan {
    SceneSnapshotPlan::from_scene(
        invalidation_for_scene(scene_stats),
        scene_stats.contains_video,
    )
}

pub(crate) fn render_scene_slot(
    runtime: &mut SceneSnapshotRuntime<'_>,
    slot: SceneSlot,
    display_tree: &DisplayTree,
    display_list: &DisplayList,
    plan: SceneSnapshotPlan,
    require_scene_snapshot: bool,
    frame_view: Option<RenderFrameView>,
) -> Result<Option<SceneSnapshot>> {
    let engine = runtime.render_engine.clone();
    if let Some(snapshot) = resolve_scene_snapshot_for_slot(
        runtime,
        slot,
        display_tree,
        display_list,
        plan,
        require_scene_snapshot,
    )? {
        if let Some(frame_view) = frame_view {
            engine.draw_scene_snapshot(
                &snapshot,
                frame_view,
                Some(&mut *runtime.backend_profile),
            )?;
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
    let mut render_context = runtime.render_context();
    engine.draw_scene_without_snapshot(
        &mut render_context,
        display_tree,
        display_list,
        plan,
        frame_view,
    )?;
    Ok(None)
}

fn resolve_scene_snapshot_for_slot(
    runtime: &mut SceneSnapshotRuntime<'_>,
    slot: SceneSlot,
    display_tree: &DisplayTree,
    display_list: &DisplayList,
    plan: SceneSnapshotPlan,
    require_scene_snapshot: bool,
) -> Result<Option<SceneSnapshot>> {
    let engine = runtime.render_engine.clone();
    if plan.contains_video {
        runtime.scene_snapshots.store_scene_snapshot(slot, None);
        if !require_scene_snapshot {
            return Ok(None);
        }
        let mut render_context = runtime.render_context();
        return engine
            .record_display_tree_snapshot(&mut render_context, display_tree)
            .map(Some);
    }

    if plan.allows_cache_reuse() {
        if let Some(snapshot) = runtime.scene_snapshots.scene_snapshot(slot) {
            runtime.backend_profile.scene_snapshot_cache_hits += 1;
            return Ok(Some(snapshot));
        }

        let mut render_context = runtime.render_context();
        let snapshot = engine.record_display_list_snapshot(&mut render_context, display_list)?;
        runtime.backend_profile.scene_snapshot_cache_misses += 1;
        runtime
            .scene_snapshots
            .store_scene_snapshot(slot, Some(snapshot.clone()));
        return Ok(Some(snapshot));
    }

    runtime.scene_snapshots.store_scene_snapshot(slot, None);
    if !require_scene_snapshot {
        return Ok(None);
    }

    if plan.strategy == SceneSnapshotStrategy::DisplayTreeWithSubtreeCache {
        let mut render_context = runtime.render_context();
        return engine
            .record_display_tree_snapshot(&mut render_context, display_tree)
            .map(Some);
    }
    let mut render_context = runtime.render_context();
    engine
        .record_display_list_snapshot(&mut render_context, display_list)
        .map(Some)
}
