use std::collections::{HashMap, HashSet};

use crate::parse::primitives::{AudioSource, ImageSource, SrtEntry, SubtitleSource};
pub use crate::parse::primitives::VideoSource;
pub use crate::resource::asset_id::AssetId;

#[derive(Default, Clone, Debug)]
pub struct ResourceRequests {
    pub images: HashSet<ImageSource>,
    pub videos: HashSet<VideoSource>,
    pub audios: HashSet<AudioSource>,
    pub subtitles: HashSet<SubtitleSource>,
}

#[derive(Default, Clone, Debug)]
pub struct ResourceCatalog {
    pub images: HashMap<AssetId, ImageMeta>,
    pub videos: HashMap<AssetId, VideoInfoMeta>,
    pub audios: HashSet<AssetId>,
    pub subtitles: HashMap<AssetId, Vec<SrtEntry>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageMeta {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,
}

#[derive(Default, Clone, Debug)]
pub struct AudioPlan {
    pub segments: Vec<AudioSegment>,
}

#[derive(Clone, Debug)]
pub struct AudioSegment {
    pub asset: AssetId,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl crate::resource::catalog::ResourceCatalog for ResourceCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> anyhow::Result<AssetId> {
        match src {
            ImageSource::Unset => anyhow::bail!("unset image source"),
            ImageSource::Url(u) => Ok(crate::resource::asset_id::asset_id_for_url(u)),
            ImageSource::Path(p) => Ok(AssetId(p.to_string_lossy().into_owned())),
            ImageSource::Query(q) => Ok(crate::resource::asset_id::asset_id_for_query(q)),
        }
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> anyhow::Result<AssetId> {
        match src {
            AudioSource::Unset => anyhow::bail!("unset audio source"),
            AudioSource::Url(u) => Ok(crate::resource::asset_id::asset_id_for_audio_url(u)),
            AudioSource::Path(p) => Ok(AssetId(format!("audio:path:{}", p.to_string_lossy()))),
        }
    }

    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let id = AssetId(locator.to_owned());
        self.images.entry(id.clone()).or_insert(ImageMeta { width, height });
        id
    }

    fn register_video_dimensions(
        &mut self,
        locator: &str,
        width: u32,
        height: u32,
        duration_secs: Option<f64>,
    ) -> AssetId {
        let id = AssetId(locator.to_owned());
        let duration_ms = duration_secs.map(|s| (s * 1000.0) as u64);
        self.videos.entry(id.clone()).or_insert(VideoInfoMeta { width, height, duration_ms });
        id
    }

    fn register_audio(&mut self, locator: &str) -> AssetId {
        let id = AssetId(locator.to_owned());
        self.audios.insert(id.clone());
        id
    }

    fn alias(&mut self, alias: AssetId, target: &AssetId) -> anyhow::Result<()> {
        if let Some(meta) = self.images.get(target).cloned() {
            self.images.insert(alias, meta);
        } else if let Some(meta) = self.videos.get(target).cloned() {
            self.videos.insert(alias, meta);
        }
        Ok(())
    }

    fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.images.get(id).map(|m| (m.width, m.height))
            .or_else(|| self.videos.get(id).map(|m| (m.width, m.height)))
            .unwrap_or((0, 0))
    }

    fn video_info(&self, id: &AssetId) -> Option<crate::resource::catalog::VideoInfoMeta> {
        self.videos.get(id).map(|m| {
            let duration_secs = m.duration_ms.map(|ms| ms as f64 / 1000.0);
            crate::resource::catalog::VideoInfoMeta {
                width: m.width,
                height: m.height,
                duration_secs,
            }
        })
    }
}
