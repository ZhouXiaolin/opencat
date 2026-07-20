//! Skottie-compatible resource provider protocol for OpenCat.
//!
//! Skia's [`skresources::ResourceProvider`](https://skia.org/docs/user/modules/skottie/)
//! resolves external Lottie dependencies via:
//!
//! ```text
//! load(resource_path, resource_name) -> bytes
//! load_typeface(name, url) -> font bytes
//! ```
//!
//! OpenCat uses the same *lookup shape* for **all** external assets (images, audio,
//! fonts, Lottie bundle deps) so engine (`skottie::Builder`) and web
//! (`CanvasKit.MakeManagedAnimation`) can share one [`ResourceProvider`] implementation
//! backed by the same manifest + byte store.
//!
//! ## Lookup conventions
//!
//! | Use case | `resource_path` | `resource_name` |
//! |----------|-----------------|-----------------|
//! | Flat OpenCat asset (image/video/audio/subtitle) | `"opencat"` | [`AssetId`] string |
//! | Lottie bundle dependency | bundle [`AssetId`] | filename from JSON (`image_0.png`) |
//! | Inline / data URI (Skottie) | `""` | full `data:...;base64,...` string |
//! | HTTP(S) flat fetch (Skottie-style) | directory prefix | file name or URL |

use std::borrow::Cow;
use std::collections::HashMap;

use crate::ir::asset_id::AssetId;

/// Skottie-compatible resource key (matches C++ `ResourceProvider` callback args).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceLookup {
    pub path: String,
    pub name: String,
}

impl ResourceLookup {
    pub fn new(path: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
        }
    }

    /// Flat OpenCat asset keyed by stable [`AssetId`].
    pub fn opencat_flat(id: &AssetId) -> Self {
        Self::new("opencat", id.0.clone())
    }

    /// Named file inside a Lottie / resource bundle.
    pub fn bundle_dep(bundle_id: &AssetId, file_name: &str) -> Self {
        Self::new(bundle_id.0.clone(), file_name.to_string())
    }

    /// Data-URI style lookup (Skottie: empty path, name is the full data URL).
    pub fn data_uri(data_url: &str) -> Self {
        Self::new("", data_url.to_string())
    }
}

/// Typeface resolution request (Skottie `loadTypeface(name, url)`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypefaceRequest {
    pub name: String,
    pub url: String,
}

/// Host-agnostic resource resolver — same contract as Skia Skottie / CanvasKit WASM.
pub trait ResourceProvider {
    /// Load raw bytes for a resource reference embedded in Lottie JSON or OpenCat markup.
    fn load(&self, path: &str, name: &str) -> Option<Cow<'_, [u8]>>;

    /// Load font bytes for a family reference in Lottie / SVG.
    fn load_typeface(&self, name: &str, url: &str) -> Option<Cow<'_, [u8]>> {
        let _ = (name, url);
        None
    }
}

/// In-memory provider used in tests and as the web/engine interchange format.
#[derive(Debug, Default, Clone)]
pub struct MapResourceProvider {
    blobs: HashMap<ResourceLookup, Vec<u8>>,
    typefaces: HashMap<TypefaceRequest, Vec<u8>>,
}

impl MapResourceProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, lookup: ResourceLookup, bytes: Vec<u8>) {
        self.blobs.insert(lookup, bytes);
    }

    pub fn insert_opencat(&mut self, id: &AssetId, bytes: Vec<u8>) {
        self.insert(ResourceLookup::opencat_flat(id), bytes);
    }

    pub fn insert_bundle_dep(&mut self, bundle_id: &AssetId, file_name: &str, bytes: Vec<u8>) {
        self.insert(ResourceLookup::bundle_dep(bundle_id, file_name), bytes);
    }

    pub fn insert_typeface(&mut self, name: &str, url: &str, bytes: Vec<u8>) {
        self.typefaces.insert(
            TypefaceRequest {
                name: name.to_string(),
                url: url.to_string(),
            },
            bytes,
        );
    }

    /// Build CanvasKit `MakeManagedAnimation(json, assets)` dictionary for one bundle.
    pub fn skottie_assets_for_bundle(&self, bundle_id: &AssetId) -> HashMap<String, Vec<u8>> {
        let prefix = format!("{}/", bundle_id.0);
        let mut out = HashMap::new();
        for (lookup, bytes) in &self.blobs {
            if lookup.path == bundle_id.0 {
                out.insert(lookup.name.clone(), bytes.clone());
            } else if lookup.path.starts_with(&prefix) {
                // tolerate path with trailing structure
                out.insert(lookup.name.clone(), bytes.clone());
            }
        }
        out
    }
}

impl ResourceProvider for MapResourceProvider {
    fn load(&self, path: &str, name: &str) -> Option<Cow<'_, [u8]>> {
        self.blobs
            .get(&ResourceLookup::new(path, name))
            .map(|b| Cow::Borrowed(b.as_slice()))
    }

    fn load_typeface(&self, name: &str, url: &str) -> Option<Cow<'_, [u8]>> {
        self.typefaces
            .get(&TypefaceRequest {
                name: name.to_string(),
                url: url.to_string(),
            })
            .map(|b| Cow::Borrowed(b.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_provider_roundtrips_opencat_and_bundle_keys() {
        let mut p = MapResourceProvider::new();
        let flat = AssetId("url:https://example.com/a.png".into());
        let bundle = AssetId("lottie:hero".into());
        p.insert_opencat(&flat, b"png".to_vec());
        p.insert_bundle_dep(&bundle, "img_0.png", b"dep".to_vec());

        assert_eq!(p.load("opencat", &flat.0).unwrap().as_ref(), b"png");
        assert_eq!(p.load(&bundle.0, "img_0.png").unwrap().as_ref(), b"dep");

        let assets = p.skottie_assets_for_bundle(&bundle);
        assert_eq!(
            assets.get("img_0.png").map(|v| v.as_slice()),
            Some(b"dep".as_ref())
        );
    }
}
