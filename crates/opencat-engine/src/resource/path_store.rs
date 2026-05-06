//! Engine-side path table: physical paths that core catalog can't hold.
//!
//! When AssetCatalog is split into "core (HashMapResourceCatalog) metadata +
//! engine physical paths", this table holds the latter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use opencat_core::resource::asset_id::AssetId;

#[derive(Default, Debug)]
pub struct AssetPathStore {
    pub entries: HashMap<AssetId, PathBuf>,
}

impl AssetPathStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: AssetId, path: impl Into<PathBuf>) {
        self.entries.insert(id, path.into());
    }

    pub fn path(&self, id: &AssetId) -> Option<&Path> {
        self.entries.get(id).map(|p| p.as_path())
    }

    pub fn require_path(&self, id: &AssetId) -> Result<&Path> {
        self.path(id)
            .ok_or_else(|| anyhow!("asset {} has no registered physical path", id.0))
    }

    pub fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        if self.entries.contains_key(&alias) {
            return Ok(());
        }
        let path = self
            .entries
            .get(target)
            .ok_or_else(|| anyhow!("cannot alias missing asset path {}", target.0))?
            .clone();
        self.entries.insert(alias, path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn insert_then_path_returns_registered_path() {
        let mut store = AssetPathStore::new();
        store.insert(AssetId("img:a".into()), PathBuf::from("/tmp/a.png"));
        assert_eq!(
            store.path(&AssetId("img:a".into())),
            Some(Path::new("/tmp/a.png"))
        );
    }

    #[test]
    fn require_path_errors_for_missing_id() {
        let store = AssetPathStore::new();
        assert!(store.require_path(&AssetId("missing".into())).is_err());
    }

    #[test]
    fn alias_copies_path_from_target() {
        let mut store = AssetPathStore::new();
        store.insert(AssetId("orig".into()), "/tmp/o.mp4");
        store
            .alias(AssetId("aka".into()), &AssetId("orig".into()))
            .unwrap();
        assert_eq!(
            store
                .path(&AssetId("aka".into()))
                .map(Path::to_string_lossy)
                .as_deref(),
            Some("/tmp/o.mp4")
        );
    }
}
