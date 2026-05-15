//! WebPlatform — Platform facade for the wasm/web target.

use opencat_core::platform::platform::Platform;
use opencat_core::scene::path_bounds::{DefaultPathBounds, PathBoundsComputer};
use opencat_core::scene::script::precomputed_host::PrecomputedScriptHost;

use crate::codec::audio::WebAudio;
use crate::engine::WebRenderEngine;
use crate::video::WebVideoSource;

pub struct WebPlatform {
    pub backend: WebRenderEngine,
    pub script: PrecomputedScriptHost,
    pub video: WebVideoSource,
    pub audio: WebAudio,
    pub path_bounds: DefaultPathBounds,
}

impl Platform for WebPlatform {
    type Backend = WebRenderEngine;
    type Script = PrecomputedScriptHost;
    type Video = WebVideoSource;

    fn render_engine(&self) -> &Self::Backend {
        &self.backend
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
