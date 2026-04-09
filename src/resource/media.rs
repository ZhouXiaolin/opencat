use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, image::CachingHint};

use crate::codec::decode::VideoDecodeCache;
use crate::resource::bitmap_source::{BitmapSourceKind, bitmap_source_kind};

pub use crate::codec::decode::VideoInfo;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VideoPreviewQuality {
    Realtime,
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoFrameTiming {
    pub media_offset_secs: f64,
    pub playback_rate: f64,
    pub looping: bool,
}

impl Default for VideoFrameTiming {
    fn default() -> Self {
        Self {
            media_offset_secs: 0.0,
            playback_rate: 1.0,
            looping: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoFrameRequest {
    pub composition_time_secs: f64,
    pub timing: VideoFrameTiming,
    pub quality: VideoPreviewQuality,
}

impl VideoFrameRequest {
    pub fn resolve_time_secs(&self, info: &VideoInfo) -> f64 {
        let composition_time_secs = self.composition_time_secs.max(0.0);
        let local_time_secs =
            self.timing.media_offset_secs + composition_time_secs * self.timing.playback_rate;

        if !self.timing.looping {
            return clamp_video_time(local_time_secs, info.duration_secs);
        }

        match info.duration_secs {
            Some(duration_secs) if duration_secs > self.timing.media_offset_secs => {
                let playable_duration = duration_secs - self.timing.media_offset_secs;
                let wrapped =
                    (composition_time_secs * self.timing.playback_rate) % playable_duration;
                self.timing.media_offset_secs + wrapped
            }
            _ => clamp_video_time(local_time_secs, info.duration_secs),
        }
    }
}

pub struct MediaContext {
    videos: VideoDecodeCache,
    images: HashMap<PathBuf, (Arc<Vec<u8>>, u32, u32)>,
    video_preview_quality: VideoPreviewQuality,
}

impl MediaContext {
    pub fn new() -> Self {
        Self {
            videos: VideoDecodeCache::new(),
            images: HashMap::new(),
            video_preview_quality: VideoPreviewQuality::Realtime,
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: VideoPreviewQuality) {
        self.video_preview_quality = quality;
    }

    pub fn video_preview_quality(&self) -> VideoPreviewQuality {
        self.video_preview_quality
    }

    pub fn get_video_frame(
        &mut self,
        path: &Path,
        request: VideoFrameRequest,
    ) -> Result<Arc<Vec<u8>>> {
        let info = self.video_info(path)?;
        let target_time_secs = request.resolve_time_secs(&info);
        self.videos
            .get_frame(path, target_time_secs, request.quality)
    }

    pub fn video_info(&mut self, path: &Path) -> Result<VideoInfo> {
        self.videos.info(path)
    }

    pub fn get_bitmap(
        &mut self,
        path: &Path,
        video_request: Option<VideoFrameRequest>,
    ) -> Result<(Arc<Vec<u8>>, u32, u32)> {
        match bitmap_source_kind(path) {
            BitmapSourceKind::Video => {
                let request = video_request.ok_or_else(|| {
                    anyhow!("video bitmap request is required for {}", path.display())
                })?;
                let data = self.get_video_frame(path, request)?;
                let info = self.video_info(path)?;
                Ok((data, info.width, info.height))
            }
            BitmapSourceKind::StaticImage => {
                if !self.images.contains_key(path) {
                    let bitmap = load_image_bitmap(path)?;
                    self.images.insert(path.to_path_buf(), bitmap);
                }

                Ok(self
                    .images
                    .get(path)
                    .expect("cached image bitmap should exist")
                    .clone())
            }
        }
    }
}

impl Default for MediaContext {
    fn default() -> Self {
        Self::new()
    }
}

fn clamp_video_time(time_secs: f64, duration_secs: Option<f64>) -> f64 {
    let clamped = time_secs.max(0.0);
    match duration_secs {
        Some(duration_secs) if duration_secs > 0.0 => clamped.min(duration_secs),
        _ => clamped,
    }
}

#[cfg(test)]
mod tests {
    use super::{VideoFrameRequest, VideoFrameTiming, VideoInfo, VideoPreviewQuality};

    #[test]
    fn video_frame_request_applies_media_offset_and_rate() {
        let info = VideoInfo {
            width: 1920,
            height: 1080,
            duration_secs: Some(12.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 2.0,
            timing: VideoFrameTiming {
                media_offset_secs: 1.5,
                playback_rate: 0.5,
                looping: false,
            },
            quality: VideoPreviewQuality::Exact,
        };

        assert!((request.resolve_time_secs(&info) - 2.5).abs() < 1e-6);
    }

    #[test]
    fn video_frame_request_wraps_looping_video_time() {
        let info = VideoInfo {
            width: 1920,
            height: 1080,
            duration_secs: Some(5.0),
        };
        let request = VideoFrameRequest {
            composition_time_secs: 6.0,
            timing: VideoFrameTiming {
                media_offset_secs: 1.0,
                playback_rate: 1.0,
                looping: true,
            },
            quality: VideoPreviewQuality::Realtime,
        };

        assert!((request.resolve_time_secs(&info) - 3.0).abs() < 1e-6);
    }
}

fn load_image_bitmap(path: &Path) -> Result<(Arc<Vec<u8>>, u32, u32)> {
    let encoded = fs::read(path)
        .with_context(|| format!("failed to read image bytes: {}", path.display()))?;
    let image = Image::from_encoded(Data::new_copy(&encoded))
        .ok_or_else(|| anyhow!("failed to decode image: {}", path.display()))?;

    let width = image.width() as u32;
    let height = image.height() as u32;
    let row_bytes = width as usize * 4;
    let mut pixels = vec![0_u8; row_bytes * height as usize];
    let info = ImageInfo::new(
        (width as i32, height as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );

    let ok = image.read_pixels(
        &info,
        pixels.as_mut_slice(),
        row_bytes,
        (0, 0),
        CachingHint::Allow,
    );
    if !ok {
        return Err(anyhow!(
            "failed to convert decoded image into RGBA pixels: {}",
            path.display()
        ));
    }

    Ok((Arc::new(pixels), width, height))
}
