use crate::runtime::render_engine::SceneSnapshot;

#[derive(Clone, Copy)]
pub(crate) enum SceneSlot {
    Scene,
    TransitionFrom,
    TransitionTo,
}

#[derive(Default)]
struct SceneSnapshotSlotCache {
    source: Option<SceneSnapshot>,
}

pub(crate) struct SceneSnapshotCache {
    scene_snapshot_slot: SceneSnapshotSlotCache,
    transition_from_snapshot_slot: SceneSnapshotSlotCache,
    transition_to_snapshot_slot: SceneSnapshotSlotCache,
}

impl SceneSnapshotCache {
    pub(crate) fn new() -> Self {
        Self {
            scene_snapshot_slot: SceneSnapshotSlotCache::default(),
            transition_from_snapshot_slot: SceneSnapshotSlotCache::default(),
            transition_to_snapshot_slot: SceneSnapshotSlotCache::default(),
        }
    }

    pub(crate) fn scene_snapshot(&self, slot: SceneSlot) -> Option<SceneSnapshot> {
        match slot {
            SceneSlot::Scene => self.scene_snapshot_slot.source.clone(),
            SceneSlot::TransitionFrom => self.transition_from_snapshot_slot.source.clone(),
            SceneSlot::TransitionTo => self.transition_to_snapshot_slot.source.clone(),
        }
    }

    pub(crate) fn store_scene_snapshot(&mut self, slot: SceneSlot, source: Option<SceneSnapshot>) {
        match slot {
            SceneSlot::Scene => self.scene_snapshot_slot.source = source,
            SceneSlot::TransitionFrom => self.transition_from_snapshot_slot.source = source,
            SceneSlot::TransitionTo => self.transition_to_snapshot_slot.source = source,
        }
    }
}

impl Default for SceneSnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}
