//! Render cache — generic LRU buckets parameterised by a `Canvas2D` backend.

use std::cell::RefCell;
use std::rc::Rc;

use crate::cache::lru::BoundedLruCache;
use crate::canvas::Canvas2D;
use crate::display::list::DisplayRect;

/// Shared (Rc<RefCell<…>>) handle to a single LRU bucket.
pub type SharedLruCache<K, V> = Rc<RefCell<BoundedLruCache<K, V>>>;

/// Cached picture for a display-tree subtree.
#[derive(Clone)]
pub struct CachedSubtreeSnapshot<P> {
    pub picture: P,
    pub secondary_fingerprint: u64,
    pub consecutive_hits: usize,
    pub recorded_bounds: DisplayRect,
}

/// Cached raster image for a display-tree subtree.
#[derive(Clone)]
pub struct CachedSubtreeImage<I> {
    pub image: I,
    pub recorded_bounds: DisplayRect,
}

/// Collection of LRU caches keyed by the Canvas2D associated types.
///
/// Each bucket is a `SharedLruCache` so it can be cloned into helper
/// functions without borrowing the whole struct.
pub struct RenderCache<C: Canvas2D> {
    pub images: SharedLruCache<String, Option<C::Image>>,
    pub subtree_snapshots: SharedLruCache<u64, CachedSubtreeSnapshot<C::Picture>>,
    pub subtree_images: SharedLruCache<u64, CachedSubtreeImage<C::Image>>,
    pub item_pictures: SharedLruCache<u64, C::Picture>,
    pub glyph_paths: SharedLruCache<u64, C::Path>,
    pub glyph_images: SharedLruCache<u64, C::Image>,
    /// Runtime-effect shader cache (keyed by SkSL hash).
    pub runtime_effects: SharedLruCache<u64, C::RuntimeEffect>,
    /// Most-recent scene-level picture snapshot (fingerprint, picture).
    pub scene_snapshot: Option<(u64, C::Picture)>,
}

impl<C: Canvas2D> RenderCache<C> {
    pub fn new(
        image_cap: usize,
        subtree_snapshot_cap: usize,
        subtree_image_cap: usize,
        item_picture_cap: usize,
        glyph_path_cap: usize,
        glyph_image_cap: usize,
        runtime_effect_cap: usize,
    ) -> Self {
        RenderCache {
            images: Rc::new(RefCell::new(BoundedLruCache::new(image_cap))),
            subtree_snapshots: Rc::new(RefCell::new(BoundedLruCache::new(
                subtree_snapshot_cap,
            ))),
            subtree_images: Rc::new(RefCell::new(BoundedLruCache::new(subtree_image_cap))),
            item_pictures: Rc::new(RefCell::new(BoundedLruCache::new(item_picture_cap))),
            glyph_paths: Rc::new(RefCell::new(BoundedLruCache::new(glyph_path_cap))),
            glyph_images: Rc::new(RefCell::new(BoundedLruCache::new(glyph_image_cap))),
            runtime_effects: Rc::new(RefCell::new(BoundedLruCache::new(runtime_effect_cap))),
            scene_snapshot: None,
        }
    }
}
