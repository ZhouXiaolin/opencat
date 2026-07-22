//! `#[wasm_bindgen]` 暴露给 JS 的资源预加载 API。
//!
//! - [`preload_assets`]: 解析 composition → 收集资源请求 → 下载到 `BlobStore`
//! - [`get_blob_bytes`]: 按 `AssetId` 取字节（扁平资源 / Draw IR）
//! - [`get_skottie_bundle_assets`]: 按 bundle id 取 Lottie 依赖 map（CanvasKit）
//! - [`load_resource_bytes`]: 按 CanvasKit 的 `(path, name)` 形状取字节

use js_sys::{Function, Uint8Array};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use opencat_core::ir::asset_id::asset_id_for_subtitle;
use opencat_core::parse::preflight::collect_resource_requests_from_parsed;
use opencat_core::parse::primitives::{LottieSource, SubtitleSource};
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::fonts::{FontSource, font_asset_id};
use opencat_core::probe::catalog::PreparedResourceCatalog;

use crate::resource::blob_store::BlobStore;

thread_local! {
    static BLOB_STORE: RefCell<BlobStore> = RefCell::new(BlobStore::new());
}

fn take_blobs() -> BlobStore {
    BLOB_STORE.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn put_blobs(blobs: BlobStore) {
    BLOB_STORE.with(|s| *s.borrow_mut() = blobs);
}

/// Snapshot the thread-local `BlobStore` into an owned `(asset_id_string ->
/// bytes)` map, keyed by canonical `AssetId` string. This is the host-side
/// byte bridge for the host-owned open flow: core's pure
/// `probe::prepare::build_catalog` consumes it via `ByteSource`. Mirrors the
/// engine's `collect_probe_bytes_by_asset_id`.
pub(crate) fn blob_byte_map() -> std::collections::HashMap<String, Vec<u8>> {
    BLOB_STORE.with(|s| s.borrow().to_byte_map())
}

/// Borrowed bytes for a single canonical asset id from the thread-local
/// `BlobStore`, as an owned `Vec` (the thread-local borrow cannot escape).
pub(crate) fn blob_bytes_owned(id: &str) -> Option<Vec<u8>> {
    BLOB_STORE.with(|s| {
        s.borrow()
            .get(&AssetId(id.to_string()))
            .map(|arc| arc.to_vec())
    })
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
    let font_manifest = font_manifest_from_source(source)
        .map_err(|e| JsValue::from_str(&format!("preload_assets: parse failed: {e}")))?;

    let mut blobs = take_blobs();

    // Fonts: same bytes in BlobStore (provider) + font_store (fontdb merge on load).
    if !font_manifest.is_empty() {
        for face in &font_manifest.faces {
            match &face.source {
                FontSource::Path(_) => {
                    return Err(JsValue::from_str(
                        "preload_assets: font path is not supported on web; use url",
                    ));
                }
                FontSource::Url(url) => {
                    let id = AssetId(font_asset_id(&FontSource::Url(url.clone())));
                    let raw = crate::resource::resolver::fetch_url(url)
                        .await
                        .map_err(|e| JsValue::from_str(&format!("preload_assets font: {e}")))?;
                    blobs.insert(id, std::sync::Arc::from(raw.clone()));
                    crate::resource::font_store::insert(face.id.clone(), raw);
                }
            }
        }
    }

    let parsed = crate::source::parse_source(source, &fontdb::Database::new())
        .map_err(|e| JsValue::from_str(&format!("preload_assets: parse failed: {e}")))?;
    let requests = collect_resource_requests_from_parsed(&parsed);

    let mut catalog = PreparedResourceCatalog::default();

    crate::resource::resolver::preload_requests(&requests, &mut blobs, &mut catalog)
        .await
        .map_err(|e| JsValue::from_str(&format!("preload_assets: {e}")))?;

    for source in &requests.subtitles {
        let id = asset_id_for_subtitle(source);
        let bytes = match source {
            SubtitleSource::Url(url) => crate::resource::fetch::fetch_bytes(url).await,
            SubtitleSource::Path(path) => {
                crate::resource::asset_reader::read_path(&path.to_string_lossy()).await
            }
        }
        .map_err(|e| JsValue::from_str(&format!("preload_assets subtitle: {e}")))?;
        blobs.insert(id, std::sync::Arc::from(bytes));
    }

    for request in &requests.lotties {
        let bundle_id = AssetId(format!("lottie:{}", request.element_id));
        let primary = match &request.source {
            LottieSource::Url(url) => crate::resource::fetch::fetch_bytes(url).await,
            LottieSource::Path(path) => {
                // Logical locator — web host interprets against document base/VFS.
                crate::resource::asset_reader::read_path(path).await
            }
            LottieSource::Unset => continue,
        }
        .map_err(|e| JsValue::from_str(&format!("preload_assets lottie: {e}")))?;
        let json = std::str::from_utf8(&primary)
            .map_err(|e| JsValue::from_str(&format!("preload_assets lottie utf-8: {e}")))?;
        // Host-only: parse metadata + deps from JSON; bytes stay in BlobStore.
        let meta = opencat_core::resource::parse_lottie_meta(json)
            .map_err(|e| JsValue::from_str(&format!("preload_assets lottie metadata: {e}")))?;
        let dependencies = meta.dependencies.clone();
        catalog.lotties.insert(bundle_id.clone(), meta);
        blobs.insert(bundle_id.clone(), std::sync::Arc::from(primary));

        for file_name in dependencies {
            let resolved = if file_name.starts_with("http://")
                || file_name.starts_with("https://")
                || file_name.starts_with('/')
            {
                file_name.clone()
            } else {
                format!("/assets/{file_name}")
            };
            let bytes = crate::resource::fetch::fetch_bytes(&resolved)
                .await
                .map_err(|e| JsValue::from_str(&format!("preload_assets lottie asset: {e}")))?;
            blobs.insert(
                AssetId(format!("{}:dep:{file_name}", bundle_id.0)),
                std::sync::Arc::from(bytes),
            );
        }
    }

    put_blobs(blobs);

    crate::resource::resolver::catalog_to_js_json(&catalog)
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
    let obj = js_sys::Object::new();
    let prefix = format!("{bundle_id}:dep:");
    BLOB_STORE.with(|store| {
        for (id, bytes) in store.borrow().iter() {
            let Some(name) = id.0.strip_prefix(&prefix) else {
                continue;
            };
            let _ = js_sys::Reflect::set(
                &obj,
                &JsValue::from_str(name),
                &Uint8Array::from(bytes.as_ref()),
            );
        }
    });
    obj.into()
}

/// CanvasKit-style lookup (`path`, `name`) → host-owned bytes.
#[wasm_bindgen]
pub fn load_resource_bytes(path: &str, name: &str) -> Option<Uint8Array> {
    let id = if path == "opencat" {
        AssetId(name.to_string())
    } else {
        AssetId(format!("{path}:dep:{name}"))
    };
    BLOB_STORE.with(|store| {
        store
            .borrow()
            .get(&id)
            .map(|bytes| Uint8Array::from(bytes.as_ref()))
    })
}

#[wasm_bindgen]
pub fn blob_count() -> usize {
    BLOB_STORE.with(|s| s.borrow().len())
}

#[wasm_bindgen]
pub fn clear_blobs() {
    BLOB_STORE.with(|s| s.borrow_mut().clear());
    crate::resource::font_store::clear();
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
