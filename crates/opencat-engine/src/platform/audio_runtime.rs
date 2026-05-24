//! Pack engine-side audio caches into AudioRuntime,
//! so EnginePlatform holds a single field instead of 2.

use crate::media::{AudioIntervalCache, DecodedAudioCache};

#[derive(Default)]
pub struct AudioRuntime {
    pub decoded: DecodedAudioCache,
    pub interval: AudioIntervalCache,
}

impl AudioRuntime {
    pub fn new() -> Self {
        Self::default()
    }
}
