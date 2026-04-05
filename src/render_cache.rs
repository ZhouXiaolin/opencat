use skia_safe::Picture;

use crate::backend::cache::{
    ImageCache, SubtreePictureCache, TextPictureCache, new_image_cache, new_subtree_picture_cache,
    new_text_picture_cache,
};

#[derive(Clone, Copy)]
pub(crate) enum SceneSlot {
    Scene,
    TransitionFrom,
    TransitionTo,
}

#[derive(Default)]
struct PictureSlotCache {
    picture: Option<Picture>,
}

pub(crate) struct RenderCacheState {
    image_cache: ImageCache,
    text_picture_cache: TextPictureCache,
    subtree_picture_cache: SubtreePictureCache,
    scene_picture_cache: PictureSlotCache,
    transition_from_picture_cache: PictureSlotCache,
    transition_to_picture_cache: PictureSlotCache,
}

impl RenderCacheState {
    pub(crate) fn new() -> Self {
        Self {
            image_cache: new_image_cache(),
            text_picture_cache: new_text_picture_cache(),
            subtree_picture_cache: new_subtree_picture_cache(),
            scene_picture_cache: PictureSlotCache::default(),
            transition_from_picture_cache: PictureSlotCache::default(),
            transition_to_picture_cache: PictureSlotCache::default(),
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

    pub(crate) fn picture(&self, slot: SceneSlot) -> Option<Picture> {
        match slot {
            SceneSlot::Scene => self.scene_picture_cache.picture.clone(),
            SceneSlot::TransitionFrom => self.transition_from_picture_cache.picture.clone(),
            SceneSlot::TransitionTo => self.transition_to_picture_cache.picture.clone(),
        }
    }

    pub(crate) fn store_picture(&mut self, slot: SceneSlot, picture: Option<Picture>) {
        match slot {
            SceneSlot::Scene => self.scene_picture_cache.picture = picture,
            SceneSlot::TransitionFrom => self.transition_from_picture_cache.picture = picture,
            SceneSlot::TransitionTo => self.transition_to_picture_cache.picture = picture,
        }
    }
}

impl Default for RenderCacheState {
    fn default() -> Self {
        Self::new()
    }
}
