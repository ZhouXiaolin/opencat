use opencat_core::platform::scene_snapshot::SceneSnapshotCache as SceneSnapshotCacheGeneric;

use crate::backend::skia::SkiaBackend;

/// Skia-monomorphized scene snapshot cache.
pub type SceneSnapshotCache = SceneSnapshotCacheGeneric<SkiaBackend>;
