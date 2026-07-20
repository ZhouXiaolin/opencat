use super::AssetId;

/// Abstraction for reading raw asset bytes by [`AssetId`].
///
/// Web uses an in-memory store (`opencat_web::resource::BlobStore`) that
/// implements this trait; it is the byte source behind the shared
/// [`crate::resource::MapResourceProvider`] hydration path. The engine reads
/// cached bytes through its own `EngineLoader` handles and does not use this
/// trait.
pub trait BlobStore {
    fn read(&self, id: &AssetId) -> Option<Vec<u8>>;
}
