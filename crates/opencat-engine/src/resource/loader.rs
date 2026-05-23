//! [`EngineAssetHandle`] + [`EngineLoader`] — 实现 core 的 `AssetHandle` / `AssetLoader` trait。

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use opencat_core::probe::{AssetHandle, AssetLoader, ResourceRequests};
use opencat_core::probe::{AudioSource, ImageSource, SubtitleSource, VideoSource};
use opencat_core::resource::asset_id::{
    AssetId, asset_id_for_audio_url, asset_id_for_query, asset_id_for_url, asset_id_for_video_url,
};

use opencat_core::resource::resolver::UrlFetcher;

use crate::resource::fetch::{EngineFetcher, build_preload_runtime};
use crate::resource::utils::cache_file_path;

#[derive(Clone)]
pub struct EngineAssetHandle {
    pub(crate) cached_path: PathBuf,
}

impl AssetHandle for EngineAssetHandle {
    fn read_bytes(&self) -> Result<Cow<'_, [u8]>> {
        let bytes = std::fs::read(&self.cached_path)
            .with_context(|| format!("read {}", self.cached_path.display()))?;
        Ok(Cow::Owned(bytes))
    }

    fn local_path(&self) -> Option<&Path> {
        Some(&self.cached_path)
    }
}

pub struct EngineLoader {
    _base_dir: PathBuf,
    cache_dir: PathBuf,
    fetcher: EngineFetcher,
    runtime: tokio::runtime::Runtime,
    handles: HashMap<AssetId, EngineAssetHandle>,
}

impl EngineLoader {
    pub fn new(base_dir: PathBuf, cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir).ok();
        Ok(Self {
            fetcher: EngineFetcher::new(cache_dir.clone())?,
            _base_dir: base_dir,
            cache_dir,
            runtime: build_preload_runtime("engine-loader")?,
            handles: HashMap::new(),
        })
    }
}

impl AssetLoader for EngineLoader {
    type Handle = EngineAssetHandle;

    fn load_all(&mut self, req: &ResourceRequests) -> Result<()> {
        let cache_dir = self.cache_dir.clone();
        let mut new_handles: Vec<(AssetId, PathBuf)> = Vec::new();

        self.runtime.block_on(async {
            for src in &req.images {
                let id = image_asset_id(src);
                match src {
                    ImageSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                    }
                    ImageSource::Path(p) => {
                        copy_local_to_cache(p, &cache_dir, &id)?;
                    }
                    ImageSource::Query(_) => continue,
                    ImageSource::Unset => continue,
                }
                new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
            }

            for src in &req.videos {
                let id = video_asset_id(src);
                match src {
                    VideoSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                    }
                    VideoSource::Path(p) => {
                        copy_local_to_cache(p, &cache_dir, &id)?;
                    }
                }
                new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
            }

            for src in &req.audios {
                let id = audio_asset_id(src);
                match src {
                    AudioSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                    }
                    AudioSource::Path(p) => {
                        copy_local_to_cache(p, &cache_dir, &id)?;
                    }
                    AudioSource::Unset => continue,
                }
                new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
            }

            for src in &req.subtitles {
                let id = subtitle_asset_id(src);
                match src {
                    SubtitleSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                    }
                    SubtitleSource::Path(p) => {
                        copy_local_to_cache(p, &cache_dir, &id)?;
                    }
                }
                new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
            }

            Ok::<_, anyhow::Error>(())
        })?;

        for (id, path) in new_handles {
            self.handles.insert(id, EngineAssetHandle { cached_path: path });
        }
        Ok(())
    }

    fn handle(&self, id: &AssetId) -> Option<&EngineAssetHandle> {
        self.handles.get(id)
    }
}

fn image_asset_id(s: &ImageSource) -> AssetId {
    match s {
        ImageSource::Url(u) => asset_id_for_url(u),
        ImageSource::Path(p) => AssetId(p.to_string_lossy().into_owned()),
        ImageSource::Query(q) => asset_id_for_query(q),
        ImageSource::Unset => AssetId(String::new()),
    }
}

fn video_asset_id(s: &VideoSource) -> AssetId {
    match s {
        VideoSource::Url(u) => asset_id_for_video_url(u),
        VideoSource::Path(p) => AssetId(format!("video:path:{}", p.to_string_lossy())),
    }
}

fn audio_asset_id(s: &AudioSource) -> AssetId {
    match s {
        AudioSource::Url(u) => asset_id_for_audio_url(u),
        AudioSource::Path(p) => AssetId(format!("audio:path:{}", p.to_string_lossy())),
        AudioSource::Unset => AssetId(String::new()),
    }
}

fn subtitle_asset_id(s: &SubtitleSource) -> AssetId {
    match s {
        SubtitleSource::Url(u) => AssetId(format!("subtitle:url:{u}")),
        SubtitleSource::Path(p) => AssetId(format!("subtitle:path:{}", p.to_string_lossy())),
    }
}

fn copy_local_to_cache(src: &Path, cache_dir: &Path, id: &AssetId) -> Result<()> {
    let dst = cache_file_path(cache_dir, id);
    if dst.exists() {
        return Ok(());
    }
    std::fs::copy(src, &dst)
        .with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_all_with_local_path_registers_handle() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();

        let mut loader = EngineLoader::new(tmp.path().to_path_buf(), cache.clone()).unwrap();

        let test_file = tmp.path().join("test.txt");
        std::fs::write(&test_file, b"hello").unwrap();

        let mut req = ResourceRequests::default();
        req.videos.insert(VideoSource::Path(test_file.clone()));

        loader.load_all(&req).unwrap();

        let id = AssetId(format!("video:path:{}", test_file.to_string_lossy()));
        let h = loader.handle(&id).unwrap();
        assert!(h.local_path().is_some());
        assert!(h.local_path().unwrap().exists());
    }
}
