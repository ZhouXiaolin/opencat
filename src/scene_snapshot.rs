use anyhow::{Result, anyhow};
use skia_safe::{Canvas, Picture};

use crate::{
    assets::AssetsMap,
    backend::{
        resource_cache::BackendResourceCache,
        skia::{
            SkiaBackend, draw_display_tree_with_subtree_cache,
            record_display_list_composite_source,
            record_display_tree_composite_source_with_subtree_cache,
        },
    },
    display::list::DisplayList,
    display::tree::DisplayTree,
    frame_ctx::FrameCtx,
    media::MediaContext,
    profile::{BackendProfile, SceneBuildStats},
    render::invalidation::{SceneInvalidation, invalidation_for_scene},
    render_cache::{SceneSlot, SceneSnapshotCache},
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
    pub width: i32,
    pub height: i32,
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
    canvas: Option<&Canvas>,
) -> Result<Option<SceneSnapshot>> {
    if let Some(snapshot) = resolve_scene_snapshot_for_slot(
        runtime,
        slot,
        display_tree,
        display_list,
        plan,
        require_scene_snapshot,
    )? {
        if let Some(canvas) = canvas {
            snapshot.draw(canvas, Some(&mut *runtime.backend_profile))?;
            return Ok(None);
        }
        return Ok(Some(snapshot));
    }

    if require_scene_snapshot {
        return Err(anyhow!(
            "scene snapshot is required for slot but no snapshot was produced"
        ));
    }

    let canvas = canvas.ok_or_else(|| {
        anyhow!("canvas is required when scene rendering falls back to direct draw")
    })?;
    draw_scene_without_snapshot(runtime, display_tree, display_list, plan, canvas)?;
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
    if plan.contains_video {
        runtime.scene_snapshots.store_scene_snapshot(slot, None);
        if !require_scene_snapshot {
            return Ok(None);
        }
        return record_display_tree_scene(runtime, display_tree).map(Some);
    }

    if plan.allows_cache_reuse() {
        if let Some(snapshot) = runtime.scene_snapshots.scene_snapshot(slot) {
            runtime.backend_profile.picture_cache_hits += 1;
            return Ok(Some(snapshot));
        }

        let snapshot = record_display_list_scene(runtime, display_list)?;
        runtime.backend_profile.picture_cache_misses += 1;
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
        return record_display_tree_scene(runtime, display_tree).map(Some);
    }
    record_display_list_scene(runtime, display_list).map(Some)
}

fn draw_scene_without_snapshot(
    runtime: &mut SceneSnapshotRuntime<'_>,
    display_tree: &DisplayTree,
    display_list: &DisplayList,
    plan: SceneSnapshotPlan,
    canvas: &Canvas,
) -> Result<()> {
    if plan.strategy == SceneSnapshotStrategy::DisplayTreeWithSubtreeCache {
        draw_display_tree_with_subtree_cache(
            display_tree,
            canvas,
            runtime.assets,
            runtime.backend_resources.image_cache(),
            runtime.backend_resources.text_picture_cache(),
            runtime.backend_resources.subtree_picture_cache(),
            Some(&mut *runtime.media_ctx),
            runtime.frame_ctx,
            Some(&mut *runtime.backend_profile),
        )?;
        return Ok(());
    }

    let mut backend = SkiaBackend::new_with_cache_and_profile(
        canvas,
        runtime.width,
        runtime.height,
        runtime.assets,
        runtime.backend_resources.image_cache(),
        runtime.backend_resources.text_picture_cache(),
        None,
        Some(&mut *runtime.media_ctx),
        runtime.frame_ctx,
        Some(&mut *runtime.backend_profile),
    );
    backend.execute(display_list)
}

fn record_display_tree_scene(
    runtime: &mut SceneSnapshotRuntime<'_>,
    display_tree: &DisplayTree,
) -> Result<SceneSnapshot> {
    record_display_tree_composite_source_with_subtree_cache(
        display_tree,
        runtime.width,
        runtime.height,
        runtime.assets,
        runtime.backend_resources.image_cache(),
        runtime.backend_resources.text_picture_cache(),
        runtime.backend_resources.subtree_picture_cache(),
        Some(&mut *runtime.media_ctx),
        runtime.frame_ctx,
        Some(&mut *runtime.backend_profile),
    )
}

fn record_display_list_scene(
    runtime: &mut SceneSnapshotRuntime<'_>,
    display_list: &DisplayList,
) -> Result<SceneSnapshot> {
    record_display_list_composite_source(
        display_list,
        runtime.width,
        runtime.height,
        runtime.assets,
        runtime.backend_resources.image_cache(),
        runtime.backend_resources.text_picture_cache(),
        Some(&mut *runtime.media_ctx),
        runtime.frame_ctx,
        Some(&mut *runtime.backend_profile),
    )
}

#[derive(Clone)]
pub(crate) struct SceneSnapshot {
    picture: Picture,
}

impl SceneSnapshot {
    pub(crate) fn new(picture: Picture) -> Self {
        Self { picture }
    }

    pub(crate) fn draw(
        &self,
        canvas: &Canvas,
        mut profile: Option<&mut BackendProfile>,
    ) -> Result<()> {
        let started = std::time::Instant::now();
        canvas.draw_picture(&self.picture, None, None);
        if let Some(profile) = profile.as_deref_mut() {
            profile.picture_draw_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        Ok(())
    }

    pub(crate) fn picture(&self) -> Result<&Picture> {
        if self.picture.cull_rect().is_empty() {
            return Err(anyhow!("scene snapshot picture has empty bounds"));
        }
        Ok(&self.picture)
    }
}
