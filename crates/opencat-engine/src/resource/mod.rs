pub mod fetch;
pub mod media;
pub mod resolver;
pub mod utils;

use opencat_core::platform::resource::ResourcePlatform;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::blob_store::BlobStore;
use opencat_core::resource::resolver::{AssetResolver, AssetSink, UrlFetcher};
use std::future::Future;

pub use opencat_core::resource::AssetPathStore;
pub use resolver::EngineAssetResolver;
pub use utils::{asset_id_for_audio_path, cache_file_path};

pub struct EngineResourcePlatform;

impl ResourcePlatform for EngineResourcePlatform {
    type Resolver = StubAssetResolver;
    type BlobStore = StubBlobStore;

    fn resolver(&mut self) -> &mut Self::Resolver {
        unimplemented!("resource resolver not wired")
    }
    fn blob_store(&self) -> Option<&Self::BlobStore> {
        None
    }
}

pub struct StubAssetResolver;

impl AssetResolver for StubAssetResolver {
    type Fetcher = StubFetcher;
    type Sink = StubSink;

    fn parts(&mut self) -> (&mut Self::Fetcher, &mut Self::Sink) {
        unimplemented!("stub resolver parts")
    }
}

pub struct StubFetcher;
impl UrlFetcher for StubFetcher {
    fn fetch_bytes(
        &mut self,
        _id: &AssetId,
        _url: &str,
    ) -> impl Future<Output = anyhow::Result<Vec<u8>>> {
        async { unimplemented!("StubFetcher::fetch_bytes") }
    }
}

pub struct StubSink;
impl AssetSink for StubSink {
    fn store(&mut self, _id: &AssetId, _bytes: Vec<u8>) {}
}

pub struct StubBlobStore;
impl BlobStore for StubBlobStore {
    fn read(&self, _id: &AssetId) -> Option<Vec<u8>> {
        None
    }
}
