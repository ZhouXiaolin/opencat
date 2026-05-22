//! Platform role for resource management (asset resolution and blob storage).

use crate::resource::{AssetResolver, BlobStore};

/// Platform role for resource management (asset resolution and blob storage).
pub trait ResourcePlatform {
    type Resolver: AssetResolver;
    type BlobStore: BlobStore;

    /// Get the asset resolver for this platform.
    fn resolver(&mut self) -> &mut Self::Resolver;
    /// Get the blob store, if available.
    fn blob_store(&self) -> Option<&Self::BlobStore>;
}
