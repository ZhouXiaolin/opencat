pub mod video_frames;

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
