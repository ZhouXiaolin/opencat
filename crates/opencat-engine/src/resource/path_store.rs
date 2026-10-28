//! Asset → physical path table.
//!
//! Maps `AssetId` to a file-system path. The engine owns this so its media
//! decode / audio paths can resolve cached bytes without going through core.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use opencat_core::ir::asset_id::AssetId;

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
            .ok_or_else(|| anyhow!("asset {} has no registered physical path", id.key))
    }

    pub fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        if self.entries.contains_key(&alias) {
            return Ok(());
        }
        let path = self
            .entries
            .get(target)
            .ok_or_else(|| anyhow!("cannot alias missing asset path {}", target.key))?
            .clone();
        self.entries.insert(alias, path);
        Ok(())
    }
}


