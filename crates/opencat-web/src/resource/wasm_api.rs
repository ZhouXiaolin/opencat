//! `#[wasm_bindgen]` 暴露给 JS 的资源预加载 API。
//!
//! - [`preload_assets`]: 解析 composition source(JSONL/XML) → 收集资源请求 → 并通过 [`AssetResolver`]
//!   下载 + 读元数据 → 把字节灌进全局 `BlobStore`，返回 catalog JSON 给 JS。
//! - [`get_blob_bytes`]: JS 用 AssetId 拉回已下载的字节（用于 CanvasKit 解码、
//!   `URL.createObjectURL` 等下游消费）。
//! - [`clear_blobs`]: 清空 BlobStore（切换 composition 时调用）。

use std::cell::RefCell;

use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

use opencat_core::parse::composition::Composition;
use opencat_core::parse::preflight::collect_resource_requests;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::resource::fonts::{font_asset_id, FontSource};
use opencat_core::resource::preload::preload_all;

use crate::resource::blob_store::BlobStore;
use crate::resource::resolver::WebAssetResolver;

thread_local! {
    static BLOB_STORE: RefCell<BlobStore> = RefCell::new(BlobStore::new());
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
        Ok(
            opencat_core::parse::markup::parse_parts_with_base_dir(source, None)?.font_manifest,
        )
    }
}

/// 下载 composition source 引用的全部资源，把字节放进 BlobStore，返回 catalog JSON。
///
/// 返回的 JSON 形态与 [`HashMapResourceCatalog::from_json`] 接受的相同：
/// `{ "<asset_id>": { "width": w, "height": h, "kind": "image"|"video"|"audio",
///                    "duration_secs": Option<f64> }, ... }`
///
/// JS 把这个串原样传给后续 `WebRenderer.build_frame_ir(resources_json=...)`。
#[wasm_bindgen]
pub async fn preload_assets(source: &str) -> Result<String, JsValue> {
    // 1) 用 core 的解析器解析 JSONL/XML → ParsedComposition。
    let font_manifest = font_manifest_from_source(source)
        .map_err(|e| JsValue::from_str(&format!("preload_assets: parse failed: {e}")))?;

    if !font_manifest.is_empty() {
        use opencat_core::resource::asset_id::AssetId;
        use opencat_core::resource::fonts::FontSource;

        let fetcher = crate::resource::resolver::WebFetcher;
        for face in &font_manifest.faces {
            let bytes = match &face.source {
                FontSource::Path(_) => {
                    return Err(JsValue::from_str(
                        "preload_assets: font path is not supported on web; use url",
                    ));
                }
                FontSource::Url(url) => {
                    let id = AssetId(font_asset_id(&FontSource::Url(url.clone())));
                    fetcher
                        .fetch_bytes(&id, url)
                        .await
                        .map_err(|e| JsValue::from_str(&format!("preload_assets font: {e}")))?
                }
            };
            crate::resource::font_store::insert(face.id.clone(), bytes);
        }
    }

    // 2) 组装 Composition（复用 parsed 的场景树 + 音频源），让 collect_resource_requests 能遍历。
    let parsed = crate::source::parse_source(source, &fontdb::Database::new())
        .map_err(|e| JsValue::from_str(&format!("preload_assets: parse failed: {e}")))?;
    let root_node = parsed.root.clone();
    let composition = Composition::new("preload")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .frames(parsed.frames.max(1) as u32)
        .root(move |_ctx| root_node.clone())
        .audio_sources(parsed.audio_sources)
        .build()
        .map_err(|e| JsValue::from_str(&format!("preload_assets: build composition: {e}")))?;

    // 3) preflight 收集请求。
    let requests = collect_resource_requests(&composition);

    // 4) 取出全局 BlobStore，构造 resolver + 临时 catalog。
    let mut blobs = take_blobs();
    let mut catalog = HashMapResourceCatalog::from_json("{}")
        .map_err(|e| JsValue::from_str(&format!("preload_assets: catalog init: {e}")))?;

    // 5) 跑 core 编排（所有 fetch + probe 都在这一步里）。
    let preload_result = {
        let mut resolver = WebAssetResolver::new(&mut blobs);
        preload_all(requests, &mut resolver, &mut catalog).await
    };
    put_blobs(blobs);

    preload_result.map_err(|e| JsValue::from_str(&format!("preload_assets: {e}")))?;

    catalog
        .to_json()
        .map_err(|e| JsValue::from_str(&format!("preload_assets: serialize: {e}")))
}

/// JS 取出某个 asset 的字节（用于 CanvasKit 解码 / Blob URL / 等）。
#[wasm_bindgen]
pub fn get_blob_bytes(asset_id: &str) -> Option<Uint8Array> {
    BLOB_STORE.with(|s| {
        s.borrow()
            .get(&AssetId(asset_id.to_string()))
            .map(|arc| Uint8Array::from(&arc[..]))
    })
}

/// 清空 BlobStore（建议在加载新 composition 前调用）。
#[wasm_bindgen]
pub fn clear_blobs() {
    BLOB_STORE.with(|s| s.borrow_mut().clear());
    crate::resource::font_store::clear();
}

/// 当前 BlobStore 条目数，调试用。
#[wasm_bindgen]
pub fn blob_count() -> usize {
    BLOB_STORE.with(|s| s.borrow().len())
}
