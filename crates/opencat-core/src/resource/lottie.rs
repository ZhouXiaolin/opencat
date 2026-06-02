//! Lottie JSON dependency discovery (Bodymovin `assets` array).

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LottieRoot {
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