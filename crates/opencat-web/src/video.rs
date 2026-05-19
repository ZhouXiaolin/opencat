//! WebVideoSource — VideoFrameProvider for wasm target.
//!
//! Receives pre-decoded RGBA frames from the JS side via `inject_frame()`.
//! JS-side video decoding uses HTMLVideoElement + OffscreenCanvas
//! (see web/src/video-decoder.ts).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};

use opencat_core::platform::video::{FrameBitmap, VideoFrameProvider};
use opencat_core::resource::asset_id::AssetId;

/// Metadata for a previously decoded video (dimensions per asset).
struct VideoMeta {
    width: u32,
    height: u32,
}

#[derive(Default)]
pub struct WebVideoSource {
    /// Latest decoded RGBA frame per asset (replaced on each inject).
    frames: HashMap<AssetId, Arc<Vec<u8>>>,
    /// Video dimensions per asset (set during frame injection).
    meta: HashMap<AssetId, VideoMeta>,
}

impl WebVideoSource {
    /// Inject a pre-decoded RGBA frame from the JS side.
    /// Called before each `build_frame` with the frame decoded at the correct time.
    pub fn inject_frame(
        &mut self,
        asset_id: AssetId,
        _frame: u32,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    ) {
        self.meta.insert(
            asset_id.clone(),
            VideoMeta { width, height },
        );
        self.frames.insert(asset_id, Arc::new(rgba));
    }

    /// Number of cached assets (useful for diagnostics).
    pub fn cached_frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Clear cached frames for a specific asset, or all frames if `None`.
    pub fn clear_cache(&mut self, asset_id: Option<&AssetId>) {
        match asset_id {
            Some(id) => {
                self.meta.remove(id);
                self.frames.remove(id);
            }
            None => {
                self.frames.clear();
                self.meta.clear();
            }
        }
    }
}

impl VideoFrameProvider for WebVideoSource {
    fn frame_rgba(&mut self, id: &AssetId, _frame: u32) -> Result<FrameBitmap> {
        if let Some(data) = self.frames.get(id) {
            let (w, h) = self.meta.get(id)
                .map(|m| (m.width, m.height))
                .unwrap_or((0, 0));
            return Ok(FrameBitmap {
                data: data.clone(),
                width: w,
                height: h,
            });
        }
        Err(anyhow!("video frame not preloaded: {id:?}"))
    }
}
