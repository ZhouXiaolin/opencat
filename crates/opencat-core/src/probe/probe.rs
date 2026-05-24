use anyhow::{Context, Result, anyhow};
use nom_exif::{EntryValue, MediaParser, MediaSource, TrackInfoTag};

use super::catalog::{ImageMeta, VideoInfoMeta};

pub fn probe_image(bytes: &[u8]) -> Result<ImageMeta> {
    let dims = imagesize::blob_size(bytes).context("imagesize: failed to read image dimensions")?;
    Ok(ImageMeta {
        width: dims.width as u32,
        height: dims.height as u32,
    })
}

pub fn probe_video(bytes: &[u8]) -> Result<VideoInfoMeta> {
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
    let duration_ms = info.get(TrackInfoTag::DurationMs).and_then(entry_u64);

    Ok(VideoInfoMeta {
        width,
        height,
        duration_ms,
    })
}

pub(crate) fn parse_srt_bytes(
    bytes: &[u8],
    fps: u32,
) -> Result<Vec<crate::parse::primitives::SrtEntry>> {
    let text = std::str::from_utf8(bytes).context("srt: not valid utf-8")?;
    crate::parse::primitives::parse_srt(text, fps)
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
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn probe_image_reads_png() {
        let meta = probe_image(PNG_1X1).expect("png dims");
        assert_eq!(meta.width, 1);
        assert_eq!(meta.height, 1);
    }
}
