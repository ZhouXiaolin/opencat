use anyhow::Result;

use crate::runtime::{
    annotation::AnnotatedDisplayTree,
    compositor::LayeredScene,
    fingerprint::scene_static_skeleton_fingerprint,
    profile::{BackendCountMetric, record_backend_count},
    render_engine::{SceneRenderContext, SharedRenderEngine},
};

pub(crate) fn record_layered_scene(
    runtime: &mut SceneRenderContext<'_>,
    render_engine: SharedRenderEngine,
    display_tree: &AnnotatedDisplayTree,
) -> Result<LayeredScene> {
    let skeleton_fp = scene_static_skeleton_fingerprint(
        &display_tree.root,
        &display_tree.analysis,
        &display_tree.invalidation,
    );
    let static_cache = runtime.cache_registry.scene_static_picture_cache();

    let static_layer = if let Some(snapshot) = static_cache.borrow_mut().get_cloned(&skeleton_fp) {
        record_backend_count(BackendCountMetric::SceneStaticCacheHit, 1);
        Some(snapshot)
    } else {
        let snapshot = render_engine.record_display_tree_static_snapshot(runtime, display_tree)?;
        static_cache
            .borrow_mut()
            .insert(skeleton_fp, snapshot.clone());
        record_backend_count(BackendCountMetric::SceneStaticCacheMiss, 1);
        Some(snapshot)
    };

    Ok(LayeredScene { static_layer })
}
