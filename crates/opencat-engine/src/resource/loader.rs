//! [`EngineAssetHandle`] + [`EngineLoader`] — 实现 core 的 `AssetHandle` / `AssetLoader` trait。

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use opencat_core::probe::{AssetHandle, AssetLoader, ResourceRequests};
use opencat_core::probe::{AudioSource, ImageSource, SubtitleSource, VideoSource};
use opencat_core::resource::asset_id::{
    AssetId, asset_id_for_audio_url, asset_id_for_query, asset_id_for_url, asset_id_for_video_url,
};
use opencat_core::resource::fonts::{font_asset_id, FontManifest};
use opencat_core::resource::manifest::ExternalResourceManifest;
use opencat_core::resource::materialize::{ByteSource, hydrate_provider_from_bytes};
use opencat_core::resource::protocol::MapResourceProvider;

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
    /// Skottie-aligned resource map after preload (for Lottie / unified host access).
    pub resource_provider: Option<MapResourceProvider>,
}

struct LoaderByteSource<'a>(&'a EngineLoader);

impl ByteSource for LoaderByteSource<'_> {
    fn bytes_for(&self, id: &AssetId) -> Option<Vec<u8>> {
        self.0
            .handle(id)?
            .read_bytes()
            .ok()
            .map(|c| c.into_owned())
    }
}

impl EngineLoader {
    pub fn base_dir(&self) -> &Path {
        &self._base_dir
    }

    /// Download / read all fonts declared in `<fonts>` for markup compositions.
    pub fn load_font_manifest(
        &mut self,
        manifest: &FontManifest,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>> {
        if manifest.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let base_dir = self._base_dir.clone();
        let cache_dir = self.cache_dir.clone();
        let mut out = std::collections::HashMap::new();
        for face in &manifest.faces {
            let bytes = match &face.source {
                opencat_core::resource::fonts::FontSource::Path(path) => {
                    let resolved = opencat_core::resource::fonts::resolve_font_source_path(
                        &path.to_string_lossy(),
                        Some(&base_dir),
                    )
                    .with_context(|| format!("font `{}`", face.id))?;
                    std::fs::read(&resolved)
                        .with_context(|| format!("read font {}", resolved.display()))?
                }
                opencat_core::resource::fonts::FontSource::Url(url) => {
                    let id = AssetId(font_asset_id(
                        &opencat_core::resource::fonts::FontSource::Url(url.clone()),
                    ));
                    let bytes = self
                        .runtime
                        .block_on(self.fetcher.fetch_bytes(&id, url))
                        .with_context(|| format!("fetch font `{}` url `{url}`", face.id))?;
                    let path = cache_file_path(&cache_dir, &id);
                    std::fs::write(&path, &bytes)?;
                    bytes
                }
            };
            out.insert(face.id.clone(), bytes);
        }
        Ok(out)
    }

    pub fn new(base_dir: PathBuf, cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir).ok();
        Ok(Self {
            fetcher: EngineFetcher::new(cache_dir.clone())?,
            _base_dir: base_dir,
            cache_dir,
            runtime: build_preload_runtime("engine-loader")?,
            handles: HashMap::new(),
            resource_provider: None,
        })
    }

    /// Register font files in the handle map under [`font_asset_id`] keys.
    pub fn register_font_handles(
        &mut self,
        manifest: &FontManifest,
        bytes_by_id: &std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<()> {
        for face in &manifest.faces {
            let bytes = bytes_by_id
                .get(&face.id)
                .with_context(|| format!("font `{}` bytes missing", face.id))?;
            let id = AssetId(font_asset_id(&face.source));
            let path = cache_file_path(&self.cache_dir, &id);
            std::fs::write(&path, bytes)
                .with_context(|| format!("write font cache {}", path.display()))?;
            self.handles.insert(id, EngineAssetHandle { cached_path: path });
        }
        Ok(())
    }

    /// Build [`MapResourceProvider`] from preloaded handles + [`ExternalResourceManifest`].
    pub fn build_resource_provider(
        &mut self,
        manifest: &ExternalResourceManifest,
    ) -> MapResourceProvider {
        let mut provider = MapResourceProvider::new();
        hydrate_provider_from_bytes(manifest, &mut provider, &LoaderByteSource(self));
        self.resource_provider = Some(provider.clone());
        provider
    }
}

impl AssetLoader for EngineLoader {
    type Handle = EngineAssetHandle;

    fn load_all(&mut self, req: &ResourceRequests) -> Result<()> {
        let base_dir = self._base_dir.clone();
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
                        copy_local_to_cache(p, &base_dir, &cache_dir, &id)?;
                    }
                    ImageSource::Query(q) => {
                        let search_id = AssetId(format!("openverse:search:{}", q.query));
                        let search_url = build_openverse_search_url(q);
                        let search_bytes = self
                            .fetcher
                            .fetch_bytes(&search_id, &search_url)
                            .await
                            .with_context(|| {
                                format!("failed to query Openverse for {:?}", q.query)
                            })?;
                        let image_url = parse_openverse_response(&search_bytes)
                            .with_context(|| format!("bad Openverse response for {:?}", q.query))?;
                        let _ = self.fetcher.fetch_bytes(&id, &image_url).await?;
                    }
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
                        copy_local_to_cache(p, &base_dir, &cache_dir, &id)?;
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
                        copy_local_to_cache(p, &base_dir, &cache_dir, &id)?;
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
                        copy_local_to_cache(p, &base_dir, &cache_dir, &id)?;
                    }
                }
                new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
            }

            Ok::<_, anyhow::Error>(())
        })?;

        for (id, path) in new_handles {
            self.handles
                .insert(id, EngineAssetHandle { cached_path: path });
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

fn copy_local_to_cache(src: &Path, base_dir: &Path, cache_dir: &Path, id: &AssetId) -> Result<()> {
    let resolved = if src.is_relative() {
        base_dir.join(src)
    } else {
        src.to_path_buf()
    };
    let dst = cache_file_path(cache_dir, id);
    if dst.exists() {
        return Ok(());
    }
    std::fs::copy(&resolved, &dst)
        .with_context(|| format!("copy {} -> {}", resolved.display(), dst.display()))?;
    Ok(())
}

fn build_openverse_search_url(query: &opencat_core::parse::primitives::OpenverseQuery) -> String {
    let page_size = query.count.max(1).to_string();
    let mut url = format!(
        "https://api.openverse.org/v1/images/?q={}&page_size={}",
        query.query, page_size
    );
    if let Some(aspect_ratio) = &query.aspect_ratio {
        url.push_str(&format!("&aspect_ratio={}", aspect_ratio));
    }
    url
}

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
