//! `#[wasm_bindgen]` 暴露给 JS 的资源预加载 API。
//!
//! - [`preload_assets`]: 解析 composition → [`collect_external_manifest`] → 下载 →
//!   填充 `BlobStore` + 构建 Skottie 对齐的 [`MapResourceProvider`]
//! - [`get_blob_bytes`]: 按 `AssetId` 取字节（扁平资源 / Draw IR）
//! - [`get_skottie_bundle_assets`]: 按 bundle id 取 Lottie 依赖 map（CanvasKit）
//! - [`load_resource_bytes`]: 按 `(path, name)` 协议取字节

use js_sys::{Function, Uint8Array};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use opencat_core::parse::composition::Composition;
use opencat_core::parse::preflight::collect_external_manifest;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::fonts::{FontSource, font_asset_id};
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::resource::preload::preload_all;
use opencat_core::resource::resolver::UrlFetcher;

use crate::resource::blob_store::BlobStore;
use crate::resource::resolver::WebAssetResolver;

thread_local! {
    static BLOB_STORE: RefCell<BlobStore> = RefCell::new(BlobStore::new());
    static EXTERNAL_MANIFEST: RefCell<Option<opencat_core::resource::ExternalResourceManifest>> =
        RefCell::new(None);
}

fn take_blobs() -> BlobStore {
    BLOB_STORE.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn put_blobs(blobs: BlobStore) {
    BLOB_STORE.with(|s| *s.borrow_mut() = blobs);
}

fn font_manifest_from_source(
    source: &str,
) -> anyhow::Result<opencat_core::resource::fonts::FontManifest> {
    let trimmed = source.trim();
    if trimmed.starts_with('{') {
        Ok(opencat_core::parse::jsonl::parse_with_base_dir(source, None)?.font_manifest)
    } else {
        Ok(opencat_core::parse::markup::parse_parts_with_base_dir(source, None)?.font_manifest)
    }
}

/// 下载 composition source 引用的全部资源，把字节放进 BlobStore，返回 catalog JSON。
#[wasm_bindgen]
pub async fn preload_assets(source: &str) -> Result<String, JsValue> {
    crate::resource::provider_store::clear();
    EXTERNAL_MANIFEST.with(|m| *m.borrow_mut() = None);

    let font_manifest = font_manifest_from_source(source)
        .map_err(|e| JsValue::from_str(&format!("preload_assets: parse failed: {e}")))?;

    let parsed = crate::source::parse_source(source, &fontdb::Database::new())
        .map_err(|e| JsValue::from_str(&format!("preload_assets: parse failed: {e}")))?;
    let root_node = parsed.root.clone();
    let composition = Composition::new("preload")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .duration(parsed.duration)
        .root(move |_ctx| root_node.clone())
        .audio_sources(parsed.audio_sources)
        .build()
        .map_err(|e| JsValue::from_str(&format!("preload_assets: build composition: {e}")))?;

    let (requests, mut external_manifest) = collect_external_manifest(&composition, &font_manifest);

    let mut blobs = take_blobs();

    // Fonts: same bytes in BlobStore (provider) + font_store (fontdb merge on load).
    if !font_manifest.is_empty() {
        let mut fetcher = crate::resource::resolver::WebFetcher;
        for face in &font_manifest.faces {
            let bytes = match &face.source {
                FontSource::Path(_) => {
                    return Err(JsValue::from_str(
                        "preload_assets: font path is not supported on web; use url",
                    ));
                }
                FontSource::Url(url) => {
                    let id = AssetId(font_asset_id(&FontSource::Url(url.clone())));
                    let raw = fetcher
                        .fetch_bytes(&id, url)
                        .await
                        .map_err(|e| JsValue::from_str(&format!("preload_assets font: {e}")))?;
                    blobs.insert(id, std::sync::Arc::from(raw.clone()));
                    crate::resource::font_store::insert(face.id.clone(), raw);
                }
            };
        }
    }

    let mut catalog = HashMapResourceCatalog::from_json("{}")
        .map_err(|e| JsValue::from_str(&format!("preload_assets: catalog init: {e}")))?;

    {
        let mut resolver = WebAssetResolver::new(&mut blobs);
        preload_all(requests, &mut resolver, &mut catalog)
            .await
            .map_err(|e| JsValue::from_str(&format!("preload_assets: {e}")))?;
    }

    {
        struct BlobLottieMap<'a>(&'a mut BlobStore);
        impl opencat_core::resource::preload_lottie::ByteSourceMap for BlobLottieMap<'_> {
            fn insert(&mut self, id: &AssetId, bytes: Vec<u8>) {
                self.0.insert(id.clone(), std::sync::Arc::from(bytes));
            }
            fn get(&self, id: &AssetId) -> Option<Vec<u8>> {
                self.0.get(id).map(|b| b.to_vec())
            }
            fn insert_bundle_dep(&mut self, bundle_id: &AssetId, file_name: &str, bytes: Vec<u8>) {
                let dep_id = AssetId(format!("{}:dep:{}", bundle_id.0, file_name));
                self.0.insert(dep_id, std::sync::Arc::from(bytes));
            }
        }

        opencat_core::resource::preload_lottie::hydrate_lottie_bundles(
            &mut external_manifest,
            &mut BlobLottieMap(&mut blobs),
            |url| {
                let resolved = if url.starts_with("http://") || url.starts_with("https://") {
                    url.to_string()
                } else if url.starts_with('/') {
                    url.to_string()
                } else {
                    format!("/assets/{url}")
                };
                Box::pin(async move { crate::resource::fetch::fetch_bytes(&resolved).await })
            },
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("preload_assets lottie: {e}")))?;

        for bundle in &external_manifest.bundles {
            if let Some(bytes) = blobs.get(&bundle.bundle_id) {
                if let Ok(json) = std::str::from_utf8(bytes.as_ref()) {
                    if let Ok(meta) = opencat_core::resource::parse_lottie_meta(json) {
                        catalog.register_lottie(&bundle.bundle_id.0, meta);
                    }
                }
            }
        }
    }

    put_blobs(blobs);
    BLOB_STORE.with(|s| {
        crate::resource::provider_store::rebuild(&external_manifest, &s.borrow());
    });
    EXTERNAL_MANIFEST.with(|m| *m.borrow_mut() = Some(external_manifest));

    catalog
        .to_json()
        .map_err(|e| JsValue::from_str(&format!("preload_assets: serialize: {e}")))
}

