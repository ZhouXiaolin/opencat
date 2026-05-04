use crate::host::runtime::render_engine::SceneSnapshot;

pub struct SceneSnapshotCache {
    source: Option<SceneSnapshot>,
}

impl SceneSnapshotCache {
    pub(crate) fn new() -> Self {
        Self { source: None }
    }

    pub(crate) fn scene_snapshot(&self) -> Option<SceneSnapshot> {
        self.source.clone()
    }

    pub(crate) fn store_scene_snapshot(&mut self, source: Option<SceneSnapshot>) {
        self.source = source;
    }
}

impl Default for SceneSnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}
