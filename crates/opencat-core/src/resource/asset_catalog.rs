use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::resource::catalog::{ResourceCatalog, VideoInfoMeta};
use crate::scene::primitives::{AudioSource, ImageSource, OpenverseQuery};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(pub String);

pub struct AssetCatalog {
    pub(crate) entries: HashMap<AssetId, AssetEntry>,
    pub(crate) video_info_meta: HashMap<AssetId, VideoInfoMeta>,
    pub(crate) cache_dir: PathBuf,
    pub(crate) openverse_token: Option<String>,
}

pub(crate) struct AssetEntry {
    path: PathBuf,
    width: u32,
    height: u32,
}

impl AssetEntry {
    pub(crate) fn image(path: &Path) -> Self {
        let (width, height) = read_image_dimensions(path);
        Self {
            path: path.to_path_buf(),
            width,
            height,
        }
    }

    pub(crate) fn audio(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            width: 0,
            height: 0,
        }
    }

    pub(crate) fn with_dimensions(path: PathBuf, width: u32, height: u32) -> Self {
        Self {
            path,
            width,
            height,
        }
    }
}

impl AssetCatalog {
    pub fn new() -> Self {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".opencat")
            .join("assets");
        Self {
            entries: HashMap::new(),
            video_info_meta: HashMap::new(),
            cache_dir,
            openverse_token: None,
        }
    }

    pub fn register(&mut self, path: &Path) -> AssetId {
        let id = AssetId(path.to_string_lossy().into_owned());
        self.insert_entry_if_missing(id, || AssetEntry::image(path))
    }

    pub fn register_image_source(&mut self, source: &ImageSource) -> Result<AssetId> {
        match source {
            ImageSource::Unset => Err(anyhow!("image source is required before rendering")),
            ImageSource::Path(path) => Ok(self.register(path)),
            ImageSource::Url(url) => {
                let id = asset_id_for_url(url);
                self.require_preloaded(id, || {
                    anyhow!("remote image source {url} was not preloaded before rendering")
                })
            }
            ImageSource::Query(query) => {
                let id = asset_id_for_query(query);
                self.require_preloaded(id, || {
                    anyhow!(
                        "Openverse query {:?} was not preloaded before rendering",
                        query.query
                    )
                })
            }
        }
    }

    pub fn ensure_image_source_entry_for_inspect(&mut self, source: &ImageSource) {
        match source {
            ImageSource::Unset => {}
            ImageSource::Path(path) => {
                let _ = self.register(path);
            }
            ImageSource::Url(url) => {
                let id = asset_id_for_url(url);
                if self.entries.contains_key(&id) {
                    return;
                }
                self.entries
                    .insert(id, AssetEntry::with_dimensions(PathBuf::from(url), 0, 0));
            }
            ImageSource::Query(query) => {
                let id = asset_id_for_query(query);
                if self.entries.contains_key(&id) {
                    return;
                }
                self.entries.insert(
                    id,
                    AssetEntry::with_dimensions(
                        PathBuf::from(format!("openverse://{}", query.query)),
                        0,
                        0,
                    ),
                );
            }
        }
    }

    pub fn register_audio_source(&mut self, source: &AudioSource) -> Result<AssetId> {
        match source {
            AudioSource::Unset => Err(anyhow!("audio source is required before rendering")),
            AudioSource::Path(path) => Ok(self.register_audio_path(path)),
            AudioSource::Url(url) => {
                let id = asset_id_for_audio_url(url);
                self.require_preloaded(id, || {
                    anyhow!("remote audio source {url} was not preloaded before rendering")
                })
            }
        }
    }

    pub fn register_dimensions(&mut self, path: &Path, width: u32, height: u32) -> AssetId {
        let id = AssetId(path.to_string_lossy().into_owned());
        self.insert_entry_if_missing(id, || {
            AssetEntry::with_dimensions(path.to_path_buf(), width, height)
        })
    }

    pub fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        if self.entries.contains_key(&alias) {
            return Ok(());
        }

        let entry = self
            .entries
            .get(target)
            .ok_or_else(|| anyhow!("cannot alias missing asset {}", target.0))?;
        self.entries.insert(
            alias,
            AssetEntry {
                path: entry.path.clone(),
                width: entry.width,
                height: entry.height,
            },
        );
        Ok(())
    }

    pub fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.entries
            .get(id)
            .map(|e| (e.width, e.height))
            .unwrap_or((0, 0))
    }

    pub fn register_video_info(&mut self, path: &Path, info: VideoInfoMeta) -> AssetId {
        let id = self.register_dimensions(path, info.width, info.height);
        self.video_info_meta.insert(id.clone(), info);
        id
    }

    pub fn video_info_meta(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.video_info_meta.get(id).copied()
    }

    pub fn path(&self, id: &AssetId) -> Option<&Path> {
        self.entries.get(id).map(|e| e.path.as_path())
    }

    pub(crate) fn register_audio_path(&mut self, path: &Path) -> AssetId {
        let id = asset_id_for_audio_path(path);
        self.insert_entry_if_missing(id, || AssetEntry::audio(path))
    }

    pub(crate) fn insert_entry_if_missing(
        &mut self,
        id: AssetId,
        build_entry: impl FnOnce() -> AssetEntry,
    ) -> AssetId {
        if self.entries.contains_key(&id) {
            return id;
        }

        self.entries.insert(id.clone(), build_entry());
        id
    }

    pub(crate) fn push_missing_request<T>(
        &self,
        id: &AssetId,
        requests: &mut Vec<T>,
        build_request: impl FnOnce() -> T,
    ) {
        if !self.entries.contains_key(id) {
            requests.push(build_request());
        }
    }

    pub(crate) fn require_preloaded(
        &self,
        id: AssetId,
        missing_error: impl FnOnce() -> anyhow::Error,
    ) -> Result<AssetId> {
        self.entries
            .contains_key(&id)
            .then_some(id)
            .ok_or_else(missing_error)
    }

    pub(crate) fn ensure_cache_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).with_context(|| {
            format!(
                "failed to create asset cache dir {}",
                self.cache_dir.display()
            )
        })
    }
}

