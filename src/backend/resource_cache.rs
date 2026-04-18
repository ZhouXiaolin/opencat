use crate::runtime::cache::{
    CacheCaps, CacheRegistry, ImageCache, ItemPictureCache, SceneStaticPictureCache,
    SubtreeSnapshotCache, TextSnapshotCache,
};

pub(crate) struct BackendResourceCache {
    registry: CacheRegistry,
}

impl BackendResourceCache {
    pub(crate) fn new(caps: CacheCaps) -> Self {
        Self {
            registry: CacheRegistry::new(caps),
        }
    }

    pub(crate) fn image_cache(&self) -> ImageCache {
        self.registry.image_cache()
    }

    pub(crate) fn text_snapshot_cache(&self) -> TextSnapshotCache {
        self.registry.text_snapshot_cache()
    }

    pub(crate) fn subtree_snapshot_cache(&self) -> SubtreeSnapshotCache {
        self.registry.subtree_snapshot_cache()
    }

    pub(crate) fn item_picture_cache(&self) -> ItemPictureCache {
        self.registry.item_picture_cache()
    }

    pub(crate) fn scene_static_picture_cache(&self) -> SceneStaticPictureCache {
        self.registry.scene_static_picture_cache()
    }
}

impl Default for BackendResourceCache {
    fn default() -> Self {
        Self::new(CacheCaps::default())
    }
}
