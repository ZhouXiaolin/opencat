//! 平台无关的资源元数据探测 —— 从字节直接读图片/视频维度。
//!
//! - 图片：[`imagesize`] 解 PNG/JPEG/WebP/GIF/HEIC/AVIF/BMP/TIFF 头部，不解码像素。
//! - 视频：[`nom_exif`] 解 MP4/MOV `tkhd` 拿 width/height/duration。

use anyhow::{Context, Result, anyhow};
use nom_exif::{EntryValue, MediaParser, MediaSource, TrackInfoTag};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageDims {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoProbe {
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

pub fn probe_image_dims(bytes: &[u8]) -> Result<ImageDims> {
    let dims = imagesize::blob_size(bytes).context("imagesize: failed to read image dimensions")?;
    Ok(ImageDims {
        width: dims.width as u32,
        height: dims.height as u32,
    })
}

pub fn probe_video(bytes: &[u8]) -> Result<VideoProbe> {
    let ms = MediaSource::from_memory(bytes.to_vec())
        .context("nom-exif: failed to wrap bytes as MediaSource")?;
    let mut parser = MediaParser::new();
    let info = parser
        .parse_track(ms)
        .context("nom-exif: parse_track failed")?;

    let width = info
        .get(TrackInfoTag::Width)
        .and_then(entry_u32)
        .ok_or_else(|| anyhow!("video: width tag missing"))?;
    let height = info
        .get(TrackInfoTag::Height)
        .and_then(entry_u32)
        .ok_or_else(|| anyhow!("video: height tag missing"))?;
    let duration_secs = info
        .get(TrackInfoTag::DurationMs)
        .and_then(entry_u64)
        .map(|ms| ms as f64 / 1000.0);

    Ok(VideoProbe {
        width,
        height,
        duration_secs,
    })
}

fn entry_u32(v: &EntryValue) -> Option<u32> {
    match v {
        EntryValue::U32(n) => Some(*n),
        _ => None,
    }
}

fn entry_u64(v: &EntryValue) -> Option<u64> {
    match v {
        EntryValue::U64(n) => Some(*n),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89,
        0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54,
        0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
        0x0D, 0x0A, 0x2D, 0xB4,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
        0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn probe_image_dims_reads_png() {
        let dims = probe_image_dims(PNG_1X1).expect("png dims");
        assert_eq!(dims.width, 1);
        assert_eq!(dims.height, 1);
    }
}
