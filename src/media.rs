use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo, image::CachingHint};

use crate::cache_policy::{BitmapSourceKind, bitmap_source_kind};
use crate::codec::decode::VideoDecodeCache;

pub use crate::codec::decode::VideoInfo;

pub struct MediaContext {
    videos: VideoDecodeCache,
    images: HashMap<PathBuf, (Arc<Vec<u8>>, u32, u32)>,
}

impl MediaContext {
    pub fn new() -> Self {
        Self {
            videos: VideoDecodeCache::new(),
            images: HashMap::new(),
        }
    }

    pub fn get_video_frame(&mut self, path: &Path, target_time_secs: f64) -> Result<Arc<Vec<u8>>> {
        self.videos.get_frame(path, target_time_secs)
    }

    pub fn video_info(&mut self, path: &Path) -> Result<VideoInfo> {
        self.videos.info(path)
    }

    pub fn get_bitmap(
        &mut self,
        path: &Path,
        target_time_secs: f64,
    ) -> Result<(Arc<Vec<u8>>, u32, u32)> {
        match bitmap_source_kind(path) {
            BitmapSourceKind::Video => {
                let data = self.get_video_frame(path, target_time_secs)?;
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
