//! Lottie JSON dependency discovery (Bodymovin `assets` array).

use anyhow::{Context, Result};
use serde::Deserialize;

/// Intrinsic timing/size from a Bodymovin root object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LottieMeta {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub in_frame: f32,
    pub out_frame: f32,
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
    let info = crate::resource::catalog::VideoInfoMeta {
        width: meta.width,
        height: meta.height,
        duration_secs: Some(meta.duration_secs()),
    };
    let time_secs = request.resolve_time_secs(&info);
    let frame = meta.in_frame + time_secs as f32 * meta.fps;
    Some(frame.clamp(meta.in_frame, (meta.out_frame - 1.0).max(meta.in_frame)))
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct LottieAsset {
    /// Embedded data URI (`data:image/png;base64,...`)
    #[serde(default)]
    p: Option<String>,
    /// External file path / URL
    #[serde(default)]
    u: Option<String>,
    /// Relative path prefix (combined with `u` by Skottie)
    #[serde(default)]
    #[allow(dead_code)]
    e: Option<String>,
}

/// Parse width/height/fps/in/out from Lottie JSON.
pub fn parse_lottie_meta(json: &str) -> Result<LottieMeta> {
    let root: LottieRoot = serde_json::from_str(json).context("parse lottie json for meta")?;
    Ok(LottieMeta {
        width: root.w.unwrap_or(0.0).round().max(1.0) as u32,
        height: root.h.unwrap_or(0.0).round().max(1.0) as u32,
        fps: root.fr.unwrap_or(30.0) as f32,
        in_frame: root.ip.unwrap_or(0.0) as f32,
        out_frame: root.op.unwrap_or(1.0) as f32,
    })
}

/// Scan a Lottie JSON string for external asset file names.
///
/// Returns basenames suitable for [`super::protocol::ResourceLookup::bundle_dep`]
/// (e.g. `image_0.png`). Data-URI assets (`p` only) are omitted — they need no fetch.
pub fn scan_lottie_dependencies(json: &str) -> Result<Vec<String>> {
    let root: LottieRoot =
        serde_json::from_str(json).context("parse lottie json for asset scan")?;
    let mut names = Vec::new();
    for asset in root.assets {
        if asset.p.as_ref().is_some_and(|p| p.starts_with("data:")) {
            continue;
        }
        if let Some(u) = asset.u {
            let name = u
                .rsplit('/')
                .next()
                .unwrap_or(&u)
                .to_string();
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_frame_respects_data_start_and_media_start() {
        let meta = LottieMeta {
            width: 280,
            height: 200,
            fps: 25.0,
            in_frame: 0.0,
            out_frame: 32.0,
        };
        let request = crate::media::VideoFrameRequest {
            composition_time_secs: 0.4,
            timing: crate::resource::types::VideoFrameTiming {
                timeline_start_secs: 0.2,
                timeline_duration_secs: None,
                media_start_secs: 0.0,
                playback_rate: 1.0,
                looping: false,
            },
            quality: crate::media::VideoPreviewQuality::Exact,
            target_size: None,
        };
        // visible: 0.4 >= 0.2, elapsed 0.2s -> frame 5
        let frame = resolve_lottie_frame(&request, &meta).unwrap();
        assert!((frame - 5.0).abs() < 0.01);

        let hidden = crate::media::VideoFrameRequest {
            composition_time_secs: 0.1,
            timing: request.timing,
            quality: request.quality,
            target_size: None,
        };
        assert!(resolve_lottie_frame(&hidden, &meta).is_none());
    }

    #[test]
    fn parse_meta_reads_w_h_fr_op() {
        let json = r#"{"w":280,"h":200,"fr":25,"ip":0,"op":32,"assets":[]}"#;
        let meta = parse_lottie_meta(json).unwrap();
        assert_eq!(meta.width, 280);
        assert_eq!(meta.height, 200);
        assert_eq!(meta.fps, 25.0);
        assert_eq!(meta.duration_frames(), 32);
    }

    #[test]
    fn scan_finds_external_u_names() {
        let json = r#"{
          "assets": [
            { "p": "data:image/png;base64,AAAA" },
            { "u": "images/photo.png", "e": "images/" }
          ]
        }"#;
        let deps = scan_lottie_dependencies(json).unwrap();
        assert_eq!(deps, vec!["photo.png"]);
    }
}