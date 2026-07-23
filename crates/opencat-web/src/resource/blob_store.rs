//! In-memory blob storage for wasm: holds fetched asset bytes keyed by
//! [`AssetId`]. JS pulls bytes back out via `get_blob_bytes` to feed
//! CanvasKit / VideoDecoder / etc.

use std::collections::HashMap;
use std::sync::Arc;

use opencat_core::ir::asset_id::AssetId;

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

    pub fn is_empty(&self) -> bool {
        self.blobs.is_empty()
    }

    /// Iterate over `(asset_id, bytes)` pairs. Used by the host-owned open
    /// flow to supply bytes to the next pipeline's BlobStore.
    pub fn iter(&self) -> impl Iterator<Item = (&AssetId, &Arc<[u8]>)> {
        self.blobs.iter()
    }

    /// Snapshot every `(canonical asset id, bytes)` pair into an owned map
    /// keyed by `AssetId` string. Host uses this to pass bytes to the next
    /// pipeline stage.
    pub fn to_byte_map(&self) -> std::collections::HashMap<String, Vec<u8>> {
        self.blobs
            .iter()
            .map(|(id, bytes)| (id.key.clone(), bytes.to_vec()))
            .collect()
    }
}