impl Default for AssetCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceCatalog for AssetCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId> {
        self.register_image_source(src)
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId> {
        self.register_audio_source(src)
    }

    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let path = std::path::Path::new(locator);
        AssetCatalog::register_dimensions(self, path, width, height)
    }

    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        AssetCatalog::alias(self, alias, target)
    }

    fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        AssetCatalog::dimensions(self, id)
    }

    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.video_info_meta(id)
    }
}

pub fn asset_id_for_url(url: &str) -> AssetId {
    AssetId(format!("url:{url}"))
}

pub fn asset_id_for_audio_path(path: &Path) -> AssetId {
    AssetId(format!("audio:path:{}", path.to_string_lossy()))
}

pub fn asset_id_for_audio_url(url: &str) -> AssetId {
    AssetId(format!("audio:url:{url}"))
}

pub fn asset_id_for_query(query: &OpenverseQuery) -> AssetId {
    AssetId(format!(
        "openverse:q={};count={};aspect_ratio={}",
        query.query,
        query.count,
        query.aspect_ratio.as_deref().unwrap_or("")
    ))
}

pub fn cache_file_path(cache_dir: &Path, id: &AssetId, extension: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.{extension}", stable_hash(&id.0)))
}

pub fn read_image_dimensions(path: &Path) -> (u32, u32) {
    let Ok(bytes) = fs::read(path) else {
        return (0, 0);
    };
    let Ok(image) = image::load_from_memory(&bytes) else {
        return (0, 0);
    };
    (image.width(), image.height())
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_returns_stable_id_for_same_path() {
        let mut catalog = AssetCatalog::new();
        let id1 = catalog.register(Path::new("/tmp/a.png"));
        let id2 = catalog.register(Path::new("/tmp/a.png"));
        assert_eq!(id1, id2);
    }

    #[test]
    fn alias_copies_entry_dimensions() {
        let mut catalog = AssetCatalog::new();
        let target = catalog.register_dimensions(Path::new("/tmp/t.png"), 100, 200);
        catalog
            .alias(AssetId("alias".into()), &target)
            .unwrap();
        assert_eq!(catalog.dimensions(&AssetId("alias".into())), (100, 200));
        assert_eq!(
            catalog.path(&AssetId("alias".into())),
            catalog.path(&target)
        );
    }

    #[test]
    fn dimensions_returns_zero_for_missing_asset() {
        let catalog = AssetCatalog::new();
        assert_eq!(catalog.dimensions(&AssetId("nonexistent".into())), (0, 0));
    }

    #[test]
    fn video_info_returns_none_when_not_registered() {
        let catalog = AssetCatalog::new();
        assert!(catalog.video_info_meta(&AssetId("no-video".into())).is_none());
    }

    #[test]
    fn video_info_returns_values_when_registered() {
        let mut catalog = AssetCatalog::new();
        let info = VideoInfoMeta {
            width: 320,
            height: 240,
            duration_secs: Some(10.0),
        };
        let id = catalog.register_video_info(Path::new("/tmp/v.mp4"), info);
        let returned = catalog.video_info_meta(&id).unwrap();
        assert_eq!(returned.width, 320);
        assert_eq!(returned.height, 240);
        assert_eq!(returned.duration_secs, Some(10.0));
    }

    #[test]
    fn register_image_source_errors_on_unset() {
        let mut catalog = AssetCatalog::new();
        assert!(catalog
            .register_image_source(&ImageSource::Unset)
            .is_err());
    }

    #[test]
    fn register_audio_source_errors_on_unset() {
        let mut catalog = AssetCatalog::new();
        assert!(catalog
            .register_audio_source(&AudioSource::Unset)
            .is_err());
    }

    #[test]
    fn register_audio_source_errors_on_missing_preloaded_url() {
        let mut catalog = AssetCatalog::new();
        let result = catalog.register_audio_source(&AudioSource::Url("https://example.com/a.mp3".into()));
        assert!(result.is_err());
    }

    #[test]
    fn register_image_source_errors_on_missing_preloaded_url() {
        let mut catalog = AssetCatalog::new();
        let result = catalog.register_image_source(&ImageSource::Url("https://example.com/b.png".into()));
        assert!(result.is_err());
    }

    #[test]
    fn asset_id_for_url_is_deterministic() {
        let id1 = asset_id_for_url("https://example.com/a.png");
        let id2 = asset_id_for_url("https://example.com/a.png");
        assert_eq!(id1, id2);
    }

    #[test]
    fn asset_id_for_query_is_deterministic() {
        let q = OpenverseQuery {
            query: "cat".into(),
            count: 3,
            aspect_ratio: Some("wide".into()),
        };
        let id1 = asset_id_for_query(&q);
        let id2 = asset_id_for_query(&q);
        assert_eq!(id1, id2);
    }

    #[test]
    fn ensure_image_source_entry_for_inspect_inserts_placeholder_for_url() {
        let mut catalog = AssetCatalog::new();
        catalog.ensure_image_source_entry_for_inspect(&ImageSource::Url(
            "https://example.com/x.png".into(),
        ));
        let id = asset_id_for_url("https://example.com/x.png");
        assert_eq!(catalog.dimensions(&id), (0, 0));
        assert!(catalog.path(&id).is_some());
    }

    #[test]
    fn ensure_image_source_entry_for_inspect_inserts_placeholder_for_query() {
        let mut catalog = AssetCatalog::new();
        let query = OpenverseQuery {
            query: "dog".into(),
            count: 1,
            aspect_ratio: None,
        };
        catalog.ensure_image_source_entry_for_inspect(&ImageSource::Query(query));
        let qid = asset_id_for_query(&OpenverseQuery {
            query: "dog".into(),
            count: 1,
            aspect_ratio: None,
        });
        assert_eq!(catalog.dimensions(&qid), (0, 0));
    }
}
