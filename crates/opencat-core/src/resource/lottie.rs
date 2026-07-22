//! Lottie metadata types — hosts parse JSON and return `LottieMeta` to prepare.
//!
//! Per issue #40: Lottie JSON parsing has moved to hosts. Core retains only
//! the metadata struct and the purely-derived `resolve_lottie_frame` mapping.

use crate::resource::catalog::VideoInfoMeta;

/// Intrinsic timing/size and external dependency names from a Bodymovin root.
///
/// Hosts parse the primary JSON, return this metadata to prepare, and keep the
/// JSON/asset bytes on the host. Core never sees Lottie bytes in the prepare path.
#[derive(Debug, Clone, PartialEq)]
pub struct LottieMeta {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub in_frame: f32,
    pub out_frame: f32,
    /// External asset basenames (e.g. `image_0.png`). Data-URI embeds are omitted.
    pub dependencies: Vec<String>,
}

impl LottieMeta {
    pub fn duration_frames(&self) -> u32 {
        ((self.out_frame - self.in_frame).max(1.0)).round() as u32
    }

    /// Playable length in seconds (for [`crate::media::VideoFrameRequest`] clamp/loop).
    pub fn duration_secs(&self) -> f64 {
        self.duration_frames() as f64 / self.fps.max(1.0) as f64
    }
}

/// Map composition time + video-style timing to a Skottie frame index.
pub fn resolve_lottie_frame(
    request: &crate::media::VideoFrameRequest,
    meta: &LottieMeta,
) -> Option<f32> {
    if !request.is_visible() {
        return None;
    }
    let info = VideoInfoMeta {
        width: meta.width,
        height: meta.height,
        duration_micros: crate::time::optional_secs_to_duration_micros(Some(meta.duration_secs())),
    };
    let time_secs = request.resolve_time_secs(&info);
    let frame = meta.in_frame + time_secs as f32 * meta.fps;
    Some(frame.clamp(meta.in_frame, (meta.out_frame - 1.0).max(meta.in_frame)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_frame_respects_data_start_and_media_start() {
        use crate::media::{VideoFrameRequest, VideoFrameTiming};

        let meta = LottieMeta {
            width: 280,
            height: 200,
            fps: 25.0,
            in_frame: 0.0,
            out_frame: 32.0,
            dependencies: vec![],
        };
        let request = VideoFrameRequest {
            composition_time_secs: 0.4,
            timing: VideoFrameTiming {
                timeline_start_secs: 0.2,
                timeline_duration_secs: None,
                media_start_secs: 0.0,
                playback_rate: 1.0,
                looping: false,
            },
        };
        let frame = resolve_lottie_frame(&request, &meta).unwrap();
        assert!((frame - 5.0).abs() < 0.01);

        let hidden = VideoFrameRequest {
            composition_time_secs: 0.1,
            timing: request.timing,
        };
        assert!(resolve_lottie_frame(&hidden, &meta).is_none());
    }

    #[test]
    fn resolve_frame_maps_composition_time_to_lottie_frame() {
        let meta = LottieMeta {
            width: 100,
            height: 100,
            fps: 10.0,
            in_frame: 0.0,
            out_frame: 20.0,
            dependencies: vec![],
        };
        let request = crate::media::VideoFrameRequest {
            composition_time_secs: 0.5,
            timing: crate::media::VideoFrameTiming::default(),
        };
        let frame = resolve_lottie_frame(&request, &meta).unwrap();
        assert!((frame - 5.0).abs() < 0.01, "frame={frame}");
    }
}
