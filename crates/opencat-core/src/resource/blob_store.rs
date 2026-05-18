use std::fs;

use super::{AssetId, AssetPathStore};

/// Abstraction for reading raw asset bytes by [`AssetId`].
///
/// Engine uses filesystem-backed stores; web uses in-memory stores.
pub trait BlobStore {
    fn read(&self, id: &AssetId) -> Option<Vec<u8>>;
}

/// [`BlobStore`] backed by an [`AssetPathStore`] on the local filesystem.
pub struct AssetPathBlobStore<'a> {
    paths: &'a AssetPathStore,
}

impl<'a> AssetPathBlobStore<'a> {
    pub fn new(paths: &'a AssetPathStore) -> Self {
        Self { paths }
    }
}

impl BlobStore for AssetPathBlobStore<'_> {
    fn read(&self, id: &AssetId) -> Option<Vec<u8>> {
        let path = self.paths.path(id)?;
        fs::read(path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store_with_file(content: &[u8]) -> (AssetPathStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("asset.bin");
        fs::write(&file_path, content).expect("write temp file");

        let mut paths = AssetPathStore::new();
        paths.insert(AssetId("test-asset".into()), &file_path);
        (paths, dir)
    }

    #[test]
    fn read_returns_bytes_when_path_exists() {
        let (paths, _dir) = make_store_with_file(b"hello world");
        let store = AssetPathBlobStore::new(&paths);
        let bytes = store.read(&AssetId("test-asset".into()));
        assert_eq!(bytes, Some(b"hello world".to_vec()));
    }

    #[test]
    fn read_returns_none_when_id_not_registered() {
        let paths = AssetPathStore::new();
        let store = AssetPathBlobStore::new(&paths);
        let bytes = store.read(&AssetId("missing".into()));
        assert!(bytes.is_none());
    }

    #[test]
    fn read_returns_none_when_path_unreadable() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file_path = dir.path().join("gone.bin");
        // Don't write anything — the file doesn't exist on disk.

        let mut paths = AssetPathStore::new();
        paths.insert(AssetId("ghost".into()), &file_path);

        let store = AssetPathBlobStore::new(&paths);
        let bytes = store.read(&AssetId("ghost".into()));
        assert!(bytes.is_none());
    }
}
