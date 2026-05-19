//! WebPlatform -- wasm/web 端的 Platform 实现。


use opencat_core::platform::platform::Platform;

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

impl Platform for WebPlatform {
    type Script = ScriptRuntimeCache;
    type Video = WebVideoSource;

    fn script_host(&mut self) -> &mut Self::Script {
        &mut self.script
    }

    fn video_source(&mut self) -> &mut Self::Video {
        &mut self.video
    }
}
