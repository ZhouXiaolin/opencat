//! Backend-typed scene snapshot cache (single-slot, cross-frame scene picture reuse).

use crate::platform::backend::BackendTypes;

pub struct SceneSnapshotCache<B: BackendTypes> {
    snapshot: Option<B::Picture>,
}

impl<B: BackendTypes> SceneSnapshotCache<B> {
    pub fn new() -> Self {
        Self { snapshot: None }
    }

    pub fn scene_snapshot(&self) -> Option<B::Picture> {
        self.snapshot.clone()
    }

    pub fn store_scene_snapshot(&mut self, snapshot: Option<B::Picture>) {
        self.snapshot = snapshot;
    }
}

impl<B: BackendTypes> Default for SceneSnapshotCache<B> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend;
    impl BackendTypes for MockBackend {
        type Picture = String;
        type Image = String;
        type GlyphPath = String;
        type GlyphImage = String;
    }

    #[test]
    fn store_and_retrieve_snapshot() {
        let mut cache: SceneSnapshotCache<MockBackend> = SceneSnapshotCache::new();
        assert_eq!(cache.scene_snapshot(), None);

        cache.store_scene_snapshot(Some("scene-frame-42".to_string()));
        assert_eq!(cache.scene_snapshot(), Some("scene-frame-42".to_string()));

        cache.store_scene_snapshot(None);
        assert_eq!(cache.scene_snapshot(), None);
    }
}
