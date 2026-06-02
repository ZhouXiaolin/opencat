//! Materialize [`ExternalResourceManifest`] into a [`MapResourceProvider`].
//!
//! Hosts implement fetch; this module only defines the target shape and helpers to
//! populate the provider map after bytes are available.

use crate::ir::asset_id::AssetId;
use crate::resource::manifest::{
    BundleDependencySource, ExternalResourceManifest, ExternalResourceKind, LottiePrimarySource,
    ProviderBinding,
};
use crate::resource::protocol::{MapResourceProvider, ResourceLookup, ResourceProvider};

/// Copy all resolved bytes from `sources` into `provider` according to manifest bindings.
pub fn hydrate_provider_from_bytes(
    manifest: &ExternalResourceManifest,
    provider: &mut MapResourceProvider,
    sources: &impl ByteSource,
) {
    for entry in &manifest.entries {
        match &entry.binding {
            ProviderBinding::Flat { asset_id } => {
                if let Some(bytes) = sources.bytes_for(asset_id) {
                    provider.insert_opencat(asset_id, bytes);
                }
            }
            ProviderBinding::BundleDependency {
                bundle_id,
                file_name,
            } => {
                let dep_id = AssetId(format!("{}:dep:{}", bundle_id.0, file_name));
                if let Some(bytes) = sources.bytes_for(&dep_id).or_else(|| {
                    sources.bytes_for(&AssetId(format!("{}:{file_name}", bundle_id.0)))
                }) {
                    provider.insert_bundle_dep(bundle_id, file_name, bytes);
                }
            }
            ProviderBinding::BundleRoot { .. } | ProviderBinding::Typeface { .. } => {}
        }
    }

    for face_binding in manifest.entries.iter().filter_map(|e| {
        if let ProviderBinding::Typeface { name, url, asset_id } = &e.binding {
            Some((name, url, asset_id))
        } else {
            None
        }
    }) {
        if let Some(bytes) = sources.bytes_for(face_binding.2) {
            provider.insert_typeface(face_binding.0, face_binding.1, bytes);
        }
    }
}

/// Trait for post-fetch byte maps (engine loader, web blob store, etc.).
pub trait ByteSource {
    fn bytes_for(&self, id: &AssetId) -> Option<Vec<u8>>;
}

impl ByteSource for std::collections::HashMap<AssetId, Vec<u8>> {
    fn bytes_for(&self, id: &AssetId) -> Option<Vec<u8>> {
        self.get(id).cloned()
    }
}

/// Primary JSON bytes for a registered bundle.
pub fn bundle_primary_json(
    bundle: &crate::resource::manifest::LottieBundleSpec,
    sources: &impl ByteSource,
) -> Option<String> {
    match &bundle.primary {
        LottiePrimarySource::InlineJson(s) => Some(s.clone()),
        LottiePrimarySource::Url(_) | LottiePrimarySource::Path(_) => {
            let bytes = sources.bytes_for(&bundle.bundle_id)?;
            String::from_utf8(bytes).ok()
        }
    }
}

/// Full Skottie-facing asset map for one bundle (web `MakeManagedAnimation` arg 2).
pub fn skottie_assets_for_bundle(
    bundle_id: &AssetId,
    provider: &MapResourceProvider,
) -> std::collections::HashMap<String, Vec<u8>> {
    provider.skottie_assets_for_bundle(bundle_id)
}

/// Resolve a bundle dependency through a flat OpenCat image if declared that way.
pub fn map_bundle_dep_to_flat_lookup(
    dep: &crate::resource::manifest::BundleDependencySpec,
) -> Option<ResourceLookup> {
    match &dep.source {
        BundleDependencySource::OpenCatImage(img) => {
            crate::resource::manifest::image_asset_id_and_label(img).map(|(id, _)| {
                ResourceLookup::opencat_flat(&id)
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::resource::manifest::{
        BundleDependencySpec, ExternalResourceManifest, LottieBundleSpec,
    };
    use crate::resource::protocol::ResourceProvider;

    #[test]
    fn hydrate_fills_skottie_asset_map() {
        let bundle_id = AssetId("lottie:hero".into());
        let mut manifest = ExternalResourceManifest::default();
        manifest.push_lottie_bundle(LottieBundleSpec {
            bundle_id: bundle_id.clone(),
            primary: LottiePrimarySource::InlineJson("{}".into()),
            dependencies: vec![BundleDependencySpec {
                file_name: "a.png".into(),
                source: BundleDependencySource::Url("a.png".into()),
            }],
        });

        let mut bytes = HashMap::new();
        bytes.insert(
            AssetId("lottie:hero:dep:a.png".into()),
            b"PNG".to_vec(),
        );

        let mut provider = MapResourceProvider::new();
        hydrate_provider_from_bytes(&manifest, &mut provider, &bytes);

        let assets = skottie_assets_for_bundle(&bundle_id, &provider);
        assert_eq!(assets.get("a.png").map(|v| v.as_slice()), Some(b"PNG".as_ref()));
    }
}