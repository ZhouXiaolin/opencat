use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use skia_safe::{
    AlphaType, ColorType, CubicResampler, Data, Image, ImageInfo, Paint, Rect, SamplingOptions,
    image::CachingHint, images, surfaces,
};

use crate::media::decode::VideoDecodeCache;
use crate::media::realtime::VideoRealtimeDecoder;
use crate::media::video_cache::VideoFrameCache;
use crate::runtime::cache::CacheCaps;
use opencat_core::probe::bitmap_source::{BitmapSourceKind, bitmap_source_kind};

pub use crate::media::decode::VideoInfo;
pub use opencat_core::media::VideoFrameTiming;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoPreviewQuality {
    Scrubbing,
    Realtime,
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrameRequest {
    pub composition_time_secs: f64,
    pub timing: VideoFrameTiming,
    pub quality: VideoPreviewQuality,
    pub target_size: Option<(u32, u32)>,
}

impl VideoFrameRequest {
    fn timeline_request(self) -> opencat_core::media::VideoFrameRequest {
        opencat_core::media::VideoFrameRequest {
            composition_time_secs: self.composition_time_secs,
            timing: self.timing,
        }
    }

    pub fn resolve_time_secs(self, info: &opencat_core::probe::catalog::VideoInfoMeta) -> f64 {
        self.timeline_request().resolve_time_secs(info)
    }

    pub fn is_visible(self) -> bool {
        self.timeline_request().is_visible()
    }
}

impl From<&VideoInfo> for opencat_core::probe::catalog::VideoInfoMeta {
    fn from(v: &VideoInfo) -> Self {
        Self {
            width: v.width,
            height: v.height,
            duration_micros: opencat_core::time::optional_secs_to_duration_micros(v.duration_secs),
            color_matrix: None,
        }
    }
}

/// Bucket size for `target_size`. Keeping target sizes on a 16-pixel grid bounds
/// the number of distinct sws scaling contexts and cache entries we generate
/// for an animation that drifts continuously across sizes.
pub(crate) const TARGET_SIZE_ALIGN: u32 = 16;

/// Quantize a requested size onto the 16-pixel grid, clamped to `max` per axis.
/// Returns `None` when the request is already at or above `max` (no shrink).
fn quantize_bucket(tw: u32, th: u32, max_w: u32, max_h: u32) -> Option<(u32, u32)> {
    if tw >= max_w && th >= max_h {
        return None;
    }
    let bucket = |v: u32, max: u32| -> u32 {
        let aligned = v.div_ceil(TARGET_SIZE_ALIGN) * TARGET_SIZE_ALIGN;
        aligned.clamp(TARGET_SIZE_ALIGN, max)
    };
    let qw = bucket(tw, max_w);
    let qh = bucket(th, max_h);
    if qw >= max_w && qh >= max_h {
        None
    } else {
        Some((qw, qh))
    }
}

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
    quantize_bucket(tw, th, info.width, info.height)
}

pub struct VideoBitmap {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub frame_cache_hit: bool,
    /// Source stream has an alpha plane. When false every decoded pixel has
    /// alpha == 255, so `data` (straight RGBA) is byte-identical to its
    /// premultiplied form.
    pub has_alpha: bool,
}

pub struct MediaContext {
    videos: VideoDecodeCache,
    images: HashMap<PathBuf, (Arc<Vec<u8>>, u32, u32)>,
    /// Lazy encoded static images keyed by path. A lazy Skia image holds the
    /// encoded bytes (file size), not decoded RGBA, so full-resolution pixels
    /// are never resident unless something actually rasterizes them.
    static_images: HashMap<PathBuf, Image>,
    /// Rasterized shrinks of static images keyed by (path, bucket w, bucket h).
    /// Entries are at most the composition's draw size per asset, so this stays
    /// tiny compared to native-resolution decodes.
    scaled_images: HashMap<(PathBuf, u32, u32), Image>,
    /// Lazy decodes of frame-embedded encoded images (`ImageRef::Encoded`,
    /// svg `<image>` bytes interned by core). Keyed by the frame's byte-table
    /// id; a tree usually re-emits the same id every frame.
    encoded_images: HashMap<opencat_core::ir::draw_types::BytesRangeId, Image>,
    video_frame_cache: VideoFrameCache,
    video_preview_quality: VideoPreviewQuality,
    composition_fps: u32,
    /// 实时解码线程。挂上后 [`Self::get_video_frame`] 先查它的帧窗口，命中就
    /// 完全不等解码。离线渲染不挂它。
    realtime: Option<Arc<VideoRealtimeDecoder>>,
}

