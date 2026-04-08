use crate::backend::skia::cache::{
    SkiaImageCache, SkiaSubtreePictureCache, SkiaTextPictureCache, new_image_cache,
    new_subtree_picture_cache, new_text_picture_cache,
};

pub(crate) struct SkiaBackendResources {
    image_cache: SkiaImageCache,
    text_picture_cache: SkiaTextPictureCache,
    subtree_picture_cache: SkiaSubtreePictureCache,
}

impl SkiaBackendResources {
    pub(crate) fn new() -> Self {
        Self {
            image_cache: new_image_cache(),
            text_picture_cache: new_text_picture_cache(),
            subtree_picture_cache: new_subtree_picture_cache(),
        }
    }

    pub(crate) fn image_cache(&self) -> SkiaImageCache {
        self.image_cache.clone()
    }

    pub(crate) fn text_picture_cache(&self) -> SkiaTextPictureCache {
        self.text_picture_cache.clone()
    }

    pub(crate) fn subtree_picture_cache(&self) -> SkiaSubtreePictureCache {
        self.subtree_picture_cache.clone()
    }
}

impl Default for SkiaBackendResources {
    fn default() -> Self {
        Self::new()
    }
}
