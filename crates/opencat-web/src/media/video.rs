//! WebVideoSource — VideoFrameProvider for wasm target.
//!
//! The browser cannot synchronously decode during `frame_rgba`, so the JS side
//! pre-decodes the requested composition frame with web-demuxer + WebCodecs and
//! injects RGBA bytes before `build_frame`.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};

use opencat_core::platform::video::{FrameBitmap, VideoFrameProvider};
use opencat_core::resource::asset_id::AssetId;

#[derive(Default)]
pub struct WebVideoSource {
    frames: HashMap<(AssetId, u32), FrameBitmap>,
}

impl WebVideoSource {
    pub fn inject_frame(
        &mut self,
        asset_id: AssetId,
        frame: u32,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    ) {
        self.frames.insert(
            (asset_id, frame),
            FrameBitmap {
                data: Arc::new(rgba),
                width,
                height,
            },
        );
    }

    pub fn clear_cache(&mut self, asset_id: Option<&AssetId>) {
        match asset_id {
            Some(id) => self.frames.retain(|(cached_id, _), _| cached_id != id),
            None => self.frames.clear(),
        }
    }

    #[cfg(test)]
    fn cached_frame_count(&self) -> usize {
        self.frames.len()
    }
}

impl VideoFrameProvider for WebVideoSource {
    fn frame_rgba(&mut self, id: &AssetId, frame: u32) -> Result<FrameBitmap> {
        self.frames
            .get(&(id.clone(), frame))
            .cloned()
            .ok_or_else(|| anyhow!("video frame not preloaded: {id:?} frame={frame}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rgba_returns_injected_composition_frame() {
        let mut source = WebVideoSource::default();
        let id = AssetId("video:test.mp4".into());
        source.inject_frame(id.clone(), 12, vec![1, 2, 3, 4], 1, 1);

        let frame = source.frame_rgba(&id, 12).expect("injected frame");

        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(&*frame.data, &[1, 2, 3, 4]);
    }

    #[test]
    fn frame_rgba_is_keyed_by_composition_frame() {
        let mut source = WebVideoSource::default();
        let id = AssetId("video:test.mp4".into());
        source.inject_frame(id.clone(), 12, vec![1, 2, 3, 4], 1, 1);
        source.inject_frame(id.clone(), 13, vec![5, 6, 7, 8], 1, 1);

        let frame = source.frame_rgba(&id, 13).expect("injected frame");

        assert_eq!(&*frame.data, &[5, 6, 7, 8]);
    }

    #[test]
    fn clear_cache_removes_injected_frames() {
        let mut source = WebVideoSource::default();
        let id = AssetId("video:test.mp4".into());
        source.inject_frame(id.clone(), 12, vec![1, 2, 3, 4], 1, 1);
        source.clear_cache(Some(&id));

        assert!(source.frame_rgba(&id, 12).is_err());
    }

    #[test]
    fn clear_cache_can_remove_all_assets() {
        let mut source = WebVideoSource::default();
        source.inject_frame(AssetId("video:a.mp4".into()), 1, vec![1, 2, 3, 4], 1, 1);
        source.inject_frame(AssetId("video:b.mp4".into()), 1, vec![5, 6, 7, 8], 1, 1);

        source.clear_cache(None);

        assert_eq!(source.cached_frame_count(), 0);
    }
}
