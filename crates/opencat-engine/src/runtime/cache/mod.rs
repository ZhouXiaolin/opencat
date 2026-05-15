pub mod lru;
pub mod video_frames;

use skia_safe::{Image as SkiaImage, Picture};

pub use opencat_core::render::cache::{
    CachedSubtreeImage, CachedSubtreeSnapshot, SharedLruCache,
};

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

pub(crate) type ImageCache = SharedLruCache<String, Option<SkiaImage>>;
pub(crate) type SubtreeSnapshotCache = SharedLruCache<u64, CachedSubtreeSnapshot<Picture>>;
pub(crate) type SubtreeImageCache = SharedLruCache<u64, CachedSubtreeImage<SkiaImage>>;
pub(crate) type ItemPictureCache = SharedLruCache<u64, Picture>;
pub(crate) type GlyphPathCache = SharedLruCache<u64, skia_safe::Path>;
pub(crate) type GlyphImageCache = SharedLruCache<u64, SkiaImage>;