#[wasm_bindgen]
pub fn get_blob_bytes(asset_id: &str) -> Option<Uint8Array> {
    let id = AssetId(asset_id.to_string());
    BLOB_STORE.with(|s| {
        s.borrow()
            .get(&id)
            .map(|arc| Uint8Array::from(arc.as_ref()))
    })
}

/// Skottie / `MakeManagedAnimation` asset dictionary: `{ "image_0.png": Uint8Array, ... }`.
#[wasm_bindgen]
pub fn get_skottie_bundle_assets(bundle_id: &str) -> JsValue {
    let Some(map) = crate::resource::provider_store::skottie_assets(bundle_id) else {
        return js_sys::Object::new().into();
    };
    let obj = js_sys::Object::new();
    for (name, bytes) in map {
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str(&name),
            &Uint8Array::from(bytes.as_slice()),
        );
    }
    obj.into()
}

/// Unified resource protocol lookup (`path`, `name`) → bytes.
#[wasm_bindgen]
pub fn load_resource_bytes(path: &str, name: &str) -> Option<Uint8Array> {
    crate::resource::provider_store::load(path, name).map(|b| Uint8Array::from(b.as_slice()))
}

#[wasm_bindgen]
pub fn blob_count() -> usize {
    BLOB_STORE.with(|s| s.borrow().len())
}

#[wasm_bindgen]
pub fn clear_blobs() {
    BLOB_STORE.with(|s| s.borrow_mut().clear());
    crate::resource::font_store::clear();
    crate::resource::provider_store::clear();
    EXTERNAL_MANIFEST.with(|m| *m.borrow_mut() = None);
}

/// 注册宿主侧 VFS reader。
///
/// 传入的 JS 函数签名为 `(path: string) => Uint8Array | ArrayBuffer | number[] | Promise<...>`。
/// 配置后，web 端的 `path="..."` 资源会通过该函数读取 bytes。
#[wasm_bindgen]
pub fn set_asset_reader(reader: Function) {
    crate::resource::asset_reader::set_reader(reader);
}

/// 清除宿主侧 VFS reader。清除后 web 端 `path="..."` 资源会再次不可用。
#[wasm_bindgen]
pub fn clear_asset_reader() {
    crate::resource::asset_reader::clear_reader();
}
