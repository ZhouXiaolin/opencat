//! Web 侧 [`AssetResolver`] —— 通过 `fetch()` 下字节、写入 [`BlobStore`]。
//! 路径变体 (`resolve_image_path` 等) 不实现 —— web 没有文件系统。
//! query 变体走 core 默认实现（Openverse 搜索也是 HTTP，WebFetcher 即可处理）。

use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;

use opencat_core::resource::asset_id::AssetId;
use opencat_core::resource::resolver::{
    AssetResolver, AssetSink, AudioMeta, ImageMeta, UrlFetcher, VideoMeta,
};
use opencat_core::resource::{probe_image_dims, probe_video};

use crate::resource::asset_reader;
use crate::resource::blob_store::BlobStore;
use crate::resource::fetch::fetch_bytes;

/// Web 端 URL → 字节下载器，直接走 `fetch()` JS 桥。
pub struct WebFetcher;

impl UrlFetcher for WebFetcher {
    fn fetch_bytes(&mut self, _id: &AssetId, url: &str) -> impl Future<Output = Result<Vec<u8>>> {
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

    // URL / query 变体走 core 默认实现。
    // path 变体通过宿主注册的 JS asset reader 读取 VFS bytes。
    fn resolve_image_path(&mut self, path: &Path) -> impl Future<Output = Result<ImageMeta>> {
        let id = AssetId(path.to_string_lossy().into_owned());
        let path = id.0.clone();
        async move {
            let bytes = asset_reader::read_path(&path).await?;
            let dims = probe_image_dims(&bytes)?;
            self.sink.store(&id, bytes);
            Ok(ImageMeta {
                id,
                width: dims.width,
                height: dims.height,
            })
        }
    }

    fn resolve_video_path(&mut self, path: &Path) -> impl Future<Output = Result<VideoMeta>> {
        let id = AssetId(path.to_string_lossy().into_owned());
        let path = id.0.clone();
        async move {
            let bytes = asset_reader::read_path(&path).await?;
            let probe = probe_video(&bytes)?;
            self.sink.store(&id, bytes);
            Ok(VideoMeta {
                id,
                width: probe.width,
                height: probe.height,
                duration_secs: probe.duration_ms.map(|ms| ms as f64 / 1000.0),
            })
        }
    }

    fn resolve_audio_path(&mut self, path: &Path) -> impl Future<Output = Result<AudioMeta>> {
        let id = AssetId(path.to_string_lossy().into_owned());
        let path = id.0.clone();
        async move {
            let bytes = asset_reader::read_path(&path).await?;
            self.sink.store(&id, bytes);
            Ok(AudioMeta { id })
        }
    }
}
