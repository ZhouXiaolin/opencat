//! Preload Lottie bundle primary JSON + discovered dependencies into a byte map.

use std::pin::Pin;

use anyhow::{Context, Result};

use crate::ir::asset_id::AssetId;
use crate::resource::lottie::parse_lottie_meta;
use crate::resource::manifest::{
    BundleDependencySource, ExternalResourceManifest, LottiePrimarySource,
};
use crate::resource::materialize::ByteSource;

type FetchFuture = Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>>>>>;

/// Fetch primary JSON and dependency bytes for every bundle in `manifest`.
pub async fn hydrate_lottie_bundles(
    manifest: &mut ExternalResourceManifest,
    bytes: &mut impl ByteSourceMap,
    mut fetch: impl FnMut(&str) -> FetchFuture,
) -> Result<()> {
    let bundle_ids: Vec<AssetId> = manifest
        .bundles
        .iter()
        .map(|b| b.bundle_id.clone())
        .collect();
    for bundle_id in bundle_ids {
        let Some(bundle) = manifest.bundles.iter().find(|b| b.bundle_id == bundle_id) else {
            continue;
        };
        let primary_bytes = match &bundle.primary {
            LottiePrimarySource::InlineJson(s) => s.as_bytes().to_vec(),
            LottiePrimarySource::Path(p) => fetch(&p.to_string_lossy()).await?,
            LottiePrimarySource::Url(u) => fetch(u).await?,
        };
        bytes.insert(&bundle_id, primary_bytes.clone());
        let json = String::from_utf8(primary_bytes).context("lottie primary json utf-8")?;
        manifest.discover_lottie_dependencies_from_json(&bundle_id, &json)?;
        let _meta = parse_lottie_meta(&json)?;
    }

    let deps: Vec<(AssetId, String, String)> = manifest
        .bundles
        .iter()
        .flat_map(|b| {
            b.dependencies.iter().map(move |d| {
                (
                    b.bundle_id.clone(),
                    d.file_name.clone(),
                    match &d.source {
                        BundleDependencySource::Url(u) => u.clone(),
                        BundleDependencySource::InlineDataUri(s) => s.clone(),
                        BundleDependencySource::OpenCatImage(_) => String::new(),
                    },
                )
            })
        })
        .collect();

    for (bundle_id, file_name, url) in deps {
        if url.is_empty() {
            continue;
        }
        let dep_id = AssetId(format!("{}:dep:{}", bundle_id.0, file_name));
        if bytes.get(&dep_id).is_some() {
            continue;
        }
        let raw = fetch(&url).await?;
        bytes.insert_bundle_dep(&bundle_id, &file_name, raw);
    }
    Ok(())
}

/// Mutable byte map used during preload (blob store adapter).
pub trait ByteSourceMap {
    fn insert(&mut self, id: &AssetId, bytes: Vec<u8>);
    fn get(&self, id: &AssetId) -> Option<Vec<u8>>;
    fn insert_bundle_dep(&mut self, bundle_id: &AssetId, file_name: &str, bytes: Vec<u8>);
}

impl ByteSourceMap for std::collections::HashMap<AssetId, Vec<u8>> {
    fn insert(&mut self, id: &AssetId, bytes: Vec<u8>) {
        self.insert(id.clone(), bytes);
    }
    fn get(&self, id: &AssetId) -> Option<Vec<u8>> {
        self.get(id).cloned()
    }
    fn insert_bundle_dep(&mut self, bundle_id: &AssetId, file_name: &str, bytes: Vec<u8>) {
        let dep_id = AssetId(format!("{}:dep:{}", bundle_id.0, file_name));
        self.insert(dep_id, bytes);
    }
}
