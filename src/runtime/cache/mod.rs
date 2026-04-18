pub mod lru;
pub mod video_frames;

use std::{cell::RefCell, rc::Rc};

use skia_safe::{Image as SkiaImage, Picture};

use crate::runtime::cache::lru::BoundedLruCache;
use crate::runtime::render_engine::SceneSnapshot;

pub(crate) type SharedLruCache<K, V> = Rc<RefCell<BoundedLruCache<K, V>>>;
pub(crate) type ImageCache = SharedLruCache<String, Option<SkiaImage>>;
pub(crate) type TextSnapshotCache = SharedLruCache<u64, Picture>;
pub(crate) type SubtreeSnapshotCache = SharedLruCache<u64, Picture>;
pub(crate) type ItemPictureCache = SharedLruCache<u64, Picture>;
pub(crate) type SceneStaticPictureCache = SharedLruCache<u64, SceneSnapshot>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheCaps {
    pub images: usize,
    pub text_snapshots: usize,
    pub subtree_snapshots: usize,
    pub item_pictures: usize,
    pub scene_static_pictures: usize,
    pub video_frames: usize,
}

impl Default for CacheCaps {
    fn default() -> Self {
        Self {
            images: 128,
            text_snapshots: 256,
            subtree_snapshots: 256,
            item_pictures: 256,
            scene_static_pictures: 256,
            video_frames: 64,
        }
    }
}

pub(crate) struct CacheRegistry {
    image_cache: ImageCache,
    text_snapshot_cache: TextSnapshotCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
    item_picture_cache: ItemPictureCache,
    scene_static_picture_cache: SceneStaticPictureCache,
}

impl CacheRegistry {
    pub(crate) fn new(caps: CacheCaps) -> Self {
        Self {
            image_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.images))),
            text_snapshot_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.text_snapshots))),
            subtree_snapshot_cache: Rc::new(RefCell::new(BoundedLruCache::new(
                caps.subtree_snapshots,
            ))),
            item_picture_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.item_pictures))),
            scene_static_picture_cache: Rc::new(RefCell::new(BoundedLruCache::new(
                caps.scene_static_pictures,
            ))),
        }
    }

    pub(crate) fn image_cache(&self) -> ImageCache {
        self.image_cache.clone()
    }

    pub(crate) fn text_snapshot_cache(&self) -> TextSnapshotCache {
        self.text_snapshot_cache.clone()
    }

    pub(crate) fn subtree_snapshot_cache(&self) -> SubtreeSnapshotCache {
        self.subtree_snapshot_cache.clone()
    }

    pub(crate) fn item_picture_cache(&self) -> ItemPictureCache {
        self.item_picture_cache.clone()
    }

    pub(crate) fn scene_static_picture_cache(&self) -> SceneStaticPictureCache {
        self.scene_static_picture_cache.clone()
    }
}

impl Default for CacheRegistry {
    fn default() -> Self {
        Self::new(CacheCaps::default())
    }
}
