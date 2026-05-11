//! Engine-side Platform impl: aggregates quickjs script runtime, ffmpeg/skia
//! media context, Skia render engine, asset path table, audio caches.

pub mod audio_runtime;

use std::sync::Arc;

use std::any::Any;

use opencat_core::Platform;
use opencat_core::scene::path_bounds::PathBoundsComputer;

use crate::backend::skia::renderer::{SkiaRenderData, SkiaRenderEngine};
use crate::resource::asset_catalog::AssetCatalog;
use crate::resource::media::MediaContext;
use crate::resource::path_store::AssetPathStore;
use crate::runtime::audio::{AudioIntervalCache, DecodedAudioCache};
use crate::runtime::cache::CacheCaps;
use crate::runtime::path_bounds::SkiaPathBounds;
use crate::script::ScriptRuntimeCache;

pub use audio_runtime::AudioRuntime;

/// Engine platform implementation.
pub struct EnginePlatform {
    pub backend: Arc<SkiaRenderEngine>,
    pub script: ScriptRuntimeCache,
    pub video: MediaContext,
    pub asset_paths: AssetPathStore,
    pub audio: AudioRuntime,
    pub path_bounds: SkiaPathBounds,
    /// Engine-level asset catalog (file paths, dimensions, remote downloads).
    pub assets: AssetCatalog,
    /// Decoded audio cache for streaming playback.
    pub audio_decode_cache: DecodedAudioCache,
    /// Audio interval cache for composition-level audio scheduling.
    pub audio_interval_cache: AudioIntervalCache,
    /// Last preflight root pointer to skip duplicate preflight runs.
    pub prepared_root_ptr: Option<usize>,
}

impl EnginePlatform {
    pub fn new(backend: Arc<SkiaRenderEngine>) -> Self {
        Self::with_cache_caps(backend, CacheCaps::default())
    }

    pub fn with_cache_caps(backend: Arc<SkiaRenderEngine>, _caps: CacheCaps) -> Self {
        Self {
            backend,
            script: ScriptRuntimeCache::default(),
            video: MediaContext::new(),
            asset_paths: AssetPathStore::new(),
            audio: AudioRuntime::new(),
            path_bounds: SkiaPathBounds,
            assets: AssetCatalog::new(),
            audio_decode_cache: DecodedAudioCache::default(),
            audio_interval_cache: AudioIntervalCache::default(),
            prepared_root_ptr: None,
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: crate::resource::media::VideoPreviewQuality) {
        self.video.set_video_preview_quality(quality);
    }
}

impl Platform for EnginePlatform {
    type Backend = SkiaRenderEngine;
    type Script = ScriptRuntimeCache;
    type Video = MediaContext;

    fn render_engine(&self) -> &Self::Backend {
        &*self.backend
    }
    fn script_host(&mut self) -> &mut Self::Script {
        &mut self.script
    }
    fn video_source(&mut self) -> &mut Self::Video {
        &mut self.video
    }
    fn path_bounds(&self) -> &dyn PathBoundsComputer {
        &self.path_bounds
    }

    /// Provide `SkiaRenderData` (assets + media_ctx raw pointer) to the backend through platform_data.
    fn with_render_context<R>(
        &mut self,
        f: impl FnOnce(&mut Self::Video, &Self::Backend, &mut dyn Any) -> R,
    ) -> R {
        let this = self as *mut Self;
        let video = unsafe { &mut *this }.video_source();
        let backend = unsafe { &*this }.render_engine();
        let assets = unsafe { &(*this).assets };
        let media_ctx_ptr = video as *mut MediaContext;
        let mut render_data = SkiaRenderData { assets, media_ctx: media_ctx_ptr };
        f(video, backend, &mut render_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencat_core::Platform;

    #[test]
    fn engine_platform_constructs() {
        let backend = crate::backend::skia::renderer::shared_raster_engine_typed();
        let mut platform = EnginePlatform::new(backend);
        let _engine: &SkiaRenderEngine = platform.render_engine();
        let _script = platform.script_host();
        let _video = platform.video_source();
        let _bounds = platform.path_bounds();
    }
}
