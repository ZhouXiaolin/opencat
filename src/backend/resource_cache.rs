use crate::backend::cache::{
    ImageCache, SubtreePictureCache, TextPictureCache, new_image_cache, new_subtree_picture_cache,
    new_text_picture_cache,
};

pub(crate) struct BackendResourceCache {
    image_cache: ImageCache,
    text_picture_cache: TextPictureCache,
    subtree_picture_cache: SubtreePictureCache,
}

impl BackendResourceCache {
    pub(crate) fn new() -> Self {
        Self {
            image_cache: new_image_cache(),
            text_picture_cache: new_text_picture_cache(),
            subtree_picture_cache: new_subtree_picture_cache(),
        }
    }

    pub(crate) fn image_cache(&self) -> ImageCache {
        self.image_cache.clone()
    }

    pub(crate) fn text_picture_cache(&self) -> TextPictureCache {
        self.text_picture_cache.clone()
    }

    pub(crate) fn subtree_picture_cache(&self) -> SubtreePictureCache {
        self.subtree_picture_cache.clone()
    }
}

impl Default for BackendResourceCache {
    fn default() -> Self {
        Self::new()
    }
}
