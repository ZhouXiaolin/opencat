use crate::runtime::render_engine::SceneSnapshot;

#[derive(Clone, Copy)]
pub(crate) enum SceneSlot {
    Scene,
    TransitionFrom,
    TransitionTo,
}

#[derive(Default)]
struct PictureSlotCache {
    source: Option<SceneSnapshot>,
}

pub(crate) struct SceneSnapshotCache {
    scene_picture_cache: PictureSlotCache,
    transition_from_picture_cache: PictureSlotCache,
    transition_to_picture_cache: PictureSlotCache,
}

impl SceneSnapshotCache {
    pub(crate) fn new() -> Self {
        Self {
            scene_picture_cache: PictureSlotCache::default(),
            transition_from_picture_cache: PictureSlotCache::default(),
            transition_to_picture_cache: PictureSlotCache::default(),
        }
    }

    pub(crate) fn scene_snapshot(&self, slot: SceneSlot) -> Option<SceneSnapshot> {
        match slot {
            SceneSlot::Scene => self.scene_picture_cache.source.clone(),
            SceneSlot::TransitionFrom => self.transition_from_picture_cache.source.clone(),
            SceneSlot::TransitionTo => self.transition_to_picture_cache.source.clone(),
        }
    }

    pub(crate) fn store_scene_snapshot(&mut self, slot: SceneSlot, source: Option<SceneSnapshot>) {
        match slot {
            SceneSlot::Scene => self.scene_picture_cache.source = source,
            SceneSlot::TransitionFrom => self.transition_from_picture_cache.source = source,
            SceneSlot::TransitionTo => self.transition_to_picture_cache.source = source,
        }
    }
}

impl Default for SceneSnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}
