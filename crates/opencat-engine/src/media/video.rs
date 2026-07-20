use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, image::CachingHint};

use crate::media::decode::VideoDecodeCache;
use crate::media::video_cache::VideoFrameCache;
use crate::runtime::cache::CacheCaps;
use opencat_core::probe::bitmap_source::{BitmapSourceKind, bitmap_source_kind};

pub use crate::media::decode::VideoInfo;
pub use opencat_core::media::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};

impl From<&VideoInfo> for opencat_core::resource::catalog::VideoInfoMeta {
    fn from(v: &VideoInfo) -> Self {
        Self {
            width: v.width,
            height: v.height,
            duration_secs: v.duration_secs,
        }
    }
}

/// Bucket size for `target_size`. Keeping target sizes on a 16-pixel grid bounds
/// the number of distinct sws scaling contexts and cache entries we generate
/// for an animation that drifts continuously across sizes.
pub(crate) const TARGET_SIZE_ALIGN: u32 = 16;

/// Normalize a caller-requested size against the actual source video.
///
/// Returns `None` when the request would scale to source resolution or larger
/// (decode at native size, no sws scale). Otherwise returns a 16-pixel-aligned
/// bucket clamped to the source dimensions.
pub(crate) fn quantize_target_size(
    requested: Option<(u32, u32)>,
    info: &VideoInfo,
) -> Option<(u32, u32)> {
    let (tw, th) = requested?;
    if tw >= info.width && th >= info.height {
        return None;
    }
    let bucket = |v: u32, max: u32| -> u32 {
        let aligned = v.div_ceil(TARGET_SIZE_ALIGN) * TARGET_SIZE_ALIGN;
        aligned.clamp(TARGET_SIZE_ALIGN, max)
    };
    let qw = bucket(tw, info.width);
    let qh = bucket(th, info.height);
    if qw >= info.width && qh >= info.height {
        None
    } else {
        Some((qw, qh))
    }
}

pub struct VideoBitmap {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub frame_cache_hit: bool,
}

pub struct MediaContext {
    videos: VideoDecodeCache,
    images: HashMap<PathBuf, (Arc<Vec<u8>>, u32, u32)>,
    video_frame_cache: VideoFrameCache,
    video_preview_quality: VideoPreviewQuality,
    composition_fps: u32,
}

impl MediaContext {
    pub fn new() -> Self {
        Self::with_cache_caps(CacheCaps::default())
    }

