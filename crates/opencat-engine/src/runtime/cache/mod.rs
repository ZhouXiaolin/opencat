pub mod lru;
pub mod video_frames;

use std::{cell::RefCell, rc::Rc};

use skia_safe::{Image as SkiaImage, Picture};

use opencat_core::display::list::DisplayRect;
use crate::runtime::cache::lru::BoundedLruCache;

pub(crate) type SharedLruCache<K, V> = Rc<RefCell<BoundedLruCache<K, V>>>;
pub(crate) type ImageCache = SharedLruCache<String, Option<SkiaImage>>;
pub(crate) type SubtreeSnapshotCache = SharedLruCache<u64, CachedSubtreeSnapshot>;
pub(crate) type SubtreeImageCache = SharedLruCache<u64, CachedSubtreeImage>;
pub(crate) type ItemPictureCache = SharedLruCache<u64, Picture>;
pub(crate) type GlyphPathCache = SharedLruCache<u64, skia_safe::Path>;
pub(crate) type GlyphImageCache = SharedLruCache<u64, SkiaImage>;

/// `SubtreeSnapshotCache` 的 value。命中时必须用 `secondary_fingerprint` 与查询端的
/// 次级 hash 做二次比对，任一不等视为 64-bit hash 碰撞，走 miss 重录。
#[derive(Clone)]
pub(crate) struct CachedSubtreeSnapshot {
    pub picture: Picture,
    pub secondary_fingerprint: u64,
    pub consecutive_hits: usize,
    pub recorded_bounds: DisplayRect,
}

/// `SubtreeImageCache` 的 value。已光栅化的 subtree 图像，可直接 draw_image。
#[derive(Clone)]
pub(crate) struct CachedSubtreeImage {
    pub image: SkiaImage,
    pub recorded_bounds: DisplayRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CacheCaps {
    pub images: usize,
    pub subtree_snapshots: usize,
    pub subtree_images: usize,
    pub item_pictures: usize,
    pub video_frames: usize,
    pub glyph_paths: usize,
    pub glyph_images: usize,
}

impl Default for CacheCaps {
    fn default() -> Self {
        Self {
            images: 128,
            subtree_snapshots: 256,
            subtree_images: 128,
            item_pictures: 256,
            video_frames: 64,
            glyph_paths: 4096,
            glyph_images: 1024,
        }
    }
}

pub(crate) struct CacheRegistry {
    image_cache: ImageCache,
    subtree_snapshot_cache: SubtreeSnapshotCache,
    subtree_image_cache: SubtreeImageCache,
    item_picture_cache: ItemPictureCache,
    glyph_path_cache: GlyphPathCache,
    glyph_image_cache: GlyphImageCache,
}

impl CacheRegistry {
    pub(crate) fn new(caps: CacheCaps) -> Self {
        Self {
            image_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.images))),
            subtree_snapshot_cache: Rc::new(RefCell::new(BoundedLruCache::new(
                caps.subtree_snapshots,
            ))),
            subtree_image_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.subtree_images))),
            item_picture_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.item_pictures))),
            glyph_path_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.glyph_paths))),
            glyph_image_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.glyph_images))),
        }
    }

    pub(crate) fn image_cache(&self) -> ImageCache {
        self.image_cache.clone()
    }

    pub(crate) fn subtree_snapshot_cache(&self) -> SubtreeSnapshotCache {
        self.subtree_snapshot_cache.clone()
    }

    pub(crate) fn subtree_image_cache(&self) -> SubtreeImageCache {
        self.subtree_image_cache.clone()
    }

    pub(crate) fn item_picture_cache(&self) -> ItemPictureCache {
        self.item_picture_cache.clone()
    }

    pub(crate) fn glyph_path_cache(&self) -> GlyphPathCache {
        self.glyph_path_cache.clone()
    }

    pub(crate) fn glyph_image_cache(&self) -> GlyphImageCache {
        self.glyph_image_cache.clone()
    }
}

impl Default for CacheRegistry {
    fn default() -> Self {
        Self::new(CacheCaps::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{CacheCaps, CacheRegistry};

    #[test]
    fn default_cache_caps_reserve_subtree_images() {
        let caps = CacheCaps::default();
        assert_eq!(caps.subtree_images, 128);
    }

    #[test]
    fn default_cache_caps_reserve_glyph_paths() {
        let caps = CacheCaps::default();
        assert_eq!(caps.glyph_paths, 4096);
    }

    #[test]
    fn cache_registry_exposes_glyph_path_cache() {
        let registry = CacheRegistry::default();
        let cache = registry.glyph_path_cache();
        assert_eq!(
            cache.borrow().capacity(),
            CacheCaps::default().glyph_paths
        );
    }
}
