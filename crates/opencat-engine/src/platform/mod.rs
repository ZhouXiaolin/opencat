//! Engine-side Platform impl: aggregates quickjs script runtime, ffmpeg/skia
//! media context, Skia render engine, asset path table, audio caches.

pub mod audio_runtime;

use std::sync::Arc;

use opencat_core::Platform;
use opencat_core::scene::path_bounds::PathBoundsComputer;

use crate::backend::skia::renderer::SkiaRenderEngine;
use crate::resource::media::MediaContext;
use crate::resource::path_store::AssetPathStore;
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
}

impl EnginePlatform {
    pub fn new(backend: Arc<SkiaRenderEngine>) -> Self {
        Self {
            backend,
            script: ScriptRuntimeCache::default(),
            video: MediaContext::new(),
            asset_paths: AssetPathStore::new(),
            audio: AudioRuntime::new(),
            path_bounds: SkiaPathBounds,
        }
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
