//! Engine 侧 [`AssetResolver`] —— tokio + reqwest 下载、core 提供探测函数、
//! `MediaContext` 仍保留用于渲染时视频帧解码（不再用于 preload 元数据探测）。
//!
//! 所有方法 async 返回 `impl Future`；`EnginePlatform::preflight` 用
//! `tokio::runtime::block_on` 同步驱动 `preload_all`。

use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use opencat_core::resource::asset_id::{AssetId, asset_id_for_query};
use opencat_core::resource::probe::{probe_image_dims, probe_video};
use opencat_core::resource::resolver::{
    AssetResolver, AssetSink, AudioMeta, ImageMeta, VideoMeta,
};
use opencat_core::scene::primitives::OpenverseQuery;

use crate::resource::fetch::{EngineFetcher, search_openverse_image};
use crate::resource::path_store::AssetPathStore;
use crate::resource::utils::cache_file_path;

/// Engine 端 (id, bytes) → path_store 索引建立。
///
/// 字节已被 [`EngineFetcher`] 写到 `cache_dir`，本 Sink 只负责注册
/// `id → cache_path` 映射，`store` 不写盘（避免重复 IO）。
pub struct EngineSink<'a> {
    path_store: &'a mut AssetPathStore,
    cache_dir: PathBuf,
}

impl<'a> EngineSink<'a> {
    pub fn new(path_store: &'a mut AssetPathStore, cache_dir: PathBuf) -> Self {
        Self {
            path_store,
            cache_dir,
        }
    }

    /// path 变体专用：直接注册外部 path（字节不进 cache）。
    pub fn register_external_path(&mut self, id: AssetId, path: PathBuf) {
        self.path_store.insert(id, path);
    }
}

impl<'a> AssetSink for EngineSink<'a> {
    fn store(&mut self, id: &AssetId, _bytes: Vec<u8>) {
        let path = cache_file_path(&self.cache_dir, id);
        self.path_store.insert(id.clone(), path);
    }
}

pub struct EngineAssetResolver<'a> {
    fetcher: EngineFetcher,
    sink: EngineSink<'a>,
    openverse_token: Option<String>,
}

impl<'a> EngineAssetResolver<'a> {
    pub fn new(
        path_store: &'a mut AssetPathStore,
        cache_dir: PathBuf,
        openverse_token: Option<String>,
    ) -> Result<Self> {
        let fetcher = EngineFetcher::new(cache_dir.clone())?;
        let sink = EngineSink::new(path_store, cache_dir);
        Ok(Self {
            fetcher,
            sink,
            openverse_token,
        })
    }
}

impl<'a> AssetResolver for EngineAssetResolver<'a> {
    type Fetcher = EngineFetcher;
    type Sink = EngineSink<'a>;

    fn parts(&mut self) -> (&mut EngineFetcher, &mut EngineSink<'a>) {
        (&mut self.fetcher, &mut self.sink)
    }

    // URL 变体走 core 的默认实现（id → fetcher → probe → sink）。

    fn resolve_image_path(&mut self, path: &Path) -> impl Future<Output = Result<ImageMeta>> {
        let path = path.to_path_buf();
        async move {
            let bytes = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read image {}", path.display()))?;
            let dims = probe_image_dims(&bytes)?;
            let id = AssetId(path.to_string_lossy().into_owned());
            self.sink.register_external_path(id.clone(), path);
            Ok(ImageMeta {
                id,
                width: dims.width,
                height: dims.height,
            })
        }
    }

    fn resolve_audio_path(&mut self, path: &Path) -> impl Future<Output = Result<AudioMeta>> {
        let path = path.to_path_buf();
        async move {
            let id = AssetId(path.to_string_lossy().into_owned());
            self.sink.register_external_path(id.clone(), path);
            Ok(AudioMeta { id })
        }
    }

    fn resolve_video_path(&mut self, path: &Path) -> impl Future<Output = Result<VideoMeta>> {
        let path = path.to_path_buf();
        async move {
            let bytes = tokio::fs::read(&path)
                .await
                .with_context(|| format!("failed to read video {}", path.display()))?;
            let probe = probe_video(&bytes)?;
            let id = AssetId(path.to_string_lossy().into_owned());
            self.sink.register_external_path(id.clone(), path);
            Ok(VideoMeta {
                id,
                width: probe.width,
                height: probe.height,
                duration_secs: probe.duration_secs,
            })
        }
    }

    fn resolve_image_query(
        &mut self,
        query: &OpenverseQuery,
    ) -> impl Future<Output = Result<ImageMeta>> {
        let id = asset_id_for_query(query);
        let cache_path = cache_file_path(self.fetcher.cache_dir(), &id);
        let client = self.fetcher.client().clone();
        let token = self.openverse_token.clone();
        let query = query.clone();
        async move {
            let bytes = if cache_path.exists() {
                tokio::fs::read(&cache_path)
                    .await
                    .with_context(|| format!("failed to read cached query asset {}", cache_path.display()))?
            } else {
                let url = search_openverse_image(&client, token.as_deref(), &query).await?;
                let bytes = crate::resource::fetch::download_bytes(&client, &url).await?;
                tokio::fs::write(&cache_path, &bytes).await.with_context(|| {
                    format!("failed to write cache {}", cache_path.display())
                })?;
                bytes
            };
            let dims = probe_image_dims(&bytes)?;
            // 直接注册到 path_store（绕过 EngineSink::store 避免重新计算 path）。
            self.sink.register_external_path(id.clone(), cache_path);
            Ok(ImageMeta {
                id,
                width: dims.width,
                height: dims.height,
            })
        }
    }
}
