use crate::ir::asset_id::{AssetId, asset_id_for_query};
use crate::parse::primitives::{AudioSource, ImageSource};
use crate::resource::lottie::LottieMeta;
use crate::resource::catalog::{ResourceCatalog, VideoInfoMeta};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMeta {
    pub width: u32,
    pub height: u32,
    pub kind: ResourceKind,
    pub duration_secs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lottie_fps: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lottie_duration_frames: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    Image,
    Video,
    Audio,
    Lottie,
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

    /// Serialize entries to the same JSON shape `from_json` accepts.
    /// Useful for handing catalog state across an FFI boundary (e.g. wasm → JS).
    pub fn to_json(&self) -> Result<String> {
        let map: HashMap<&str, &ResourceMeta> = self
            .entries
            .iter()
            .map(|(id, meta)| (id.0.as_str(), meta))
            .collect();
        Ok(serde_json::to_string(&map)?)
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
            ImageSource::Url(u) => format!("url:{u}"),
            ImageSource::Query(q) => asset_id_for_query(q).0,
        };
        Ok(self.resolve_key(&key))
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId> {
        let key = match src {
            AudioSource::Unset => "unset".to_string(),
            AudioSource::Path(p) => p.to_string_lossy().to_string(),
            AudioSource::Url(u) => format!("audio:url:{u}"),
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
            lottie_fps: None,
            lottie_duration_frames: None,
        });
        id
    }

    fn register_video_dimensions(
        &mut self,
        locator: &str,
        width: u32,
        height: u32,
        duration_secs: Option<f64>,
    ) -> AssetId {
        let id = self.resolve_key(locator);
        self.entries.entry(id.clone()).or_insert(ResourceMeta {
            width,
            height,
            kind: ResourceKind::Video,
            duration_secs,
            lottie_fps: None,
            lottie_duration_frames: None,
        });
        id
    }

    fn register_audio(&mut self, locator: &str) -> AssetId {
        let id = self.resolve_key(locator);
        self.entries.entry(id.clone()).or_insert(ResourceMeta {
            width: 0,
            height: 0,
            kind: ResourceKind::Audio,
            duration_secs: None,
            lottie_fps: None,
            lottie_duration_frames: None,
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

    fn resolve_lottie(&mut self, element_id: &str) -> Result<AssetId> {
        Ok(self.resolve_key(&format!("lottie:{element_id}")))
    }

    fn lottie_meta(&self, id: &AssetId) -> Option<LottieMeta> {
        HashMapResourceCatalog::lottie_meta(self, id)
    }
}

impl HashMapResourceCatalog {
    pub fn register_lottie(&mut self, locator: &str, meta: LottieMeta) -> AssetId {
        let id = self.resolve_key(locator);
        self.entries.entry(id.clone()).or_insert(ResourceMeta {
            width: meta.width,
            height: meta.height,
            kind: ResourceKind::Lottie,
            duration_secs: Some(meta.duration_frames() as f64 / meta.fps.max(1.0) as f64),
            lottie_fps: Some(meta.fps),
            lottie_duration_frames: Some(meta.duration_frames()),
        });
        id
    }

    pub fn lottie_meta(&self, id: &AssetId) -> Option<LottieMeta> {
        self.entries.get(id).and_then(|m| {
            if m.kind != ResourceKind::Lottie {
                return None;
            }
            Some(LottieMeta {
                width: m.width,
                height: m.height,
                fps: m.lottie_fps.unwrap_or(30.0),
                in_frame: 0.0,
                out_frame: m.lottie_duration_frames.unwrap_or(1) as f32,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::primitives::{AudioSource, ImageSource, OpenverseQuery};
    use crate::resource::catalog::ResourceCatalog;
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
    fn resolves_image_url_with_prefix() {
        let json = r#"{"url:https://example.com/a.png":{"width":800,"height":600,"kind":"image"}}"#;
        let mut catalog = HashMapResourceCatalog::from_json(json).unwrap();
        let src = ImageSource::Url("https://example.com/a.png".to_string());
        let id = catalog.resolve_image(&src).unwrap();
        assert_eq!(catalog.dimensions(&id), (800, 600));
    }

    #[test]
    fn resolves_image_query() {
        let json = r#"{"openverse:q=cats;count=3":{"width":640,"height":480,"kind":"image"}}"#;
        let mut catalog = HashMapResourceCatalog::from_json(json).unwrap();
        let q = OpenverseQuery {
            query: "cats".into(),
            count: 3,
            aspect_ratio: None,
        };
        let src = ImageSource::Query(q);
        let id = catalog.resolve_image(&src).unwrap();
        assert_eq!(catalog.dimensions(&id), (640, 480));
    }

    #[test]
    fn resolves_audio_url_with_prefix() {
        let json =
            r#"{"audio:url:https://example.com/music.mp3":{"width":0,"height":0,"kind":"audio"}}"#;
        let mut catalog = HashMapResourceCatalog::from_json(json).unwrap();
        let src = AudioSource::Url("https://example.com/music.mp3".to_string());
        let id = catalog.resolve_audio(&src).unwrap();
        assert_eq!(catalog.dimensions(&id), (0, 0));
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
