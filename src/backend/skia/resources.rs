use crate::backend::skia::cache::{
    SkiaImageCache, SkiaSubtreeSnapshotCache, SkiaTextSnapshotCache, new_image_cache,
    new_subtree_snapshot_cache, new_text_snapshot_cache,
};

pub(crate) struct SkiaBackendResources {
    image_cache: SkiaImageCache,
    text_snapshot_cache: SkiaTextSnapshotCache,
    subtree_snapshot_cache: SkiaSubtreeSnapshotCache,
}

impl SkiaBackendResources {
    pub(crate) fn new() -> Self {
        Self {
            image_cache: new_image_cache(),
            text_snapshot_cache: new_text_snapshot_cache(),
            subtree_snapshot_cache: new_subtree_snapshot_cache(),
        }
    }

    pub(crate) fn image_cache(&self) -> SkiaImageCache {
        self.image_cache.clone()
    }

    pub(crate) fn text_snapshot_cache(&self) -> SkiaTextSnapshotCache {
        self.text_snapshot_cache.clone()
    }

    pub(crate) fn subtree_snapshot_cache(&self) -> SkiaSubtreeSnapshotCache {
        self.subtree_snapshot_cache.clone()
    }
}

impl Default for SkiaBackendResources {
    fn default() -> Self {
        Self::new()
    }
}
