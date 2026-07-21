//! Engine-side runtime state: quickjs script runtime, ffmpeg/skia media
//! context, asset path table, and audio caches.

pub mod audio_runtime;

use crate::media::{AudioIntervalCache, DecodedAudioCache, MediaContext, VideoPreviewQuality};
use crate::resource::AssetPathStore;
use crate::script::ScriptRuntimeCache;

pub use audio_runtime::AudioRuntime;

/// Engine runtime services owned by the engine render session.
pub struct EnginePlatform {
    pub script: ScriptRuntimeCache,
    pub video: MediaContext,
    pub asset_paths: AssetPathStore,
    pub audio: AudioRuntime,
    /// Decoded audio cache for streaming playback.
    pub audio_decode_cache: DecodedAudioCache,
    /// Audio interval cache for composition-level audio scheduling.
    pub audio_interval_cache: AudioIntervalCache,
}

impl EnginePlatform {
    pub fn new() -> Self {
        Self {
            script: ScriptRuntimeCache::default(),
            video: MediaContext::new(),
            asset_paths: AssetPathStore::new(),
            audio: AudioRuntime::new(),
            audio_decode_cache: DecodedAudioCache::default(),
            audio_interval_cache: AudioIntervalCache::default(),
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: VideoPreviewQuality) {
        self.video.set_video_preview_quality(quality);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_platform_constructs_runtime_services() {
        let platform = EnginePlatform::new();
        assert!(platform.asset_paths.entries.is_empty());
    }
}
