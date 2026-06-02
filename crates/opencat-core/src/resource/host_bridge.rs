//! Host integration notes (engine / web) for the unified [`ResourceProvider`] protocol.
//!
//! ## Web (CanvasKit)
//!
//! 1. `collect_external_manifest` → list of fetches by `ExternalResourceEntry::source_label`
//! 2. Fetch bytes into `HashMap<AssetId, Vec<u8>>`
//! 3. `hydrate_provider_from_bytes` → [`MapResourceProvider`]
//! 4. For each `LottieBundleSpec`:
//!    - `bundle_primary_json` → JSON string
//!    - `skottie_assets_for_bundle` → `Record<string, ArrayBuffer>` for `MakeManagedAnimation`
//! 5. Flat images: still `inject_image_bytes` **or** read via `provider.load("opencat", asset_id)`
//!
//! ## Engine (skia-safe + skottie feature)
//!
//! Implement `skia_safe::resources::ResourceProvider` by delegating to
//! [`MapResourceProvider`] / [`IndexedResourceProvider`]:
//!
//! ```ignore
//! impl skia_safe::resources::ResourceProvider for OpenCatSkottieProvider {
//!     fn load(&self, path: &str, name: &str) -> Option<Data> {
//!         self.inner.load(path, name).map(|b| Data::new_copy(&b))
//!     }
//!     fn load_typeface(&self, name: &str, url: &str) -> Option<Typeface> {
//!         ...
//!     }
//!     fn font_mgr(&self) -> FontMgr { self.font_mgr.clone() }
//! }
//! ```
//!
//! Then `skottie::Builder::new().set_resource_provider(provider).make(json)`.

use crate::resource::manifest::ExternalResourceManifest;
use crate::resource::materialize::{ByteSource, hydrate_provider_from_bytes};
use crate::resource::protocol::MapResourceProvider;

/// Convenience: manifest + byte map → provider ready for Skottie / CanvasKit.
pub fn provider_from_manifest(
    manifest: &ExternalResourceManifest,
    sources: &impl ByteSource,
) -> MapResourceProvider {
    let mut provider = MapResourceProvider::new();
    hydrate_provider_from_bytes(manifest, &mut provider, sources);
    provider
}