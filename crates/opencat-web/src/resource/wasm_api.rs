//! `#[wasm_bindgen]` 暴露给 JS 的资源预加载 API。
//!
//! - [`preload_assets`]: 解析 JSONL → 收集资源请求 → 并通过 [`AssetResolver`]
//!   下载 + 读元数据 → 把字节灌进全局 `BlobStore`，返回 catalog JSON 给 JS。
//! - [`get_blob_bytes`]: JS 用 AssetId 拉回已下载的字节（用于 CanvasKit 解码、
//!   `URL.createObjectURL` 等下游消费）。
//! - [`clear_blobs`]: 清空 BlobStore（切换 composition 时调用）。

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;

use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;

use opencat_core::jsonl::JsonLine;
use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::resource::preload::preload_all;
use opencat_core::runtime::preflight_collect::collect_resource_requests;
use opencat_core::scene::composition::Composition;
use opencat_core::scene::primitives::{AudioSource, ImageSource, VideoSource};

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

/// 下载 JSONL 引用的全部资源，把字节放进 BlobStore，返回 catalog JSON。
///
/// 返回的 JSON 形态与 [`HashMapResourceCatalog::from_json`] 接受的相同：
/// `{ "<asset_id>": { "width": w, "height": h, "kind": "image"|"video"|"audio",
///                    "duration_secs": Option<f64> }, ... }`
///
/// JS 把这个串原样传给后续 `WebRenderer.build_frame(resource_meta=...)`。
#[wasm_bindgen]
pub async fn preload_assets(jsonl: &str) -> Result<String, JsValue> {
    // 1) JSONL → Composition（最小拼装：足够 preflight_collect 工作）。
    let composition = build_composition_from_jsonl(jsonl)
        .map_err(|e| JsValue::from_str(&format!("preload_assets: parse failed: {e}")))?;

    // 2) preflight 收集请求。
    let requests = collect_resource_requests(&composition);

    // 3) 取出全局 BlobStore，构造 resolver + 临时 catalog。
    let mut blobs = take_blobs();
    let mut catalog = HashMapResourceCatalog::from_json("{}")
        .map_err(|e| JsValue::from_str(&format!("preload_assets: catalog init: {e}")))?;

    // 4) 跑 core 编排（所有 fetch + probe 都在这一步里）。
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
}

/// 当前 BlobStore 条目数，调试用。
#[wasm_bindgen]
pub fn blob_count() -> usize {
    BLOB_STORE.with(|s| s.borrow().len())
}

// ── helpers ──────────────────────────────────────────────────────────────

/// 把 JSONL 解析成一个最小 Composition，仅含 preflight_collect 需要的字段：
/// composition 尺寸 + 一个静态 root 节点把所有 image/video/audio 都塞进去。
fn build_composition_from_jsonl(input: &str) -> anyhow::Result<Composition> {
    use opencat_core::scene::primitives::{div, image, video, video_url};
    use std::sync::Arc;

    let mut width = 1920_i32;
    let mut height = 1080_i32;
    let mut fps = 30_i32;
    let mut frames = 1_i32;

    let mut image_sources: Vec<ImageSource> = Vec::new();
    let mut audio_sources: Vec<AudioSource> = Vec::new();
    let mut video_sources: Vec<VideoSource> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: serde_json::Result<JsonLine> = serde_json::from_str(trimmed);
        let Ok(line) = parsed else { continue };
        match line {
            JsonLine::Composition {
                width: w,
                height: h,
                fps: f,
                frames: fs,
            } => {
                width = w;
                height = h;
                fps = f;
                frames = fs;
            }
            JsonLine::Image { url, .. } => {
                if let Some(url) = url {
                    image_sources.push(ImageSource::Url(url));
                }
            }
            JsonLine::Audio { url, .. } => {
                if let Some(url) = url {
                    audio_sources.push(AudioSource::Url(url));
                }
            }
            JsonLine::Video { url, path, .. } => match (url, path) {
                (Some(u), _) => video_sources.push(VideoSource::Url(u)),
                (None, Some(p)) if !p.is_empty() => {
                    video_sources.push(VideoSource::Path(std::path::PathBuf::from(p)));
                }
                _ => {}
            },
            _ => {}
        }
    }

    // 组装一个 root: 每个资源做一个 leaf。preflight_collect 会去遍历。
    let img_clones = image_sources.clone();
    let vid_clones = video_sources.clone();
    let root_builder = Arc::new(move |_ctx: &opencat_core::FrameCtx| {
        let mut root = div().id("__preload_root__");
        for (i, src) in img_clones.iter().enumerate() {
            let img = match src {
                ImageSource::Url(u) => {
                    let id_str = format!("__preload_img_{i}");
                    image().id(&id_str[..]).url(u)
                }
                _ => continue,
            };
            root = root.child(img);
        }
        for (i, src) in vid_clones.iter().enumerate() {
            let id_str = format!("__preload_vid_{i}");
            let v = match src {
                VideoSource::Url(u) => video_url(u).id(&id_str[..]),
                VideoSource::Path(p) => {
                    video(p.to_string_lossy().to_string()).id(&id_str[..])
                }
            };
            root = root.child(v);
        }
        root.into()
    });

    let audio_sources_for_builder = audio_sources.clone();
    let builder = Composition::new("preload")
        .size(width, height)
        .fps(fps as u32)
        .frames(frames.max(1) as u32)
        .root(move |ctx| root_builder(ctx))
        .global_audio_sources(audio_sources_for_builder);
    builder.build()
}
