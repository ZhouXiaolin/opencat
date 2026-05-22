//! WebPlatform -- wasm/web 端的 Platform 实现。

use std::future::Future;

use opencat_core::draw::cache::CachedDrawRange;
use opencat_core::draw::frame::DrawOpFrame;
use opencat_core::platform::draw::{DrawError, DrawPlatform, DrawStats, RenderSessionHeader};
use opencat_core::platform::media::{
    AudioPlanSlice, PrepareMode, FrameMediaPlan, MediaError, MediaPlatform,
};
use opencat_core::platform::platform::Platform;
use opencat_core::platform::resource::ResourcePlatform;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::{AssetResolver, AssetSink, BlobStore, UrlFetcher};

use crate::script::ScriptRuntimeCache;
use crate::video::WebVideoSource;

pub struct WebPlatform {
    pub script: ScriptRuntimeCache,
    pub video: WebVideoSource,
}

impl WebPlatform {
    pub fn new() -> Self {
        Self {
            script: ScriptRuntimeCache::default(),
            video: WebVideoSource::default(),
        }
    }
}

impl Default for WebPlatform {
    fn default() -> Self {
        Self::new()
    }
}

// ── Stub types — will be replaced with real implementations later ───

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

pub struct StubAssetResolver {
    fetcher: StubFetcher,
    sink: StubSink,
}
impl AssetResolver for StubAssetResolver {
    type Fetcher = StubFetcher;
    type Sink = StubSink;
    fn parts(&mut self) -> (&mut Self::Fetcher, &mut Self::Sink) {
        (&mut self.fetcher, &mut self.sink)
    }
}

pub struct StubBlobStore;
impl BlobStore for StubBlobStore {
    fn read(&self, _id: &AssetId) -> Option<Vec<u8>> {
        None
    }
}

pub struct StubResourcePlatform {
    resolver: StubAssetResolver,
    blob_store: StubBlobStore,
}
impl ResourcePlatform for StubResourcePlatform {
    type Resolver = StubAssetResolver;
    type BlobStore = StubBlobStore;
    fn resolver(&mut self) -> &mut Self::Resolver {
        &mut self.resolver
    }
    fn blob_store(&self) -> Option<&Self::BlobStore> {
        Some(&self.blob_store)
    }
}

pub struct StubMediaPlatform;
impl MediaPlatform for StubMediaPlatform {
    type PreparedFrameMedia = ();
    type PreparedAudioSlice = ();
    fn prepare_frame(
        &mut self,
        _plan: &FrameMediaPlan,
        _mode: PrepareMode,
    ) -> Result<Self::PreparedFrameMedia, MediaError> {
        Err(MediaError("stub".into()))
    }
    fn prepare_audio_slice(
        &mut self,
        _slice: &AudioPlanSlice,
        _mode: PrepareMode,
    ) -> Result<Self::PreparedAudioSlice, MediaError> {
        Err(MediaError("stub".into()))
    }
}

pub struct StubDrawPlatform;
impl DrawPlatform for StubDrawPlatform {
    type Target = ();
    type PreparedFrameMedia = ();
    fn execute(
        &mut self,
        _header: &RenderSessionHeader,
        _draw: &DrawOpFrame,
        _media: &Self::PreparedFrameMedia,
        _target: &mut Self::Target,
    ) -> Result<DrawStats, DrawError> {
        Err(DrawError("stub".into()))
    }
    fn compile_range(
        &mut self,
        _cached: &CachedDrawRange,
        _draw: &DrawOpFrame,
    ) -> Result<(), DrawError> {
        Err(DrawError("stub".into()))
    }
    fn evict_range(&mut self, _fingerprint: u64) {}
}

impl Platform for WebPlatform {
    type Script = ScriptRuntimeCache;
    type Resource = StubResourcePlatform;
    type Media = StubMediaPlatform;
    type Draw = StubDrawPlatform;
    type Video = WebVideoSource;

    fn script(&mut self) -> &mut Self::Script {
        &mut self.script
    }

    fn resources(&mut self) -> &mut Self::Resource {
        unimplemented!("WebPlatform::resources not yet implemented")
    }

    fn media(&mut self) -> &mut Self::Media {
        unimplemented!("WebPlatform::media not yet implemented")
    }

    fn draw(&mut self) -> &mut Self::Draw {
        unimplemented!("WebPlatform::draw not yet implemented")
    }

    fn video_source(&mut self) -> &mut Self::Video {
        &mut self.video
    }
}
