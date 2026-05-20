//! In-memory blob storage for wasm: holds fetched asset bytes keyed by
//! [`AssetId`]. JS pulls bytes back out via `get_blob_bytes` to feed
//! CanvasKit / VideoDecoder / etc.

use std::collections::HashMap;
use std::sync::Arc;

use opencat_core::resource::asset_id::AssetId;

#[derive(Default)]
pub struct BlobStore {
    blobs: HashMap<AssetId, Arc<[u8]>>,
}

impl BlobStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: AssetId, bytes: Arc<[u8]>) {
        self.blobs.insert(id, bytes);
    }

    pub fn get(&self, id: &AssetId) -> Option<Arc<[u8]>> {
        self.blobs.get(id).cloned()
    }

    pub fn clear(&mut self) {
        self.blobs.clear();
    }

    pub fn len(&self) -> usize {
        self.blobs.len()
    }
}

impl opencat_core::resource::BlobStore for BlobStore {
    fn read(&self, id: &AssetId) -> Option<Vec<u8>> {
        self.blobs.get(id).map(|arc| arc.to_vec())
    }
}
