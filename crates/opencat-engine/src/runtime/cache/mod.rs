pub mod lru;
pub mod video_frames;

use skia_safe::{Image as SkiaImage, Picture};

// Re-export core's generic cache primitives.
pub use opencat_core::runtime::cache::{
    CacheCaps, CachedSubtreeImage, CachedSubtreeSnapshot, SharedLruCache,
};

// Existing aliases for compatibility
pub(crate) type ImageCache = SharedLruCache<String, Option<SkiaImage>>;
pub(crate) type SubtreeSnapshotCache = SharedLruCache<u64, CachedSubtreeSnapshot<Picture>>;
pub(crate) type SubtreeImageCache = SharedLruCache<u64, CachedSubtreeImage<SkiaImage>>;
pub(crate) type ItemPictureCache = SharedLruCache<u64, Picture>;
pub(crate) type GlyphPathCache = SharedLruCache<u64, skia_safe::Path>;
pub(crate) type GlyphImageCache = SharedLruCache<u64, SkiaImage>;

#[cfg(test)]
mod tests {
    use super::CacheCaps;

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
}
