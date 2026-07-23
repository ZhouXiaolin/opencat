use std::collections::{HashMap, HashSet};

pub use crate::ir::asset_id::{AssetId, ResourceKind};
pub use crate::parse::primitives::VideoSource;
use crate::parse::primitives::{AudioSource, ImageSource, LottieSource, SubtitleSource};
use crate::lottie::LottieMeta;

/// One unique Lottie source locator — bundle identity is source-based.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LottieRequest {
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
/// [`AssetId`]. This is the pure metadata container used during rendering.
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

/// Probe/prepare video metadata. Duration is microsecond-based so engine and
/// web share one time unit with [`VideoInfoMeta`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_micros: Option<crate::time::DurationMicros>,
}

impl VideoInfoMeta {
    pub fn duration_secs(&self) -> Option<f64> {
        self.duration_micros
            .map(|d| crate::time::timestamp_micros_to_secs(d.0))
    }
}

// Re-export SrtEntry from parse primitives for catalog consumers.
pub use crate::parse::primitives::SrtEntry;

impl PreparedResourceCatalog {
    /// Resolve an image source to its canonical AssetId.
    pub fn resolve_image(&self, src: &ImageSource) -> anyhow::Result<AssetId> {
        match crate::ir::asset_id::asset_id_for_image(src) {
            Some(id) => Ok(id),
            None => anyhow::bail!("unset image source"),
        }
    }

    /// Resolve an audio source to its canonical AssetId.
    pub fn resolve_audio(&self, src: &AudioSource) -> anyhow::Result<AssetId> {
        match crate::ir::asset_id::asset_id_for_audio(src) {
            Some(id) => Ok(id),
            None => anyhow::bail!("unset audio source"),
        }
    }

    /// Register image dimensions under a path-based id. Used by hosts during
    /// resource probing (before prepare).
    pub fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let id = AssetId::new(ResourceKind::Image, locator.to_owned());
        self.images
            .entry(id.clone())
            .or_insert(ImageMeta { width, height });
        id
    }

    /// Register video metadata under a path-based id. Used by hosts during
    /// resource probing (before prepare).
    pub fn register_video_dimensions(
        &mut self,
        locator: &str,
        width: u32,
        height: u32,
        duration_secs: Option<f64>,
    ) -> AssetId {
        let id = AssetId::new(ResourceKind::Video, locator.to_owned());
        let duration_micros = crate::time::optional_secs_to_duration_micros(duration_secs);
        self.videos.entry(id.clone()).or_insert(VideoInfoMeta {
            width,
            height,
            duration_micros,
        });
        id
    }

    /// Register an audio asset id by locator string. Used by hosts during
    /// resource probing (before prepare).
    pub fn register_audio(&mut self, locator: &str) -> AssetId {
        let id = AssetId::new(ResourceKind::Audio, locator.to_owned());
        self.audios.insert(id.clone());
        id
    }

    /// Bind a pipeline-internal alias to a canonical asset. The alias is
    /// resolved during rendering by [`resolve_alias`].
    pub fn alias(&mut self, alias: AssetId, target: &AssetId) -> anyhow::Result<()> {
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
                let meta = self.lotties.get(target).cloned().expect("checked above");
                self.lotties.insert(alias.clone(), meta);
            }
        }
        self.aliases.insert(alias, target.clone());
        Ok(())
    }

    /// Look up image or video pixel dimensions by AssetId.
    pub fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.images
            .get(id)
            .map(|m| (m.width, m.height))
            .or_else(|| self.videos.get(id).map(|m| (m.width, m.height)))
            .unwrap_or((0, 0))
    }

    /// Look up video metadata by AssetId.
    pub fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.videos.get(id).map(|m| VideoInfoMeta {
            width: m.width,
            height: m.height,
            duration_micros: m.duration_micros,
        })
    }

    /// Resolve a Lottie source locator to its bundle AssetId.
    pub fn resolve_lottie(&self, src: &LottieSource) -> anyhow::Result<AssetId> {
        crate::ir::asset_id::asset_id_for_lottie(src)
            .ok_or_else(|| anyhow::anyhow!("unset lottie source"))
    }

    /// Look up Lottie metadata by AssetId.
    pub fn lottie_meta(&self, id: &AssetId) -> Option<LottieMeta> {
        self.lotties.get(id).cloned()
    }

    /// Resolve a pipeline-internal alias to its canonical AssetId.
    pub fn resolve_alias(&self, alias: &AssetId) -> Option<AssetId> {
        self.aliases.get(alias).cloned()
    }

    /// Returns true when `id` is a known canonical asset or a registered alias.
    pub fn is_known_asset(&self, id: &AssetId) -> bool {
        self.canonical_kind(id).is_some() || self.aliases.contains_key(id)
    }

    /// Classify a canonical `AssetId`, or `None` if it is not a declared
    /// asset. Alias keys that were mirrored into a metadata map are not
    /// canonical; resolve them via [`resolve_alias`] first.
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

/// Which canonical asset map an `AssetId` belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalKind {
    Image,
    Video,
    Audio,
    Subtitle,
    Lottie,
}
