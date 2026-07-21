use std::collections::{HashMap, HashSet};

pub use crate::ir::asset_id::AssetId;
pub use crate::parse::primitives::VideoSource;
use crate::parse::primitives::{AudioSource, ImageSource, LottieSource, SrtEntry, SubtitleSource};
use crate::resource::lottie::LottieMeta;

/// One `<lottie id="…">` node — bundle id is `lottie:{element_id}`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LottieRequest {
    pub element_id: String,
    pub source: LottieSource,
}

#[derive(Default, Clone, Debug)]
pub struct ResourceRequests {
    pub images: HashSet<ImageSource>,
    pub videos: HashSet<VideoSource>,
    pub audios: HashSet<AudioSource>,
    pub subtitles: HashSet<SubtitleSource>,
    pub lotties: HashSet<LottieRequest>,
}

/// Probe / prepare result: validated resource metadata keyed by canonical
/// [`AssetId`]. Distinct from the behavioral [`crate::resource::catalog::ResourceResolver`]
/// trait used during resolve/render.
#[derive(Default, Clone, Debug)]
pub struct PreparedResourceCatalog {
    pub images: HashMap<AssetId, ImageMeta>,
    pub videos: HashMap<AssetId, VideoInfoMeta>,
    pub audios: HashSet<AssetId>,
    pub subtitles: HashMap<AssetId, Vec<SrtEntry>>,
    pub lotties: HashMap<AssetId, LottieMeta>,
    /// Pipeline-internal alias -> canonical `AssetId` bindings. Aliases are
    /// resolved to canonical IDs before any `DrawOp`/`FrameMediaPlan` leaves
    /// the pipeline, so hosts never observe an alias. The catalog itself does
    /// not serve metadata under an alias key from this map; lookups resolve the
    /// alias first.
    pub aliases: HashMap<AssetId, AssetId>,
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

impl crate::resource::catalog::ResourceResolver for PreparedResourceCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> anyhow::Result<AssetId> {
        match crate::ir::asset_id::asset_id_for_image(src) {
            Some(id) => Ok(id),
            None => anyhow::bail!("unset image source"),
        }
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> anyhow::Result<AssetId> {
        match crate::ir::asset_id::asset_id_for_audio(src) {
            Some(id) => Ok(id),
            None => anyhow::bail!("unset audio source"),
        }
    }

    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let id = AssetId(locator.to_owned());
        self.images
            .entry(id.clone())
            .or_insert(ImageMeta { width, height });
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
        self.videos.entry(id.clone()).or_insert(VideoInfoMeta {
            width,
            height,
            duration_ms,
        });
        id
    }

    fn register_audio(&mut self, locator: &str) -> AssetId {
        let id = AssetId(locator.to_owned());
        self.audios.insert(id.clone());
        id
    }

    fn alias(&mut self, alias: AssetId, target: &AssetId) -> anyhow::Result<()> {
        // An alias must point at a declared, canonical asset. An unknown
        // target is a render error rather than a silent no-op.
        let kind = self
            .canonical_kind(target)
            .ok_or_else(|| anyhow::anyhow!("alias target {target:?} is not a declared asset"))?;
        match kind {
            CanonicalKind::Image => {
                let meta = self.images.get(target).copied().expect("checked above");
                self.images.insert(alias.clone(), meta);
            }
            CanonicalKind::Video => {
                let meta = self.videos.get(target).cloned().expect("checked above");
                self.videos.insert(alias.clone(), meta);
            }
            CanonicalKind::Audio => {
                self.audios.insert(alias.clone());
            }
            CanonicalKind::Subtitle => {
                let entries = self.subtitles.get(target).cloned().expect("checked above");
                self.subtitles.insert(alias.clone(), entries);
            }
            CanonicalKind::Lottie => {
                let meta = self.lotties.get(target).copied().expect("checked above");
                self.lotties.insert(alias.clone(), meta);
            }
        }
        self.aliases.insert(alias, target.clone());
        Ok(())
    }

    fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.images
            .get(id)
            .map(|m| (m.width, m.height))
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

    fn resolve_lottie(&mut self, element_id: &str) -> anyhow::Result<AssetId> {
        Ok(AssetId(format!("lottie:{element_id}")))
    }

    fn lottie_meta(&self, id: &AssetId) -> Option<LottieMeta> {
        self.lotties.get(id).copied()
    }

    fn resolve_alias(&self, alias: &AssetId) -> Option<AssetId> {
        self.aliases.get(alias).cloned()
    }

    fn is_known_asset(&self, id: &AssetId) -> bool {
        self.canonical_kind(id).is_some() || self.aliases.contains_key(id)
    }
}

/// Which canonical asset map an `AssetId` belongs to. Used internally to keep
/// `alias()` and lookups consistent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CanonicalKind {
    Image,
    Video,
    Audio,
    Subtitle,
    Lottie,
}

impl PreparedResourceCatalog {
    /// Classify a canonical `AssetId`, or `None` if it is not a declared
    /// asset. Alias keys that were mirrored into a metadata map are not
    /// canonical; resolve them via [`PreparedResourceCatalog::resolve_alias`] first.
    pub fn canonical_kind(&self, id: &AssetId) -> Option<CanonicalKind> {
        if self.images.contains_key(id) {
            Some(CanonicalKind::Image)
        } else if self.videos.contains_key(id) {
            Some(CanonicalKind::Video)
        } else if self.audios.contains(id) {
            Some(CanonicalKind::Audio)
        } else if self.subtitles.contains_key(id) {
            Some(CanonicalKind::Subtitle)
        } else if self.lotties.contains_key(id) {
            Some(CanonicalKind::Lottie)
        } else {
            None
        }
    }
}
