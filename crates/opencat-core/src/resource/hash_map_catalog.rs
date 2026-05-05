use std::collections::HashMap;
use anyhow::Result;
use crate::resource::asset_id::AssetId;
use crate::resource::catalog::{ResourceCatalog, VideoInfoMeta};
use crate::scene::primitives::{AudioSource, ImageSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMeta {
    pub width: u32,
    pub height: u32,
    pub kind: ResourceKind,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    Image,
    Video,
    Audio,
}

/// Catalog built from JS-preloaded resource metadata.
/// AssetId uses string keys matching resource locators.
pub struct HashMapResourceCatalog {
    entries: HashMap<AssetId, ResourceMeta>,
    asset_cache: HashMap<String, AssetId>,
}

impl HashMapResourceCatalog {
    /// Build catalog from JSON string:
    /// `{ "path/to/image.png": { "width": 800, "height": 600, "kind": "image" }, ... }`
    pub fn from_json(json: &str) -> Result<Self> {
        let map: HashMap<String, ResourceMeta> = serde_json::from_str(json)?;
        let mut catalog = Self {
            entries: HashMap::new(),
            asset_cache: HashMap::new(),
        };
        for (locator, meta) in map {
            let id = AssetId(locator.clone());
            catalog.asset_cache.insert(locator.clone(), id.clone());
            catalog.entries.insert(id, meta);
        }
        Ok(catalog)
    }

    fn resolve_key(&mut self, key: &str) -> AssetId {
        if let Some(id) = self.asset_cache.get(key) {
            return id.clone();
        }
        let id = AssetId(key.to_string());
        self.asset_cache.insert(key.to_string(), id.clone());
        id
    }
}

impl ResourceCatalog for HashMapResourceCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId> {
        let key = match src {
            ImageSource::Unset => "unset".to_string(),
            ImageSource::Path(p) => p.to_string_lossy().to_string(),
            ImageSource::Url(u) => u.clone(),
            ImageSource::Query(q) => format!("query:{}", q.query),
        };
        Ok(self.resolve_key(&key))
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId> {
        let key = match src {
            AudioSource::Unset => "unset".to_string(),
            AudioSource::Path(p) => p.to_string_lossy().to_string(),
            AudioSource::Url(u) => u.clone(),
        };
        Ok(self.resolve_key(&key))
    }

    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let id = self.resolve_key(locator);
        self.entries.entry(id.clone()).or_insert(ResourceMeta {
            width,
            height,
            kind: ResourceKind::Image,
            duration_secs: None,
        });
        id
    }

    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        let meta = self.entries.get(target).cloned();
        if let Some(m) = meta {
            self.entries.insert(alias, m);
        }
        Ok(())
    }

    fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.entries
            .get(id)
            .map(|m| (m.width, m.height))
            .unwrap_or((0, 0))
    }

    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.entries.get(id).and_then(|m| {
            if m.kind == ResourceKind::Video {
                Some(VideoInfoMeta {
                    width: m.width,
                    height: m.height,
                    duration_secs: m.duration_secs,
                })
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::catalog::ResourceCatalog;
    use crate::scene::primitives::ImageSource;
    use std::path::PathBuf;

    #[test]
    fn from_json_parses_and_resolves() {
        let json = r#"{"/img/a.png":{"width":100,"height":200,"kind":"image"}}"#;
        let mut catalog = HashMapResourceCatalog::from_json(json).unwrap();
        let src = ImageSource::Path(PathBuf::from("/img/a.png"));
        let id = catalog.resolve_image(&src).unwrap();
        assert_eq!(catalog.dimensions(&id), (100, 200));
    }

    #[test]
    fn video_info_returns_duration() {
        let json = r#"{"/v/b.mp4":{"width":1920,"height":1080,"kind":"video","durationSecs":5.5}}"#;
        let mut catalog = HashMapResourceCatalog::from_json(json).unwrap();
        let src = ImageSource::Path(PathBuf::from("/v/b.mp4"));
        let id = catalog.resolve_image(&src).unwrap();
        let info = catalog.video_info(&id).unwrap();
        assert_eq!(info.width, 1920);
        assert_eq!(info.duration_secs, Some(5.5));
    }

    #[test]
    fn unknown_resource_returns_zero_dimensions() {
        let catalog = HashMapResourceCatalog::from_json("{}").unwrap();
        assert_eq!(catalog.dimensions(&AssetId("unknown".to_string())), (0, 0));
    }
}
