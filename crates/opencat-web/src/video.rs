
//! WebVideoSource — VideoFrameProvider for wasm target.
//!
//! Supports two injection paths:
//!   - inject_frame: pre-decoded RGBA bytes (legacy, CPU round-trip)
//!   - inject_texture: CanvasKit SkImage handle (zero-copy GPU texture)
//!
//! The rendering pipeline checks `take_texture()` first; if it returns
//! `Some`, the SkImage is used directly without any RGBA conversion.
//! Otherwise the pipeline falls back to `frame_rgba()`.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use wasm_bindgen::JsValue;

use opencat_core::platform::video::{FrameBitmap, VideoFrameProvider};
use opencat_core::resource::asset_id::AssetId;

/// Metadata for a previously decoded video (dimensions per asset).
struct VideoMeta {
    width: u32,
    height: u32,
}

/// Zero-copy GPU texture entry (CanvasKit SkImage handle).
struct TextureEntry {
    #[allow(dead_code)]
    image: JsValue,
    width: u32,
    height: u32,
}

#[derive(Default)]
pub struct WebVideoSource {
    /// Latest decoded RGBA frame per asset (replaced on each inject).
    frames: HashMap<AssetId, Arc<Vec<u8>>>,
    /// Video dimensions per asset (set during frame injection).
    meta: HashMap<AssetId, VideoMeta>,
    /// Zero-copy GPU textures per asset (set by inject_texture, consumed by take_texture).
    textures: HashMap<AssetId, TextureEntry>,
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

    /// Inject a CanvasKit SkImage as a zero-copy GPU texture.
    /// The `image` must be obtained from `CK.MakeLazyImageFromTextureSource`
    /// and updated each frame with `surface.updateTextureFromSource`.
    pub fn inject_texture(
        &mut self,
        asset_id: AssetId,
        image: JsValue,
        width: u32,
        height: u32,
    ) {
        self.meta.insert(
            asset_id.clone(),
            VideoMeta { width, height },
        );
        self.textures.insert(asset_id, TextureEntry { image, width, height });
    }

    /// Take (consume) a previously injected GPU texture.
    /// Called by the canvas during rendering — if this returns `Some`,
    /// the texture path is used; otherwise `frame_rgba` fallback kicks in.
    pub fn take_texture(&mut self, id: &AssetId) -> Option<(JsValue, u32, u32)> {
        self.textures.remove(id).map(|e| (e.image, e.width, e.height))
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
                self.textures.remove(id);
            }
            None => {
                self.frames.clear();
                self.meta.clear();
                self.textures.clear();
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
