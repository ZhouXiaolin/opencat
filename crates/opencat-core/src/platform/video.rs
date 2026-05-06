//! Video frame provider trait — backend reads decoded RGBA frames by AssetId.
//!
//! Phase C: lets core pipeline call during record/draw without depending on
//! engine's MediaContext. Engine's MediaContext impls VideoFrameProvider; wasm
//! will have its own WebVideoSource.

use anyhow::Result;
use crate::resource::asset_id::AssetId;

/// Decoded RGBA video frame.
pub struct FrameBitmap {
    pub data: std::sync::Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

pub trait VideoFrameProvider {
    /// Get RGBA bitmap for video `id` at `frame` index.
    /// Returns Err if asset not preloaded.
    fn frame_rgba(&mut self, id: &AssetId, frame: u32) -> Result<FrameBitmap>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider;
    impl VideoFrameProvider for MockProvider {
        fn frame_rgba(&mut self, _id: &AssetId, frame: u32) -> Result<FrameBitmap> {
            Ok(FrameBitmap {
                data: std::sync::Arc::new(vec![frame as u8; 16]),
                width: 2,
                height: 2,
            })
        }
    }

    #[test]
    fn mock_video_provider_returns_bitmap() {
        let mut p = MockProvider;
        let id = AssetId("video:test".into());
        let bm = p.frame_rgba(&id, 5).expect("frame");
        assert_eq!(bm.width, 2);
        assert_eq!(bm.data.len(), 16);
        assert_eq!(bm.data[0], 5);
    }

    #[test]
    fn video_provider_is_object_safe() {
        let mut p: Box<dyn VideoFrameProvider> = Box::new(MockProvider);
        let id = AssetId("video:obj".into());
        let _bm = p.frame_rgba(&id, 0).unwrap();
    }
}
