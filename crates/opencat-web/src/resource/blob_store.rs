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

    pub fn is_empty(&self) -> bool {
        self.blobs.is_empty()
    }

    /// Iterate over `(asset_id, bytes)` pairs. Used by the host-owned open
    /// flow to build the `ByteSource` map fed to core's pure `build_catalog`.
    pub fn iter(&self) -> impl Iterator<Item = (&AssetId, &Arc<[u8]>)> {
        self.blobs.iter()
    }

    /// Snapshot every `(canonical asset id, bytes)` pair into an owned map
    /// keyed by `AssetId` string. This is the host-side bridge to core's pure
    /// `probe::prepare::build_catalog` (which keys on canonical id strings),
    /// mirroring the engine's `collect_probe_bytes_by_asset_id`.
    pub fn to_byte_map(&self) -> std::collections::HashMap<String, Vec<u8>> {
        self.blobs
            .iter()
            .map(|(id, bytes)| (id.0.clone(), bytes.to_vec()))
            .collect()
    }
}

impl opencat_core::resource::BlobStore for BlobStore {
    fn read(&self, id: &AssetId) -> Option<Vec<u8>> {
        self.blobs.get(id).map(|arc| arc.to_vec())
    }
}
