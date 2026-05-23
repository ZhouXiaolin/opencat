//! AssetResolver trait — 统一资源解析接口。
//!
//! 输入：[`crate::parse::primitives::ImageSource`] /
//! [`crate::parse::primitives::AudioSource`] /
//! [`crate::parse::primitives::VideoSource`] / Path / Openverse query。
//! 输出：`*Meta`（含 [`AssetId`] + 宽高/时长等元数据）。
//!
//! 设计：core 持有 URL 变体 + Openverse 查询的完整流水线（id → fetch → probe → store），
//! 平台只需提供 [`UrlFetcher`] + [`AssetSink`] 两个底层 trait 的实现。
//! Path 变体由平台 override（engine 实现，wasm 默认 bail）。
//!
//! 没有 `Send` bound：原生 tokio multi-thread 实现自然返回 Send future，
//! wasm 单线程实现返回 !Send future，两端都能编译。

use std::future::Future;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::parse::primitives::OpenverseQuery;
use crate::resource::asset_id::{
    AssetId, asset_id_for_audio_url, asset_id_for_query, asset_id_for_url, asset_id_for_video_url,
};
use crate::resource::probe::{probe_image_dims, probe_video};

#[derive(Clone, Debug, PartialEq)]
pub struct ImageMeta {
    pub id: AssetId,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoMeta {
    pub id: AssetId,
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioMeta {
    pub id: AssetId,
}

/// 平台特定的 URL → 字节下载器。
///
/// 实现方可自由处理 cache 策略（Engine 写盘 + 命中复用；Web 直 fetch）。
/// `id` 参数允许实现方基于稳定 hash 决定 cache 路径。
pub trait UrlFetcher {
    fn fetch_bytes(&mut self, id: &AssetId, url: &str) -> impl Future<Output = Result<Vec<u8>>>;
}

/// 平台特定的字节持久化。Engine: 更新 path_store；Web: 写 BlobStore。
///
/// 注意 Engine 实现可能忽略 `bytes` 参数（字节已被 Fetcher 写到 cache_dir，
/// Sink 只需要建立 id → path 的索引）。
pub trait AssetSink {
    fn store(&mut self, id: &AssetId, bytes: Vec<u8>);
}

/// 统一资源解析。core 提供 URL 变体默认实现，平台通过关联类型注入
/// [`UrlFetcher`] + [`AssetSink`] 即可。
pub trait AssetResolver {
    type Fetcher: UrlFetcher;
    type Sink: AssetSink;

    /// 同时取出 fetcher 和 sink 的可变引用。
    /// 必须同时返回避免 borrow checker 在 async 块里冲突。
    fn parts(&mut self) -> (&mut Self::Fetcher, &mut Self::Sink);

    fn resolve_image_url(&mut self, url: &str) -> impl Future<Output = Result<ImageMeta>> {
        let id = asset_id_for_url(url);
        let url = url.to_string();
        async move {
            let (fetcher, sink) = self.parts();
            let bytes = fetcher.fetch_bytes(&id, &url).await?;
            let dims = probe_image_dims(&bytes)?;
            sink.store(&id, bytes);
            Ok(ImageMeta {
                id,
                width: dims.width,
                height: dims.height,
            })
        }
    }

    fn resolve_audio_url(&mut self, url: &str) -> impl Future<Output = Result<AudioMeta>> {
        let id = asset_id_for_audio_url(url);
        let url = url.to_string();
        async move {
            let (fetcher, sink) = self.parts();
            let bytes = fetcher.fetch_bytes(&id, &url).await?;
            sink.store(&id, bytes);
            Ok(AudioMeta { id })
        }
    }

    fn resolve_video_url(&mut self, url: &str) -> impl Future<Output = Result<VideoMeta>> {
        let id = asset_id_for_video_url(url);
        let url = url.to_string();
        async move {
            let (fetcher, sink) = self.parts();
            let bytes = fetcher.fetch_bytes(&id, &url).await?;
            let probe = probe_video(&bytes)?;
            sink.store(&id, bytes);
            Ok(VideoMeta {
                id,
                width: probe.width,
                height: probe.height,
                duration_secs: probe.duration_ms.map(|ms| ms as f64 / 1000.0),
            })
        }
    }

    /// 本地文件系统路径。engine override；wasm 默认 bail。
    fn resolve_image_path(&mut self, _path: &Path) -> impl Future<Output = Result<ImageMeta>> {
        async { anyhow::bail!("resolve_image_path not supported on this platform") }
    }
    fn resolve_video_path(&mut self, _path: &Path) -> impl Future<Output = Result<VideoMeta>> {
        async { anyhow::bail!("resolve_video_path not supported on this platform") }
    }
    fn resolve_audio_path(&mut self, _path: &Path) -> impl Future<Output = Result<AudioMeta>> {
        async { anyhow::bail!("resolve_audio_path not supported on this platform") }
    }

    /// Openverse 图片搜索。core 提供默认实现：搜索 Openverse → 取图片 URL →
    /// fetcher 下载 → probe → sink 存储。各端无需 override。
    fn resolve_image_query(
        &mut self,
        query: &OpenverseQuery,
    ) -> impl Future<Output = Result<ImageMeta>> {
        let id = asset_id_for_query(query);
        let query = query.clone();
        async move {
            let (fetcher, sink) = self.parts();

            // 用 search-specific ID 搜索，避免与 query_id 缓存冲突
            let search_id = AssetId(format!("openverse:search:{}", query.query));
            let search_url = build_openverse_search_url(&query);
            let search_bytes = fetcher
                .fetch_bytes(&search_id, &search_url)
                .await
                .with_context(|| format!("failed to query Openverse for {:?}", query.query))?;

            // 解析搜索结果，拿到图片 URL
            let image_url = parse_openverse_response(&search_bytes)
                .with_context(|| format!("bad Openverse response for {:?}", query.query))?;

            // 下载图片（fetcher 缓存到 query_id），probe，sink 登记
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

/// 构建 Openverse 搜索 URL（core 内部帮助函数，两端共享）。
fn build_openverse_search_url(query: &OpenverseQuery) -> String {
    let page_size = query.count.max(1).to_string();
    let mut url = url::Url::parse("https://api.openverse.org/v1/images/")
        .expect("static Openverse endpoint URL is valid");
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("q", &query.query);
        params.append_pair("page_size", &page_size);
        if let Some(aspect_ratio) = &query.aspect_ratio {
            params.append_pair("aspect_ratio", aspect_ratio);
        }
    }
    url.to_string()
}

/// 解析 Openverse JSON 响应，提取第一张图片的 URL。
fn parse_openverse_response(bytes: &[u8]) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct ImageResult {
        url: Option<String>,
        thumbnail: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct SearchResponse {
        results: Vec<ImageResult>,
    }

    let resp: SearchResponse = serde_json::from_slice(bytes)?;
    resp.results
        .into_iter()
        .find_map(|r| r.url.or(r.thumbnail))
        .ok_or_else(|| anyhow!("Openverse returned no image"))
}
