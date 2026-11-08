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
    asset_id_for_lottie, asset_id_for_url,
    asset_id_for_video,
};
use opencat_core::parse::primitives::{AudioSource, ImageSource, LottieSource, VideoSource};
use opencat_core::probe::catalog::ResourceRequests;
use opencat_core::fonts::{FontManifest, font_asset_id};

use crate::resource::fetch::{EngineFetcher, build_preload_runtime};
use crate::resource::path_store::cache_file_path;

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
                | NodeKind::Path(_)
                | NodeKind::Svg(_)
                | NodeKind::Lottie(_) => {}
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

}

impl EngineLoader {
    pub fn load_all(&mut self, req: &ResourceRequests) -> Result<()> {
        let base_dir = self._base_dir.clone();
        let cache_dir = self.cache_dir.clone();
        let mut new_handles: Vec<(AssetId, PathBuf)> = Vec::new();

        self.runtime.block_on(async {
            for src in &req.images {
                let id = image_asset_id(src);
                let loaded = match src {
                    ImageSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                        true
                    }
                    ImageSource::Path(p) => {
                        copy_local_to_cache(std::path::Path::new(p), &base_dir, &cache_dir, &id)?
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
                        true
                    }
                    ImageSource::Unset => false,
                };
                if loaded {
                    new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
                }
            }

            for src in &req.videos {
                let id = video_asset_id(src);
                let loaded = match src {
                    VideoSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                        true
                    }
                    VideoSource::Path(p) => {
                        copy_local_to_cache(Path::new(p), &base_dir, &cache_dir, &id)?
                    }
                };
                if loaded {
                    new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
                }
            }

            for src in &req.audios {
                let id = audio_asset_id(src);
                let loaded = match src {
                    AudioSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                        true
                    }
                    AudioSource::Path(p) => {
                        copy_local_to_cache(p, &base_dir, &cache_dir, &id)?
                    }
                    AudioSource::Unset => false,
                };
                if loaded {
                    new_handles.push((id.clone(), cache_file_path(&cache_dir, &id)));
                }
            }

            for lottie_req in &req.lotties {
                if matches!(lottie_req.source, LottieSource::Unset) {
                    continue;
                }
                let id = lottie_asset_id(&lottie_req.source);
                let loaded = match &lottie_req.source {
                    LottieSource::Url(u) => {
                        let _ = self.fetcher.fetch_bytes(&id, u).await?;
                        true
                    }
                    LottieSource::Path(p) => {
                        // Logical locator — host joins document base.
                        copy_local_to_cache(std::path::Path::new(p), &base_dir, &cache_dir, &id)?
                    }
                    LottieSource::Unset => continue,
                };
                if !loaded {
                    continue;
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

    /// 缺失图片资产的占位：1×1 透明 PNG 进缓存并注册 handle。
    ///
    /// 浏览器语义里 `<img>` 404 是 onerror + 空内容，页面继续渲染；prepare
    /// 的契约（Image 必须正尺寸）由占位满足，视觉上不可见。页面脚本对
    /// `complete`/`naturalWidth` 的探测走 JS 面，不读这里的字节。
    pub fn register_missing_placeholder(&mut self, id: &AssetId) -> Result<()> {
        if self.handles.contains_key(id) {
            return Ok(());
        }
        let png = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        let mut bytes = Vec::new();
        png.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .context("encode 1x1 placeholder png")?;
        let path = cache_file_path(&self.cache_dir, id);
        std::fs::write(&path, bytes)
            .with_context(|| format!("write image placeholder {}", path.display()))?;
        self.handles
            .insert(id.clone(), EngineAssetHandle { cached_path: path });
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

/// 本地资产进缓存。返回 `false` = 源文件不存在（已跳过）：浏览器语义里
/// `<img>`/`<video>` 404 是 onerror + 空内容，页面脚本普遍探测
/// `complete`/`naturalWidth` 再消费，不能让整个渲染硬失败。
fn copy_local_to_cache(src: &Path, base_dir: &Path, cache_dir: &Path, id: &AssetId) -> Result<bool> {
    let resolved = if src.is_relative() {
        base_dir.join(src)
    } else {
        src.to_path_buf()
    };
    let dst = cache_file_path(cache_dir, id);
    if !resolved.exists() {
        eprintln!(
            "opencat: local asset not found, rendering without it: {}",
            resolved.display()
        );
        return Ok(false);
    }
    if dst.exists() && cached_copy_is_current(&dst, &resolved) {
        return Ok(true);
    }
    std::fs::copy(&resolved, &dst)
        .with_context(|| format!("copy {} -> {}", resolved.display(), dst.display()))?;
    write_cache_stamp(&dst, &resolved);
    Ok(true)
}

/// Sidecar recording which source file a cached blob was copied from.
///
/// `AssetId` keys path assets by their *logical locator* ("video:path:assets/
/// bg.mp4"), so two projects that both ship an `assets/bg.mp4` share one cache
/// slot. Reusing a stale blob silently renders the wrong video — this stamp
/// lets a cache hit prove the blob still matches the file on disk.
fn cache_stamp_path(cached: &Path) -> PathBuf {
    let mut p = cached.as_os_str().to_owned();
    p.push(".src");
    PathBuf::from(p)
}

fn source_signature(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Some(format!("{}\n{}\n{}", path.display(), meta.len(), mtime))
}

fn cached_copy_is_current(cached: &Path, source: &Path) -> bool {
    let stamp = match std::fs::read_to_string(cache_stamp_path(cached)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    source_signature(source).is_some_and(|sig| sig == stamp)
}

fn write_cache_stamp(cached: &Path, source: &Path) {
    if let Some(sig) = source_signature(source) {
        let _ = std::fs::write(cache_stamp_path(cached), sig);
    }
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


