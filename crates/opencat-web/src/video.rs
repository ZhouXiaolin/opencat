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
    /// Pre-injected RGBA frame data keyed by (AssetId, frame index).
    frames: HashMap<(AssetId, u32), Arc<Vec<u8>>>,
    /// Video dimensions per asset (set during frame injection).
    meta: HashMap<AssetId, VideoMeta>,
}

impl WebVideoSource {
    /// Inject a pre-decoded RGBA frame from the JS side.
    /// Call during preflight after `prepareVideoFrames()` completes.
    pub fn inject_frame(
        &mut self,
        asset_id: AssetId,
        frame: u32,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    ) {
        self.meta.insert(
            asset_id.clone(),
            VideoMeta { width, height },
        );
        self.frames
            .insert((asset_id, frame), Arc::new(rgba));
    }

    /// Number of cached frames (useful for diagnostics).
    pub fn cached_frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Clear cached frames for a specific asset, or all frames if `None`.
    pub fn clear_cache(&mut self, asset_id: Option<&AssetId>) {
        match asset_id {
            Some(id) => {
                self.meta.remove(id);
                self.frames.retain(|(aid, _), _| aid != id);
            }
            None => {
                self.frames.clear();
                self.meta.clear();
            }
        }
    }
}

impl VideoFrameProvider for WebVideoSource {
    fn frame_rgba(&mut self, id: &AssetId, frame: u32) -> Result<FrameBitmap> {
        // Look up pre-injected frame in cache
        if let Some(data) = self.frames.get(&(id.clone(), frame)) {
            let meta = self.meta.get(id);
            let (w, h) = match meta {
                Some(m) => (m.width, m.height),
                None => (0, 0),
            };
            return Ok(FrameBitmap {
                data: data.clone(),
                width: w,
                height: h,
            });
        }

        // Fallback: try synchronous JS decode via global window function
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(rgba) = js_sync_decode(&id.0, frame) {
                let meta = self.meta.get(id);
                let (w, h) = match meta {
                    Some(m) => (m.width, m.height),
                    None => (0, 0),
                };
                let data = Arc::new(rgba);
                self.frames.insert((id.clone(), frame), data.clone());
                return Ok(FrameBitmap {
                    data,
                    width: w,
                    height: h,
                });
            }
        }

        Err(anyhow!("video frame not preloaded: {id:?}:{frame}"))
    }
}

/// Attempt synchronous frame decode via JS global `__video_decode_frame_sync`.
/// Returns `Some(rgba_bytes)` if the JS side has the frame cached.
#[cfg(target_arch = "wasm32")]
fn js_sync_decode(url: &str, frame: u32) -> Option<Vec<u8>> {
    use js_sys::{Reflect, Uint8Array};
    use wasm_bindgen::{JsCast, JsValue};

    let window = web_sys::window()?;
    let global_fn = Reflect::get(&window, &JsValue::from_str("__video_decode_frame_sync"))
        .ok()?;

    if !global_fn.is_function() {
        return None;
    }

    let fn_obj = global_fn.dyn_into::<js_sys::Function>().ok()?;
    let result = fn_obj
        .call2(&JsValue::NULL, &JsValue::from_str(url), &JsValue::from_f64(frame as f64))
        .ok()?;

    if result.is_null() || result.is_undefined() {
        return None;
    }

    let arr = Uint8Array::new(&result);
    Some(arr.to_vec())
}
