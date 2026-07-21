//! Lottie JSON dependency discovery (Bodymovin `assets` array).

use anyhow::{Context, Result};
use serde::Deserialize;

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

/// Parse width/height/fps/in/out and external dependencies from Lottie JSON.
///
/// Zero or missing dimensions are preserved as 0 so prepare can fail-fast on
/// layout-critical metadata; callers must not mask missing size as 1×1.
pub fn parse_lottie_meta(json: &str) -> Result<LottieMeta> {
    let root: LottieRoot = serde_json::from_str(json).context("parse lottie json for meta")?;
    let dependencies = external_dependency_names(&root.assets);
    Ok(LottieMeta {
        width: root.w.unwrap_or(0.0).round().max(0.0) as u32,
        height: root.h.unwrap_or(0.0).round().max(0.0) as u32,
        fps: root.fr.unwrap_or(0.0) as f32,
        in_frame: root.ip.unwrap_or(0.0) as f32,
        out_frame: root.op.unwrap_or(0.0) as f32,
        dependencies,
    })
}

/// Scan a Lottie JSON string for external asset file names.
///
/// Returns dependency basenames (e.g. `image_0.png`). Data-URI assets (`p` only)
/// are omitted because they need no external bytes.
pub fn scan_lottie_dependencies(json: &str) -> Result<Vec<String>> {
    let root: LottieRoot =
        serde_json::from_str(json).context("parse lottie json for asset scan")?;
    Ok(external_dependency_names(&root.assets))
}

/// Collect external asset basenames from Bodymovin `assets`.
///
/// Standard layout: `u` is a directory prefix, `p` is the file name (or a
/// `data:` URI). Some exports put the full relative path in `u` alone. Data-URI
/// embeds need no host fetch and are omitted. Return values are basenames so
/// hosts can resolve them against their own document base (same contract as web
/// `{bundle}:dep:{basename}` keys).
fn external_dependency_names(assets: &[LottieAsset]) -> Vec<String> {
    let mut names = Vec::new();
    for asset in assets {
        let p = asset.p.as_deref().unwrap_or("");
        if p.starts_with("data:") {
            continue;
        }
        // Prefer `p` as the file name when it is a non-empty non-data path.
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
            dependencies: vec![],
        };
        let request = crate::media::VideoFrameRequest {
            composition_time_secs: 0.4,
            timing: crate::media::VideoFrameTiming {
                timeline_start_secs: 0.2,
                timeline_duration_secs: None,
                media_start_secs: 0.0,
                playback_rate: 1.0,
                looping: false,
            },
        };
        // visible: 0.4 >= 0.2, elapsed 0.2s -> frame 5
        let frame = resolve_lottie_frame(&request, &meta).unwrap();
        assert!((frame - 5.0).abs() < 0.01);

        let hidden = crate::media::VideoFrameRequest {
            composition_time_secs: 0.1,
            timing: request.timing,
        };
        assert!(resolve_lottie_frame(&hidden, &meta).is_none());
    }

    #[test]
    fn parse_meta_reads_w_h_fr_op_and_deps() {
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
    fn parse_meta_preserves_zero_dimensions_for_fail_fast() {
        let json = r#"{"w":0,"h":0,"fr":30,"ip":0,"op":10,"assets":[]}"#;
        let meta = parse_lottie_meta(json).unwrap();
        assert_eq!(meta.width, 0);
        assert_eq!(meta.height, 0);
    }

    #[test]
    fn scan_finds_external_u_names() {
        // Non-standard: full path only in `u` (legacy / some exporters).
        let json = r#"{
          "assets": [
            { "p": "data:image/png;base64,AAAA" },
            { "u": "images/photo.png" }
          ]
        }"#;
        let deps = scan_lottie_dependencies(json).unwrap();
        assert_eq!(deps, vec!["photo.png"]);
    }

    #[test]
    fn scan_standard_bodymovin_u_prefix_p_filename() {
        // Standard Bodymovin: `u` directory prefix + `p` file name.
        let json = r#"{
          "assets": [
            { "id": "image_0", "u": "images/", "p": "img_0.png" },
            { "id": "image_1", "u": "images/", "p": "data:image/png;base64,AAAA" }
          ]
        }"#;
        let deps = scan_lottie_dependencies(json).unwrap();
        assert_eq!(deps, vec!["img_0.png"]);
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
        // At t=0.5s with timeline start 0 → frame 5
        let request = crate::media::VideoFrameRequest {
            composition_time_secs: 0.5,
            timing: crate::media::VideoFrameTiming::default(),
        };
        let frame = resolve_lottie_frame(&request, &meta).unwrap();
        assert!((frame - 5.0).abs() < 0.01, "frame={frame}");
    }
}
