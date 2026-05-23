//! Engine 侧 [`AssetResolver`] —— tokio + reqwest 下载、core 提供探测函数、
//! `MediaContext` 仍保留用于渲染时视频帧解码（不再用于 preload 元数据探测）。
//!
//! 所有方法 async 返回 `impl Future`；`EnginePlatform::preflight` 用
//! `tokio::runtime::block_on` 同步驱动 `preload_all`。

use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::probe::{probe_image_dims, probe_video};
use opencat_core::resource::resolver::{AssetResolver, AssetSink, AudioMeta, ImageMeta, VideoMeta};

use crate::resource::AssetPathStore;
use crate::resource::fetch::EngineFetcher;
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
}

impl<'a> EngineAssetResolver<'a> {
    pub fn new(path_store: &'a mut AssetPathStore, cache_dir: PathBuf) -> Result<Self> {
        let fetcher = EngineFetcher::new(cache_dir.clone())?;
        let sink = EngineSink::new(path_store, cache_dir);
        Ok(Self { fetcher, sink })
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
                duration_secs: probe.duration_ms.map(|ms| ms as f64 / 1000.0),
            })
        }
    }
}
