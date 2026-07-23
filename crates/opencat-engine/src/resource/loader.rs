//! [`EngineAssetHandle`] + [`EngineLoader`] — the engine's host-owned resource
//! cache. They no longer implement core loader traits (issue #2 / #11): core is
//! a pure derivation kernel and the engine is its own host, so these are plain
//! concrete types with inherent lookup methods.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use opencat_core::ir::asset_id::{
    AssetId, ResourceKind, asset_id_for_audio, asset_id_for_image,
    asset_id_for_lottie, asset_id_for_subtitle, asset_id_for_url,
    asset_id_for_video,
};
use opencat_core::parse::primitives::{AudioSource, ImageSource, LottieSource, SubtitleSource, VideoSource};
use opencat_core::probe::catalog::ResourceRequests;
use opencat_core::fonts::{FontManifest, font_asset_id};

use crate::resource::fetch::{EngineFetcher, build_preload_runtime};
use crate::resource::utils::cache_file_path;

#[derive(Clone)]
pub struct EngineAssetHandle {
    pub(crate) cached_path: PathBuf,
}

impl EngineAssetHandle {
    /// Read the cached bytes for this asset from disk.
    pub fn read_bytes(&self) -> Result<Cow<'_, [u8]>> {
        let bytes = std::fs::read(&self.cached_path)
            .with_context(|| format!("read {}", self.cached_path.display()))?;
        Ok(Cow::Owned(bytes))
    }

    /// Local filesystem path backing this asset.
    pub fn local_path(&self) -> Option<&Path> {
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
        let cache_dir = self.cache_dir.clone();
        let base_dir = self._base_dir.clone();
        let mut out = std::collections::HashMap::new();
        for face in &manifest.faces {
            let bytes = match &face.source {
                opencat_core::fonts::FontSource::Path(path) => {
                    // Manifest path is a logical locator; join document base when relative.
                    let resolved = if path.is_absolute() {
                        path.clone()
                    } else {
                        base_dir.join(path)
                    };
                    std::fs::read(&resolved).with_context(|| {
                        format!("read font `{}` from {}", face.id, resolved.display())
                    })?
                }
                opencat_core::fonts::FontSource::Url(url) => {
                    let id = AssetId::new(
                        ResourceKind::Font,
                        font_asset_id(&opencat_core::fonts::FontSource::Url(url.clone())),
                    );
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
            let id = AssetId::new(ResourceKind::Font, font_asset_id(&face.source));
            let path = cache_file_path(&self.cache_dir, &id);
            std::fs::write(&path, bytes)
                .with_context(|| format!("write font cache {}", path.display()))?;
            self.handles
                .insert(id, EngineAssetHandle { cached_path: path });
        }
        Ok(())
    }

    /// Register canvas `asset_id` aliases so `ctx.getImage("hero")` resolves via loader handles.
    ///
    /// Walk the scene after preload and register canvas `asset_id` aliases so
    /// path/url assets (loaded under content ids) also resolve by user-facing alias.
    pub fn register_canvas_asset_aliases(
        &mut self,
        composition: &opencat_core::parse::composition::Composition,
    ) {
        use opencat_core::frame_ctx::FrameCtx;
        use opencat_core::parse::node::{Node, NodeKind};
        use opencat_core::parse::primitives::ImageSource;
        use opencat_core::parse::time::{FrameState, TimelineSegment, frame_state_for_root};

        fn register_from_node(loader: &mut EngineLoader, node: &Node) {
            match node.kind() {
                NodeKind::Div(div) => {
                    for child in div.children_ref() {
                        register_from_node(loader, child);
                    }
                }
                NodeKind::Canvas(canvas) => {
                    for asset in canvas.assets_ref() {
                        if let ImageSource::Path(ref path) = asset.source {
                            let target = image_asset_id(&ImageSource::Path(path.clone()));
                            let alias = AssetId::new(ResourceKind::Image, asset.asset_id.clone());
                            if let Some(handle) = loader.handles.get(&target).cloned() {
                                loader.handles.entry(alias).or_insert(handle);
                            }
                        }
                    }
                    for child in canvas.hidden_children_ref() {
                        register_from_node(loader, child);
                    }
                }
                NodeKind::Video(video) => {
                    for child in video.children_ref() {
                        register_from_node(loader, child);
                    }
                }
                NodeKind::Timeline(timeline) => {
                    for segment in timeline.segments() {
                        match segment {
                            TimelineSegment::Scene { scene, .. } => {
                                register_from_node(loader, scene);
                            }
                            TimelineSegment::Transition { from, to, .. } => {
                                register_from_node(loader, from);
                                register_from_node(loader, to);
                            }
                        }
                    }
                }
                NodeKind::Image(_)
                | NodeKind::Text(_)
                | NodeKind::Lucide(_)
                | NodeKind::Path(_)
                | NodeKind::Lottie(_)
                | NodeKind::Caption(_) => {}
            }
        }

        for frame in 0..composition.frames.max(1) {
            let frame_ctx = FrameCtx {
                frame,
                fps: composition.fps,
                width: composition.width,
                height: composition.height,
                frames: composition.frames,
            };
            let root = composition.root_node(&frame_ctx);
            match frame_state_for_root(&root, &frame_ctx) {
                FrameState::Scene { scene, .. } => register_from_node(self, &scene),
                FrameState::Transition { from, to, .. } => {
                    register_from_node(self, &from);
                    register_from_node(self, &to);
                }
            }
        }
    }

    /// Collect decoded SRT text for a single subtitle `AssetId`, for
    /// core's pure `hydrate_captions`. Returns `None` when no bytes are cached.
    ///
    /// After `load_all` has fetched/cached the subtitle file, the host reads bytes,
    /// decodes them as UTF-8, and hands the text to `insert_subtitle_text`. Core
    /// never opens a subtitle file.
    pub fn srt_text_for_subtitle_id(&self, id: &AssetId) -> Option<String> {
        self.handle(id)
            .and_then(|h| h.read_bytes().ok())
            .and_then(|bytes| String::from_utf8(bytes.into_owned()).ok())
    }
}

impl EngineLoader {
    pub fn load_all(&mut self, req: &ResourceRequests) -> Result<()> {
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
                        copy_local_to_cache(std::path::Path::new(p), &base_dir, &cache_dir, &id)?;
                    }
                    ImageSource::Query(q) => {
                        let search_id =
                            AssetId::new(ResourceKind::Image, format!("openverse:search:{}", q.query));
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
                        copy_local_to_cache(Path::new(p), &base_dir, &cache_dir, &id)?;
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

            for lottie_req in &req.lotties {
                if matches!(lottie_req.source, LottieSource::Unset) {
                    continue;
                }
                let id = lottie_asset_id(&lottie_req.source);
                match &lottie_req.source {
                    LottieSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                    }
                    LottieSource::Path(p) => {
                        // Logical locator — host joins document base.
                        copy_local_to_cache(std::path::Path::new(p), &base_dir, &cache_dir, &id)?;
                    }
                    LottieSource::Unset => continue,
                }
                let cached_path = cache_file_path(&cache_dir, &id);
                new_handles.push((id.clone(), cached_path.clone()));
                // Also register under the canonical bundle id from core
                // (`asset_id_for_lottie`) so DrawOp::LottieRect / FrameMediaPlan
                // can resolve bytes without re-deriving the scheme.
                let bundle_id = asset_id_for_lottie(&lottie_req.source)
                    .expect("Unset LottieSource was already filtered above");
                new_handles.push((bundle_id.clone(), cached_path.clone()));

                // Host-only: scan primary JSON for external deps and cache them
                // under `{bundle_id}:dep:{basename}` (same shape as web BlobStore).
                // Core prepare only receives LottieMeta.dependencies, never these bytes.
                if let Ok(primary) = std::fs::read(&cached_path) {
                    if let Ok(json) = std::str::from_utf8(&primary) {
                        if let Ok(deps) =
                            crate::probe::scan_lottie_dependencies(json)
                        {
                            let primary_dir = match &lottie_req.source {
                                LottieSource::Path(p) => {
                                    let full = if std::path::Path::new(p).is_relative() {
                                        base_dir.join(p)
                                    } else {
                                        std::path::PathBuf::from(p)
                                    };
                                    full.parent().map(|d| d.to_path_buf())
                                }
                                _ => None,
                            };
                            for file_name in deps {
                                let dep_id = AssetId::new(
                                    ResourceKind::Lottie,
                                    format!("{}:dep:{}", bundle_id.key, file_name),
                                );
                                if file_name.starts_with("http://")
                                    || file_name.starts_with("https://")
                                {
                                    let _ =
                                        self.fetcher.fetch_bytes(&dep_id, &file_name).await?;
                                    let path = cache_file_path(&cache_dir, &dep_id);
                                    new_handles.push((dep_id, path));
                                } else if let Some(dir) = &primary_dir {
                                    // Try sibling of JSON, then images/ under that dir.
                                    let candidates = [
                                        dir.join(&file_name),
                                        dir.join("images").join(&file_name),
                                        base_dir.join(&file_name),
                                        base_dir.join("images").join(&file_name),
                                    ];
                                    if let Some(src) =
                                        candidates.into_iter().find(|c| c.is_file())
                                    {
                                        copy_local_to_cache(
                                            &src,
                                            &base_dir,
                                            &cache_dir,
                                            &dep_id,
                                        )?;
                                        let path = cache_file_path(&cache_dir, &dep_id);
                                        new_handles.push((dep_id, path));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok::<_, anyhow::Error>(())
        })?;

        for (id, path) in new_handles {
            self.handles
                .insert(id, EngineAssetHandle { cached_path: path });
        }
        Ok(())
    }

    pub fn handle(&self, id: &AssetId) -> Option<&EngineAssetHandle> {
        self.handles.get(id)
    }
}

fn image_asset_id(s: &ImageSource) -> AssetId {
    // Always use core's canonical rule — never re-derive (#15).
    asset_id_for_image(s).unwrap_or_else(|| AssetId::new(ResourceKind::Image, String::new()))
}

fn video_asset_id(s: &VideoSource) -> AssetId {
    asset_id_for_video(s)
}

fn audio_asset_id(s: &AudioSource) -> AssetId {
    asset_id_for_audio(s).unwrap_or_else(|| AssetId::new(ResourceKind::Audio, String::new()))
}

fn subtitle_asset_id(s: &SubtitleSource) -> AssetId {
    asset_id_for_subtitle(s)
}

/// Lottie probe byte key. Returns `Image` kind intentionally — the probe
/// phase reads Lottie primary JSON by logical path / URL, which live in the
/// same string-keyed byte map as image path ids. The typed [`AssetId`] for
/// Lottie draw ops / FrameMediaPlan is created separately via
/// [`asset_id_for_lottie`] with `ResourceKind::Lottie` (see `load_all`).
/// # Rename guard
/// If this helper's name doesn't communicate "probe byte key, not bundle id",
/// rename it — keeping the caller correct is more important than the name.
fn lottie_asset_id(s: &LottieSource) -> AssetId {
    match s {
        LottieSource::Unset => AssetId::new(ResourceKind::Image, String::new()),
        // Probe byte key is the logical path string (same as Image path ids).
        LottieSource::Path(p) => AssetId::new(ResourceKind::Image, p.clone()),
        LottieSource::Url(u) => asset_id_for_url(u),
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
    use opencat_core::fonts::{FontFaceDecl, FontSource};

    #[test]
    fn load_all_with_local_path_registers_handle() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();

        let mut loader = EngineLoader::new(tmp.path().to_path_buf(), cache.clone()).unwrap();

        let test_file = tmp.path().join("test.txt");
        std::fs::write(&test_file, b"hello").unwrap();

        let mut req = ResourceRequests::default();
        // Host resolves the logical locator against its document base; the source
        // itself stays logical (relative to base_dir).
        let logical = "test.txt".to_string();
        req.videos.insert(VideoSource::Path(logical.clone()));

        loader.load_all(&req).unwrap();

        let id = AssetId::new(ResourceKind::Video, format!("video:path:{logical}"));
        let h = loader.handle(&id).unwrap();
        assert!(h.local_path().is_some());
        assert!(h.local_path().unwrap().exists());
    }

    #[test]
    fn load_font_manifest_joins_logical_path_against_base_dir() {
        let tmp = tempfile::tempdir_in(".").unwrap();
        let base_dir = tmp.path().join("examples");
        let assets_dir = tmp.path().join("assets");
        let cache_dir = tmp.path().join("cache");
        std::fs::create_dir_all(&base_dir).unwrap();
        std::fs::create_dir_all(&assets_dir).unwrap();

        // Logical relative path from document base (examples/) → ../assets/test.otf
        std::fs::write(assets_dir.join("test.otf"), b"font bytes").unwrap();
        let manifest = FontManifest {
            default_face_id: Some("sans".to_string()),
            faces: vec![FontFaceDecl {
                id: "sans".to_string(),
                family: Some("Test Sans".to_string()),
                source: FontSource::Path(std::path::PathBuf::from("../assets/test.otf")),
                role: None,
            }],
        };
        let mut loader = EngineLoader::new(base_dir, cache_dir).unwrap();

        let fonts = loader.load_font_manifest(&manifest).unwrap();

        assert_eq!(fonts.get("sans").map(Vec::as_slice), Some(b"font bytes".as_slice()));
    }
}
