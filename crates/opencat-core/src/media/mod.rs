//! Backend-neutral media contracts and planning helpers.
//!
//! This module owns pure media vocabulary and strategy. Platform crates keep
//! IO, decoder, renderer, and encoder implementations.

pub mod codec;
pub mod export;
pub mod preview;
pub mod seek;
pub mod types;

pub use types::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::codec::{AudioPcm, VideoFrameRgba, VideoSourceMeta};
    use super::export::{ExportFrameSource, ExportJob, ExportKind, ExportProgress};
    use super::preview::PreviewFrameRequest;
    use super::{VideoFrameTiming, VideoPreviewQuality};

    #[test]
    fn codec_audio_pcm_reports_sample_frames() {
        let pcm = AudioPcm::new(48_000, 2, vec![0.0; 96]);
        assert_eq!(pcm.sample_frames(), 48);
        assert!(!pcm.is_empty());
    }

    #[test]
    fn preview_frame_request_maps_frame_to_video_request() {
        let preview = PreviewFrameRequest {
            frame_index: 12,
            fps: 24,
            quality: VideoPreviewQuality::Scrubbing,
            target_size: Some((320, 180)),
        };

        let request = preview.video_frame_request(VideoFrameTiming::default());

        assert_eq!(request.composition_time_secs, 0.5);
        assert_eq!(request.quality, VideoPreviewQuality::Scrubbing);
        assert_eq!(request.target_size, Some((320, 180)));
    }

    #[test]
    fn export_progress_reports_ratio() {
        let progress = ExportProgress {
            completed_frames: 12,
            total_frames: 48,
        };
        assert_eq!(progress.ratio(), 0.25);
    }

    #[test]
    fn export_frame_source_contract_returns_rgba_frame() {
        struct OneFrame;

        impl ExportFrameSource for OneFrame {
            fn frame_rgba(&mut self, frame_index: u32) -> anyhow::Result<VideoFrameRgba> {
                assert_eq!(frame_index, 3);
                Ok(VideoFrameRgba {
                    data: Arc::new(vec![255; 16]),
                    width: 2,
                    height: 2,
                })
            }
        }

        let job = ExportJob {
            width: 2,
            height: 2,
            fps: 30,
            frames: 4,
            kind: ExportKind::Mp4,
        };
        let meta = VideoSourceMeta {
            width: job.width,
            height: job.height,
            duration_secs: Some(job.frames as f64 / job.fps as f64),
        };

        let mut source = OneFrame;
        let frame = source.frame_rgba(3).expect("frame");
        assert_eq!(frame.width, meta.width);
        assert_eq!(frame.data.len(), 16);
    }
}
