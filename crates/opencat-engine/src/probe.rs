//! Host-owned resource probing (issue #40).
//!
//! These functions replace the old core `probe_image` / `probe_video` /
//! `parse_lottie_meta` that have been removed from `opencat-core`. The engine
//! probes media bytes directly and feeds metadata into
//! [`opencat_core::lifecycle::HostInputs::insert_image`] and friends.

use anyhow::{Context, Result, anyhow};
use nom_exif::{EntryValue, MediaParser, MediaSource, TrackInfoTag};
use serde::Deserialize;

use opencat_core::probe::{ImageMeta, VideoInfoMeta};

/// Internal Lottie asset struct for dependency scanning.
#[derive(Deserialize)]
struct LottieAsset {
    #[serde(default)]
    p: Option<String>,
    #[serde(default)]
    u: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    e: Option<String>,
}

/// Probe image dimensions from raw bytes.
pub fn probe_image(bytes: &[u8]) -> Result<ImageMeta> {
    let dims = imagesize::blob_size(bytes)
        .context("imagesize: failed to read image dimensions")?;
    Ok(ImageMeta {
        width: dims.width as u32,
        height: dims.height as u32,
    })
}

/// Probe video metadata from raw bytes.
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
    let duration_micros = duration_ms.map(opencat_core::time::ms_to_duration_micros);

    Ok(VideoInfoMeta {
        width,
        height,
        duration_micros,
    })
}

/// Parse Lottie JSON metadata (width/height/fps/frame range/dependencies).
pub fn parse_lottie_meta(json: &str) -> Result<opencat_core::lottie::LottieMeta> {
    #[derive(Deserialize)]
    struct LottieRoot {
        #[serde(default)]
        w: Option<f64>,
        #[serde(default)]
        h: Option<f64>,
        #[serde(default)]
        fr: Option<f64>,
        #[serde(default)]
        ip: Option<f64>,
        #[serde(default)]
        op: Option<f64>,
        #[serde(default)]
        assets: Vec<LottieAsset>,
    }

    let root: LottieRoot = serde_json::from_str(json).context("parse lottie json for meta")?;
    let dependencies = external_dependency_names(&root.assets);
    Ok(opencat_core::lottie::LottieMeta {
        width: root.w.unwrap_or(0.0).round().max(0.0) as u32,
        height: root.h.unwrap_or(0.0).round().max(0.0) as u32,
        fps: root.fr.unwrap_or(0.0) as f32,
        in_frame: root.ip.unwrap_or(0.0) as f32,
        out_frame: root.op.unwrap_or(0.0) as f32,
        dependencies,
    })
}

/// Scan Lottie JSON for external asset file names.
pub fn scan_lottie_dependencies(json: &str) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct LottieRoot {
        #[serde(default)]
        assets: Vec<LottieAsset>,
    }

    let root: LottieRoot =
        serde_json::from_str(json).context("parse lottie json for asset scan")?;
    Ok(external_dependency_names(&root.assets))
}

fn external_dependency_names(assets: &[LottieAsset]) -> Vec<String> {
    let mut names = Vec::new();
    for asset in assets {
        let p = asset.p.as_deref().unwrap_or("");
        if p.starts_with("data:") {
            continue;
        }
        let candidate = if !p.is_empty() {
            Some(p)
        } else {
            asset.u.as_deref().filter(|u| !u.is_empty())
        };
        let Some(raw) = candidate else {
            continue;
        };
        let name = raw.rsplit('/').next().unwrap_or(raw).to_string();
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
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

    #[test]
    fn parse_lottie_meta_reads_dimensions_and_frame_range() {
        let json = r#"{"w":280,"h":200,"fr":25,"ip":0,"op":32,"assets":[
            {"p":"data:image/png;base64,AAAA"},
            {"u":"images/","p":"photo.png"}
        ]}"#;
        let meta = parse_lottie_meta(json).unwrap();
        assert_eq!(meta.width, 280);
        assert_eq!(meta.height, 200);
        assert_eq!(meta.fps, 25.0);
        assert_eq!(meta.duration_frames(), 32);
        assert_eq!(meta.dependencies, vec!["photo.png"]);
    }

    #[test]
    fn scan_finds_external_u_names() {
        let json = r#"{
          "assets": [
            { "p": "data:image/png;base64,AAAA" },
            { "u": "images/photo.png" }
          ]
        }"#;
        let deps = scan_lottie_dependencies(json).unwrap();
        assert_eq!(deps, vec!["photo.png"]);
    }
}
