//! Web 侧 [`AssetResolver`] —— 通过 `fetch()` 下字节、写入 [`BlobStore`]。
//! 探测函数复用 core 的 `imagesize` + `nom-exif` 实现；本文件只负责装配。
//!
//! 路径变体 (`resolve_image_path` 等) 不实现 —— web 没有文件系统。

use std::future::Future;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};

use opencat_core::resource::asset_id::{AssetId, asset_id_for_query};
use opencat_core::resource::probe::probe_image_dims;
use opencat_core::resource::resolver::{AssetResolver, AssetSink, ImageMeta, UrlFetcher};
use opencat_core::scene::primitives::OpenverseQuery;

use crate::resource::blob_store::BlobStore;
use crate::resource::fetch::fetch_bytes;

const OPENVERSE_IMAGES_ENDPOINT: &str = "https://api.openverse.org/v1/images/";

/// Web 端 URL → 字节下载器，直接走 `fetch()` JS 桥。
pub struct WebFetcher;

impl UrlFetcher for WebFetcher {
    fn fetch_bytes(
        &mut self,
        _id: &AssetId,
        url: &str,
    ) -> impl Future<Output = Result<Vec<u8>>> {
        let url = url.to_string();
        async move { fetch_bytes(&url).await }
    }
}

/// Web 端 (id, bytes) → BlobStore 持久化。
pub struct WebSink<'a> {
    blobs: &'a mut BlobStore,
}

impl<'a> WebSink<'a> {
    pub fn new(blobs: &'a mut BlobStore) -> Self {
        Self { blobs }
    }
}

impl<'a> AssetSink for WebSink<'a> {
    fn store(&mut self, id: &AssetId, bytes: Vec<u8>) {
        self.blobs.insert(id.clone(), Arc::from(bytes));
    }
}

pub struct WebAssetResolver<'a> {
    fetcher: WebFetcher,
    sink: WebSink<'a>,
}

impl<'a> WebAssetResolver<'a> {
    pub fn new(blobs: &'a mut BlobStore) -> Self {
        Self {
            fetcher: WebFetcher,
            sink: WebSink::new(blobs),
        }
    }
}

impl<'a> AssetResolver for WebAssetResolver<'a> {
    type Fetcher = WebFetcher;
    type Sink = WebSink<'a>;

    fn parts(&mut self) -> (&mut WebFetcher, &mut WebSink<'a>) {
        (&mut self.fetcher, &mut self.sink)
    }

    // URL 变体走 core 默认实现。
    // path 变体走 trait 的默认 bail（web 没有文件系统）。

    fn resolve_image_query(
        &mut self,
        query: &OpenverseQuery,
    ) -> impl Future<Output = Result<ImageMeta>> {
        let id = asset_id_for_query(query);
        let query = query.clone();
        async move {
            let image_url = search_openverse_image(&query).await?;
            let (fetcher, sink) = self.parts();
            let bytes = fetcher.fetch_bytes(&id, &image_url).await?;
            let dims = probe_image_dims(&bytes)?;
            sink.store(&id, bytes);
            Ok(ImageMeta {
                id,
                width: dims.width,
                height: dims.height,
            })
        }
    }
}

async fn search_openverse_image(query: &OpenverseQuery) -> Result<String> {
    let page_size = query.count.max(1).to_string();
    let mut url = url::Url::parse(OPENVERSE_IMAGES_ENDPOINT)
        .context("failed to parse Openverse images endpoint")?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("q", &query.query);
        params.append_pair("page_size", &page_size);
        if let Some(aspect_ratio) = &query.aspect_ratio {
            params.append_pair("aspect_ratio", aspect_ratio);
        }
    }

    let bytes = fetch_bytes(url.as_str())
        .await
        .with_context(|| format!("failed to query Openverse for {:?}", query.query))?;

    #[derive(serde::Deserialize)]
    struct OpenverseImageResult {
        url: Option<String>,
        thumbnail: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct OpenverseSearchResponse {
        results: Vec<OpenverseImageResult>,
    }

    let payload: OpenverseSearchResponse = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to decode Openverse response for {:?}", query.query))?;

    payload
        .results
        .into_iter()
        .find_map(|r| r.url.or(r.thumbnail))
        .ok_or_else(|| anyhow!("Openverse returned no image for query {:?}", query.query))
}
