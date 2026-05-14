//! Engine 侧 [`AssetResolver`] 实现 —— tokio + reqwest 下载、`image` crate
//! 读图片维度、`MediaContext` 读视频维度。
//!
//! 所有方法 async 返回 `impl Future`；`EnginePlatform::preflight` 用
//! `tokio::runtime::block_on` 同步驱动 `preload_all`。

use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::Result;

use opencat_core::resource::asset_id::{
    asset_id_for_audio_url, asset_id_for_query, asset_id_for_url, asset_id_for_video_url, AssetId,
};
use opencat_core::resource::resolver::{AssetResolver, AudioMeta, ImageMeta, VideoMeta};
use opencat_core::scene::primitives::OpenverseQuery;

use crate::resource::fetch::{
    build_http_client, download_to_cache, search_openverse_image,
};
use crate::resource::media::MediaContext;
use crate::resource::path_store::AssetPathStore;
use crate::resource::utils::{cache_file_path, read_image_dimensions};

pub struct EngineAssetResolver<'a> {
    client: reqwest::Client,
    cache_dir: PathBuf,
    openverse_token: Option<String>,
    path_store: &'a mut AssetPathStore,
    video_probe: &'a mut MediaContext,
}

impl<'a> EngineAssetResolver<'a> {
    pub fn new(
        path_store: &'a mut AssetPathStore,
        video_probe: &'a mut MediaContext,
        cache_dir: PathBuf,
        openverse_token: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            client: build_http_client("failed to build async http client")?,
            cache_dir,
            openverse_token,
            path_store,
            video_probe,
        })
    }

    fn cache_path(&self, id: &AssetId, ext: &str) -> PathBuf {
        cache_file_path(&self.cache_dir, id, ext)
    }
}

impl<'a> AssetResolver for EngineAssetResolver<'a> {
    fn resolve_image_url(&mut self, url: &str) -> impl Future<Output = Result<ImageMeta>> {
        let id = asset_id_for_url(url);
        let path = self.cache_path(&id, "img");
        let client = self.client.clone();
        let url = url.to_string();
        async move {
            if !path.exists() {
                download_to_cache(&client, &url, &path, "image").await?;
            }
            let (width, height) = read_image_dimensions(&path);
            self.path_store.insert(id.clone(), path);
            Ok(ImageMeta { id, width, height })
        }
    }

    fn resolve_image_path(&mut self, path: &Path) -> impl Future<Output = Result<ImageMeta>> {
        let path = path.to_path_buf();
        async move {
            let (width, height) = read_image_dimensions(&path);
            let id = AssetId(path.to_string_lossy().into_owned());
            self.path_store.insert(id.clone(), path);
            Ok(ImageMeta { id, width, height })
        }
    }

    fn resolve_image_query(
        &mut self,
        query: &OpenverseQuery,
    ) -> impl Future<Output = Result<ImageMeta>> {
        let id = asset_id_for_query(query);
        let path = self.cache_path(&id, "img");
        let client = self.client.clone();
        let token = self.openverse_token.clone();
        let query = query.clone();
        async move {
            if !path.exists() {
                let url = search_openverse_image(&client, token.as_deref(), &query).await?;
                download_to_cache(&client, &url, &path, "image").await?;
            }
            let (width, height) = read_image_dimensions(&path);
            self.path_store.insert(id.clone(), path);
            Ok(ImageMeta { id, width, height })
        }
    }

    fn resolve_audio_url(&mut self, url: &str) -> impl Future<Output = Result<AudioMeta>> {
        let id = asset_id_for_audio_url(url);
        let path = self.cache_path(&id, "audio");
        let client = self.client.clone();
        let url = url.to_string();
        async move {
            if !path.exists() {
                download_to_cache(&client, &url, &path, "audio").await?;
            }
            self.path_store.insert(id.clone(), path);
            Ok(AudioMeta { id })
        }
    }

    fn resolve_audio_path(&mut self, path: &Path) -> impl Future<Output = Result<AudioMeta>> {
        let path = path.to_path_buf();
        async move {
            let id = AssetId(path.to_string_lossy().into_owned());
            self.path_store.insert(id.clone(), path);
            Ok(AudioMeta { id })
        }
    }

    fn resolve_video_url(&mut self, url: &str) -> impl Future<Output = Result<VideoMeta>> {
        let id = asset_id_for_video_url(url);
        let path = self.cache_path(&id, "mp4");
        let client = self.client.clone();
        let url = url.to_string();
        async move {
            if !path.exists() {
                download_to_cache(&client, &url, &path, "video").await?;
            }
            let info = self.video_probe.video_info(&path)?;
            self.path_store.insert(id.clone(), path);
            Ok(VideoMeta {
                id,
                width: info.width,
                height: info.height,
                duration_secs: info.duration_secs,
            })
        }
    }

    fn resolve_video_path(&mut self, path: &Path) -> impl Future<Output = Result<VideoMeta>> {
        let path = path.to_path_buf();
        async move {
            let info = self.video_probe.video_info(&path)?;
            let id = AssetId(path.to_string_lossy().into_owned());
            self.path_store.insert(id.clone(), path);
            Ok(VideoMeta {
                id,
                width: info.width,
                height: info.height,
                duration_secs: info.duration_secs,
            })
        }
    }
}