impl MediaContext {
    pub fn new() -> Self {
        Self::with_cache_caps(CacheCaps::default())
    }

    pub fn with_cache_caps(caps: CacheCaps) -> Self {
        Self {
            videos: VideoDecodeCache::new(),
            images: HashMap::new(),
            static_images: HashMap::new(),
            scaled_images: HashMap::new(),
            encoded_images: HashMap::new(),
            video_frame_cache: VideoFrameCache::new(caps.video_frames),
            video_preview_quality: VideoPreviewQuality::Realtime,
            composition_fps: 30,
            realtime: None,
        }
    }

    /// 打开实时视频解码线程（预览器用）。
    ///
    /// 必须在 [`Self::set_composition_fps`] 之后调用：解码线程向前推进的步长
    /// 取自合成帧率。离线渲染不调用它，`get_video_frame` 会直接同步解码。
    pub fn enable_realtime_decode(&mut self) {
        let decoder = VideoRealtimeDecoder::spawn(self.composition_fps, self.video_preview_quality);
        self.realtime = Some(Arc::new(decoder));
    }

    /// 预热一个视频素材：打开、探测、解一帧丢掉，让解码线程在开播前就位。
    ///
    /// 第一次解一个 1080p 素材要 ~300ms，落在播放中间就是一帧的卡顿，还会连锁
    /// 触发跳帧。没有挂线程时是空操作。
    pub fn warmup_video(&self, path: &Path) {
        if let Some(decoder) = &self.realtime {
            decoder.warmup(path);
        }
    }

    /// 告诉实时解码线程播放头现在在哪。线程随后把该素材的窗口补到播放头前方，
    /// 下一帧取帧就命中窗口。没有挂线程时是空操作。
    pub fn prefetch_video(
        &self,
        path: &Path,
        time_secs: f64,
        target_size: Option<(u32, u32)>,
        color_matrix: Option<opencat_core::media::VideoColorMatrix>,
    ) {
        if let Some(decoder) = &self.realtime {
            decoder.prefetch(path, time_secs, target_size, color_matrix);
        }
    }

    pub fn set_video_preview_quality(&mut self, quality: VideoPreviewQuality) {
        self.video_preview_quality = quality;
    }

