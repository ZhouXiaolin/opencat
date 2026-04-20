use std::collections::HashMap;

use crate::runtime::render_engine::SceneSnapshot;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SceneSlot {
    path: Vec<u8>,
    role: SceneSlotRole,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SceneSlotRole {
    Scene,
    TransitionFrom,
    TransitionTo,
}

impl SceneSlot {
    pub(crate) fn root_scene() -> Self {
        Self {
            path: Vec::new(),
            role: SceneSlotRole::Scene,
        }
    }

    pub(crate) fn root_transition_from() -> Self {
        Self {
            path: Vec::new(),
            role: SceneSlotRole::TransitionFrom,
        }
    }

    pub(crate) fn root_transition_to() -> Self {
        Self {
            path: Vec::new(),
            role: SceneSlotRole::TransitionTo,
        }
    }

    pub(crate) fn child_scene(child_index: usize) -> Self {
        Self {
            path: vec![child_index as u8],
            role: SceneSlotRole::Scene,
        }
    }

    pub(crate) fn child_transition_from(child_index: usize) -> Self {
        Self {
            path: vec![child_index as u8],
            role: SceneSlotRole::TransitionFrom,
        }
    }

    pub(crate) fn child_transition_to(child_index: usize) -> Self {
        Self {
            path: vec![child_index as u8],
            role: SceneSlotRole::TransitionTo,
        }
    }
}

struct SceneSnapshotSlotCache {
    source: Option<SceneSnapshot>,
}

impl Default for SceneSnapshotSlotCache {
    fn default() -> Self {
        Self { source: None }
    }
}

pub(crate) struct SceneSnapshotCache {
    slots: HashMap<SceneSlot, SceneSnapshotSlotCache>,
}

impl SceneSnapshotCache {
    pub(crate) fn new() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }

    pub(crate) fn scene_snapshot(&self, slot: &SceneSlot) -> Option<SceneSnapshot> {
        self.slots.get(slot).and_then(|s| s.source.clone())
    }

    pub(crate) fn store_scene_snapshot(&mut self, slot: SceneSlot, source: Option<SceneSnapshot>) {
        self.slots
            .entry(slot)
            .and_modify(|e| e.source = source.clone())
            .or_insert_with(|| SceneSnapshotSlotCache { source });
    }
}

impl Default for SceneSnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}
