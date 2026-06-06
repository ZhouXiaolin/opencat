//! Thread-local [`MapResourceProvider`] built after [`super::wasm_api::preload_assets`].

use std::cell::RefCell;

use opencat_core::resource::manifest::ExternalResourceManifest;
use opencat_core::resource::materialize::{CoreBlobStoreSource, hydrate_provider_from_bytes};
use opencat_core::resource::protocol::{MapResourceProvider, ResourceProvider};

use super::blob_store::BlobStore;

thread_local! {
    static PROVIDER: RefCell<Option<MapResourceProvider>> = RefCell::new(None);
}

pub fn set(provider: MapResourceProvider) {
    PROVIDER.with(|p| *p.borrow_mut() = Some(provider));
}

pub fn clear() {
    PROVIDER.with(|p| *p.borrow_mut() = None);
}

pub fn with<R>(f: impl FnOnce(&MapResourceProvider) -> R) -> Option<R> {
    PROVIDER.with(|p| p.borrow().as_ref().map(f))
}

/// Rebuild Skottie-compatible provider from manifest + blob store bytes.
pub fn rebuild(manifest: &ExternalResourceManifest, blobs: &BlobStore) {
    let mut provider = MapResourceProvider::new();
    hydrate_provider_from_bytes(manifest, &mut provider, &CoreBlobStoreSource(blobs));
    set(provider);
}

/// CanvasKit `MakeManagedAnimation` assets argument for a bundle id.
pub fn skottie_assets(bundle_id: &str) -> Option<std::collections::HashMap<String, Vec<u8>>> {
    with(|p| {
        let id = opencat_core::resource::asset_id::AssetId(bundle_id.to_string());
        let map = p.skottie_assets_for_bundle(&id);
        if map.is_empty() { None } else { Some(map) }
    })
    .flatten()
}

/// Load bytes via unified `(path, name)` protocol (debug / future hosts).
pub fn load(path: &str, name: &str) -> Option<Vec<u8>> {
    with(|p| p.load(path, name).map(|c| c.into_owned())).flatten()
}