    /// Author-asserted YUV→RGB matrix for a video asset
    /// (`data-color-matrix`). Routes the decoder's conversion to the asset's
    /// real encoding when stream metadata is mislabeled.
    pub fn set_video_color_matrix(
        &mut self,
        path: &Path,
        matrix: opencat_core::media::VideoColorMatrix,
    ) {
        self.videos.set_color_matrix_override(path, matrix);
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
        let meta: opencat_core::probe::catalog::VideoInfoMeta = (&info).into();
        let target_time_secs = request.resolve_time_secs(&meta);
        let scale_target = quantize_target_size(request.target_size, &info);
        let (out_w, out_h) = scale_target.unwrap_or((info.width, info.height));
        // 实时窗口优先：它由后台线程按同一个请求时刻填充。窗口里还没有就等它送来
        // ——主线程自己再解一遍会让两个解码器读同一个文件，互相拖慢。等超时了才
        // 落到下面的同步路径。
        //
        // 直接返回窗口帧自己的尺寸：解码尺寸和本次请求的绘制尺寸可能不同（解码线程
        // 是按**上一次**提示的尺寸解的），但调用方用 `width / 源宽` 算解码缩放比，
        // 绘制路径按这个比例映射 src，所以尺寸不一致不会画错。按请求尺寸重采样反而
        // 要每帧多搬一遍整幅 RGBA。
        if let Some(frame) = self
            .realtime
            .as_ref()
            .and_then(|decoder| decoder.wait_for(path, target_time_secs))
        {
            return Ok((frame.data, frame.width, frame.height, true));
        }
        if !opencat_core::ablate::ablated(opencat_core::ablate::Ablation::VideoFrames)
            && let Some(cached) = self
                .video_frame_cache
                .get(path, target_time_secs, scale_target)
        {
            return Ok((cached, out_w, out_h, true));
        }
        let data = self
            .videos
            .get_frame(path, target_time_secs, request.quality, scale_target)?;
        if !opencat_core::ablate::ablated(opencat_core::ablate::Ablation::VideoFrames) {
            self.video_frame_cache
                .insert(path, target_time_secs, scale_target, data.clone());
        }
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
        let has_alpha = self.video_info(path)?.has_alpha;
        Ok(VideoBitmap {
            data,
            width,
            height,
            frame_cache_hit,
            has_alpha,
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

    /// 内存里的一段已编码图片（如 GLB 内嵌的 `bufferView` 纹理）→ RGBA 位图。
    /// 与 [`Self::get_bitmap`] 的 `StaticImage` 分支同一条解码路径（image
    /// crate；skia codec 缺 webp），只是来源是字节而不是文件。
    pub fn decode_encoded_bitmap(&mut self, bytes: &[u8]) -> Result<(Arc<Vec<u8>>, u32, u32)> {
        let image = Image::from_encoded(Data::new_copy(bytes))
            .ok_or_else(|| anyhow!("failed to decode embedded image bytes"))?;
        let width = image.width() as u32;
        let height = image.height() as u32;
        let mut pixels = vec![0_u8; width as usize * height as usize * 4];
        let info = ImageInfo::new(
            (width as i32, height as i32),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        );
        if !image.read_pixels(&info, pixels.as_mut_slice(), width as usize * 4, (0, 0), CachingHint::Allow) {
            anyhow::bail!("failed to read decoded embedded image pixels");
        }
        Ok((Arc::new(pixels), width, height))
    }

    pub fn frame_rgba_at_time_by_path(
        &mut self,
        path: &Path,
        time_secs: f64,
    ) -> Result<VideoBitmap> {
        self.frame_rgba_at_time_sized(path, time_secs, None)
            .map(|(bitmap, _)| bitmap)
    }

    /// Like [`Self::frame_rgba_at_time_by_path`] but decodes down to `target`
    /// (16-pixel bucket, shrink-only). Returns the bitmap plus the horizontal
    /// decode scale (`bitmap.width / source_width`) so callers can map source
    /// rects onto the decoded pixels.
    pub fn frame_rgba_at_time_sized(
        &mut self,
        path: &Path,
        time_secs: f64,
        target: Option<(u32, u32)>,
    ) -> Result<(VideoBitmap, f32)> {
        let request = VideoFrameRequest {
            composition_time_secs: time_secs.max(0.0),
            timing: VideoFrameTiming::default(),
            quality: self.video_preview_quality,
            target_size: target,
        };
        let bitmap = self.get_video_bitmap(path, request)?;
        let source_width = self.video_info(path)?.width as f32;
        let scale = bitmap.width as f32 / source_width;
        Ok((bitmap, scale))
    }

    /// Static image for drawing, decoded down to `target` (shrink-only, 16-pixel
    /// bucket). Returns the image plus the horizontal decode scale
    /// (`image.width() / source_width`); `1.0` means native/lazy (no shrink).
    ///
    /// The lazy encoded image (file bytes only) is cached per path; shrunk
    /// rasters are cached per bucket so full-resolution RGBA pixels are never
    /// resident across frames.
    pub fn static_image_scaled(
        &mut self,
        path: &Path,
        target: Option<(u32, u32)>,
    ) -> Result<(Image, f32)> {
        let lazy = match self.static_images.get(path) {
            Some(image) => image.clone(),
            None => {
                let encoded = fs::read(path)
                    .with_context(|| format!("failed to read image bytes: {}", path.display()))?;
                let image = Image::from_encoded(Data::new_copy(&encoded))
                    .ok_or_else(|| anyhow!("failed to decode image: {}", path.display()))?;
                self.static_images.insert(path.to_path_buf(), image.clone());
                image
            }
        };
        let (w0, h0) = (lazy.width() as u32, lazy.height() as u32);
        let Some((bw, bh)) = target.and_then(|(tw, th)| quantize_bucket(tw, th, w0, h0)) else {
            return Ok((lazy, 1.0));
        };

        let key = (path.to_path_buf(), bw, bh);
        if let Some(cached) = self.scaled_images.get(&key) {
            return Ok((cached.clone(), bw as f32 / w0 as f32));
        }
        let mut surface = surfaces::raster_n32_premul((bw as i32, bh as i32))
            .ok_or_else(|| anyhow!("failed to allocate {}x{} surface for {}", bw, bh, path.display()))?;
        let sampling = SamplingOptions {
            use_cubic: true,
            cubic: CubicResampler::mitchell(),
            ..SamplingOptions::default()
        };
        surface.canvas().draw_image_rect_with_sampling_options(
            lazy,
            None,
            Rect::new(0.0, 0.0, bw as f32, bh as f32),
            sampling,
            &Paint::default(),
        );
        let shrunk = surface.image_snapshot();
        self.scaled_images.insert(key, shrunk.clone());
        Ok((shrunk, bw as f32 / w0 as f32))
    }

    /// Decode a frame-embedded encoded image (`ImageRef::Encoded`) at natural
    /// size. No pre-shrink: svg `<image>` draws resample once from natural
    /// pixels to device pixels through the CTM, so any intermediate raster
    /// would add a second resample.
    ///
    /// Decoder is `image` (the skia codec lacks webp and would resample JPEGs
    /// differently). Pixels are premultiplied for skia's RGBA8888 premul
    /// layout.
    pub fn encoded_image(
        &mut self,
        key: opencat_core::ir::draw_types::BytesRangeId,
        encoded: &[u8],
    ) -> Result<Image> {
        if let Some(image) = self.encoded_images.get(&key) {
            return Ok(image.clone());
        }
        let img = image::load_from_memory(encoded).map_err(|e| {
            anyhow!("failed to decode embedded image ({} bytes): {e}", encoded.len())
        })?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut pix = vec![0_u8; (w as usize) * (h as usize) * 4];
        for (p, d) in rgba.pixels().zip(pix.chunks_exact_mut(4)) {
            let a = p.0[3] as f64 / 255.0;
            d[0] = (p.0[0] as f64 * a + 0.5) as u8;
            d[1] = (p.0[1] as f64 * a + 0.5) as u8;
            d[2] = (p.0[2] as f64 * a + 0.5) as u8;
            d[3] = p.0[3];
        }
        let info = ImageInfo::new(
            (w as i32, h as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let image = images::raster_from_data(&info, Data::new_copy(&pix), w as usize * 4)
            .ok_or_else(|| anyhow!("failed to rasterize embedded image {}x{}", w, h))?;
        self.encoded_images.insert(key, image.clone());
        Ok(image)
    }
}

impl Default for MediaContext {
    fn default() -> Self {
        Self::new()
    }
}

/// skia 解不了的格式（webp 等）用 `image` crate 解码后重编码成 PNG 字节再
/// 交给 skia —— 两条路径最终都产 `skia_safe::Image`，色彩空间与预乘一致。
fn decode_with_image_crate(encoded: &[u8]) -> Option<Image> {
    let img = image::load_from_memory(encoded).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let info = skia_safe::ImageInfo::new(
        (w as i32, h as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let data = skia_safe::Data::new_copy(img.as_raw());
    skia_safe::images::raster_from_data(&info, data, (w as usize) * 4)
}

fn load_image_bitmap(path: &Path) -> Result<(Arc<Vec<u8>>, u32, u32)> {
    let encoded = fs::read(path)
        .with_context(|| format!("failed to read image bytes: {}", path.display()))?;
    let image = Image::from_encoded(Data::new_copy(&encoded))
        // skia 的编码器不含 webp（GLB 内嵌纹理常见 image/webp），回退到
        // `image` crate 解码后走 `Image::from_raster` 同一路径。
        .or_else(|| decode_with_image_crate(&encoded))
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
            has_alpha: false,
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
            has_alpha: false,
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
    fn quantize_target_size_clamps_to_source_resolution() {
        let info = VideoInfo {
            width: 100,
            height: 100,
            duration_secs: None,
            has_alpha: false,
        };
        // Above source on one axis but below on the other -> still scale, clamped
        assert_eq!(
            quantize_target_size(Some((50, 200)), &info),
            Some((64, 100))
        );
    }
}