    pub fn with_cache_caps(caps: CacheCaps) -> Self {
        Self {
            videos: VideoDecodeCache::new(),
            images: HashMap::new(),
            video_frame_cache: VideoFrameCache::new(caps.video_frames),
            video_preview_quality: VideoPreviewQuality::Realtime,
            composition_fps: 30,
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: VideoPreviewQuality) {
        self.video_preview_quality = quality;
    }

    pub fn video_preview_quality(&self) -> VideoPreviewQuality {
        self.video_preview_quality
    }

    pub fn set_composition_fps(&mut self, fps: u32) {
        self.composition_fps = fps;
    }

    pub fn get_video_frame(
        &mut self,
        path: &Path,
        request: VideoFrameRequest,
    ) -> Result<(Arc<Vec<u8>>, u32, u32, bool)> {
        let info = self.video_info(path)?;
        let meta: opencat_core::resource::catalog::VideoInfoMeta = (&info).into();
        let target_time_secs = request.resolve_time_secs(&meta);
        let scale_target = quantize_target_size(request.target_size, &info);
        let (out_w, out_h) = scale_target.unwrap_or((info.width, info.height));
        if let Some(cached) = self
            .video_frame_cache
            .get(path, target_time_secs, scale_target)
        {
            return Ok((cached, out_w, out_h, true));
        }
        let data = self
            .videos
            .get_frame(path, target_time_secs, request.quality, scale_target)?;
        self.video_frame_cache
            .insert(path, target_time_secs, scale_target, data.clone());
        Ok((data, out_w, out_h, false))
    }

    pub fn video_info(&mut self, path: &Path) -> Result<VideoInfo> {
        self.videos.info(path)
    }

    pub fn get_video_bitmap(
        &mut self,
        path: &Path,
        request: VideoFrameRequest,
    ) -> Result<VideoBitmap> {
        let (data, width, height, frame_cache_hit) = self.get_video_frame(path, request)?;
        Ok(VideoBitmap {
            data,
            width,
            height,
            frame_cache_hit,
        })
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
                let bitmap = self.get_video_bitmap(path, request)?;
                Ok((bitmap.data, bitmap.width, bitmap.height))
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

    pub fn frame_rgba_by_path(
        &mut self,
        path: &Path,
        frame: u32,
    ) -> Result<opencat_core::platform::video::FrameBitmap> {
        let composition_time_secs = frame as f64 / self.composition_fps.max(1) as f64;
        let request = VideoFrameRequest {
            composition_time_secs,
            timing: VideoFrameTiming::default(),
            quality: self.video_preview_quality,
            target_size: None,
        };
        let (data, width, height, _hit) = self.get_video_frame(path, request)?;
        Ok(opencat_core::platform::video::FrameBitmap {
            data,
            width,
            height,
        })
    }

    pub fn frame_rgba_at_time_by_path(
        &mut self,
        path: &Path,
        time_secs: f64,
    ) -> Result<opencat_core::platform::video::FrameBitmap> {
        let request = VideoFrameRequest {
            composition_time_secs: time_secs.max(0.0),
            timing: VideoFrameTiming::default(),
            quality: self.video_preview_quality,
            target_size: None,
        };
        let (data, width, height, _hit) = self.get_video_frame(path, request)?;
        Ok(opencat_core::platform::video::FrameBitmap {
            data,
            width,
            height,
        })
    }
}

impl Default for MediaContext {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// VideoFrameProvider adapter
// ---------------------------------------------------------------------------

use opencat_core::ir::asset_id::AssetId;
use opencat_core::platform::video::{FrameBitmap, VideoFrameProvider};
use crate::resource::AssetPathStore;

/// Adapter pairing a [`MediaContext`] with an [`AssetPathStore`] so it can
/// serve [`VideoFrameProvider`]'s `AssetId`-indexed contract.
pub struct EngineVideoProvider<'a> {
    pub media: &'a mut MediaContext,
    pub paths: &'a AssetPathStore,
}

impl VideoFrameProvider for EngineVideoProvider<'_> {
    fn frame_rgba(&mut self, id: &AssetId, frame: u32) -> anyhow::Result<FrameBitmap> {
        let path = self
            .paths
            .path(id)
            .ok_or_else(|| anyhow::anyhow!("video asset {:?} not in path store", id))?;
        self.media.frame_rgba_by_path(path, frame)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rgba_by_path_returns_err_for_missing_video() {
        let mut ctx = MediaContext::new();
        ctx.set_composition_fps(30);
        let result = ctx.frame_rgba_by_path(Path::new("/nonexistent/video.mp4"), 0);
        assert!(result.is_err());
    }

    #[test]
    fn set_composition_fps_updates_field() {
        let mut ctx = MediaContext::new();
        assert_eq!(ctx.composition_fps, 30);
        ctx.set_composition_fps(60);
        assert_eq!(ctx.composition_fps, 60);
    }

    #[test]
    fn quantize_target_size_returns_none_when_at_or_above_source() {
        let info = VideoInfo {
            width: 1920,
            height: 1080,
            duration_secs: None,
        };
        assert_eq!(quantize_target_size(None, &info), None);
        assert_eq!(quantize_target_size(Some((1920, 1080)), &info), None);
        assert_eq!(quantize_target_size(Some((4000, 4000)), &info), None);
    }

    #[test]
    fn quantize_target_size_buckets_to_16_pixel_grid() {
        let info = VideoInfo {
            width: 1920,
            height: 1080,
            duration_secs: None,
        };
        // 320x180 -> already aligned
        assert_eq!(
            quantize_target_size(Some((320, 180)), &info),
            Some((320, 192))
        );
        // 321x181 rounds up to next 16 boundary
        assert_eq!(
            quantize_target_size(Some((321, 181)), &info),
            Some((336, 192))
        );
        // values below the alignment floor get clamped to 16
        assert_eq!(quantize_target_size(Some((4, 4)), &info), Some((16, 16)));
    }

    #[test]
    fn engine_video_provider_returns_err_for_missing_path() {
        use opencat_core::platform::video::VideoFrameProvider;

        let mut mc = MediaContext::new();
        let paths = AssetPathStore::new();
        let mut provider = super::EngineVideoProvider {
            media: &mut mc,
            paths: &paths,
        };
        let id = opencat_core::ir::asset_id::AssetId("nonexistent".into());
        assert!(provider.frame_rgba(&id, 0).is_err());
    }

    #[test]
    fn quantize_target_size_clamps_to_source_resolution() {
        let info = VideoInfo {
            width: 100,
            height: 100,
            duration_secs: None,
        };
        // Above source on one axis but below on the other -> still scale, clamped
        assert_eq!(
            quantize_target_size(Some((50, 200)), &info),
            Some((64, 100))
        );
    }
}
