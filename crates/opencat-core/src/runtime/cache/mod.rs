//! Backend-agnostic caches used by the render pipeline.

use std::cell::RefCell;
use std::rc::Rc;

use crate::cache::lru::BoundedLruCache;
use crate::display::list::DisplayRect;
use crate::platform::backend::BackendTypes;

pub mod video_frames;

pub type SharedLruCache<K, V> = Rc<RefCell<BoundedLruCache<K, V>>>;

#[derive(Clone)]
pub struct CachedSubtreeSnapshot<P> {
    pub picture: P,
    pub secondary_fingerprint: u64,
    pub consecutive_hits: usize,
    pub recorded_bounds: DisplayRect,
}

#[derive(Clone)]
pub struct CachedSubtreeImage<I> {
    pub image: I,
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

pub struct CacheRegistry<B: BackendTypes> {
    image_cache: SharedLruCache<String, Option<B::Image>>,
    subtree_snapshot_cache: SharedLruCache<u64, CachedSubtreeSnapshot<B::Picture>>,
    subtree_image_cache: SharedLruCache<u64, CachedSubtreeImage<B::Image>>,
    item_picture_cache: SharedLruCache<u64, B::Picture>,
    glyph_path_cache: SharedLruCache<u64, B::GlyphPath>,
    glyph_image_cache: SharedLruCache<u64, B::GlyphImage>,
}

impl<B: BackendTypes> CacheRegistry<B> {
    pub fn new(caps: CacheCaps) -> Self {
        Self {
            image_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.images))),
            subtree_snapshot_cache: Rc::new(RefCell::new(BoundedLruCache::new(
                caps.subtree_snapshots,
            ))),
            subtree_image_cache: Rc::new(RefCell::new(BoundedLruCache::new(
                caps.subtree_images,
            ))),
            item_picture_cache: Rc::new(RefCell::new(BoundedLruCache::new(
                caps.item_pictures,
            ))),
            glyph_path_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.glyph_paths))),
            glyph_image_cache: Rc::new(RefCell::new(BoundedLruCache::new(caps.glyph_images))),
        }
    }

    pub fn image_cache(&self) -> SharedLruCache<String, Option<B::Image>> {
        self.image_cache.clone()
    }

    pub fn subtree_snapshot_cache(&self) -> SharedLruCache<u64, CachedSubtreeSnapshot<B::Picture>> {
        self.subtree_snapshot_cache.clone()
    }

    pub fn subtree_image_cache(&self) -> SharedLruCache<u64, CachedSubtreeImage<B::Image>> {
        self.subtree_image_cache.clone()
    }

    pub fn item_picture_cache(&self) -> SharedLruCache<u64, B::Picture> {
        self.item_picture_cache.clone()
    }

    pub fn glyph_path_cache(&self) -> SharedLruCache<u64, B::GlyphPath> {
        self.glyph_path_cache.clone()
    }

    pub fn glyph_image_cache(&self) -> SharedLruCache<u64, B::GlyphImage> {
        self.glyph_image_cache.clone()
    }
}

impl<B: BackendTypes> Default for CacheRegistry<B> {
    fn default() -> Self {
        Self::new(CacheCaps::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend;
    impl BackendTypes for MockBackend {
        type Picture = Vec<u8>;
        type Image = String;
        type GlyphPath = Vec<u32>;
        type GlyphImage = String;
    }

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
        let registry = CacheRegistry::<MockBackend>::default();
        let cache = registry.glyph_path_cache();
        assert_eq!(
            cache.borrow().capacity(),
            CacheCaps::default().glyph_paths
        );
    }

    #[test]
    fn cache_registry_custom_caps_override_defaults() {
        let caps = CacheCaps {
            images: 1,
            subtree_snapshots: 2,
            subtree_images: 3,
            item_pictures: 4,
            video_frames: 5,
            glyph_paths: 6,
            glyph_images: 7,
        };
        let registry = CacheRegistry::<MockBackend>::new(caps);
        assert_eq!(registry.image_cache().borrow().capacity(), 1);
        assert_eq!(registry.subtree_snapshot_cache().borrow().capacity(), 2);
        assert_eq!(registry.subtree_image_cache().borrow().capacity(), 3);
        assert_eq!(registry.item_picture_cache().borrow().capacity(), 4);
        assert_eq!(registry.glyph_path_cache().borrow().capacity(), 6);
        assert_eq!(registry.glyph_image_cache().borrow().capacity(), 7);
    }
}
