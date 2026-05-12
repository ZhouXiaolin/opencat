//! WebVideoSource — stub VideoFrameProvider for wasm target.
//!
//! Real frame decoding will be wired in Phase D7.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};

use opencat_core::platform::video::{FrameBitmap, VideoFrameProvider};
use opencat_core::resource::asset_id::AssetId;

pub struct WebVideoSource {
    frames: HashMap<(AssetId, u32), Arc<Vec<u8>>>,
}

impl Default for WebVideoSource {
    fn default() -> Self {
        Self {
            frames: HashMap::new(),
        }
    }
}

impl VideoFrameProvider for WebVideoSource {
    fn frame_rgba(&mut self, id: &AssetId, frame: u32) -> Result<FrameBitmap> {
        if let Some(data) = self.frames.get(&(id.clone(), frame)) {
            return Ok(FrameBitmap {
                data: data.clone(),
                width: 0,
                height: 0,
            });
        }
        Err(anyhow!(
            "WebVideoSource not yet implemented (planned for D7)"
        ))
    }
}
