use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::Path;


use crate::parse::primitives::{
    AudioSource, ImageSource, LottieSource, OpenverseQuery, VideoSource,
};

/// Kind of external resource an [`AssetId`] names. One `AssetId` namespace per
/// kind, so a single typed resource map cannot suffer cross-kind collisions
/// (issue #39). Lives next to [`AssetId`] so identity rules have exactly one
/// home; the lifecycle re-exports it for the contract surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceKind {
    Image,
    Video,
    Audio,
    Lottie,
    /// Document font face from `<fonts>` / `FontManifest`. Stable identity is
    /// [`crate::fonts::font_asset_id`]; host supplies raw bytes only.
    Font,
    /// External script file. Host supplies raw text; core never reads files.
    Script,
}

/// Canonical, core-generated identity for one external resource.
///
/// Carries a typed [`ResourceKind`] namespace plus a `key` whose string value
/// is the canonical wire form (byte-identical to the historical id form, so
/// OCIR output, host caches and DrawOp asset references are unchanged).
/// Equality and hashing are over `(kind, key)`, so two resources of different
/// kinds never collide even when they share a key. Hosts treat the id as
/// opaque and never derive it from locator values (issue #38 / #39).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetId {
    pub kind: ResourceKind,
    pub key: String,
}

impl AssetId {
    /// Canonical constructor — the only way identity is minted outside the
    /// per-kind helpers below. `key` is the stable wire string.
    pub fn new(kind: ResourceKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }

    /// Canonical wire string (stable across OCIR / host caches / DrawOp).
    pub fn as_str(&self) -> &str {
        &self.key
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key)
    }
}

// AssetId serializes as its canonical wire string, preserving the historical
// "AssetId is a string on the wire" contract for any serde/inspect consumer.
// Nothing currently round-trips an AssetId through serde; deserialization
// recovers the kind from the canonical prefix as a defensive best effort.
impl serde::Serialize for AssetId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.key)
    }
}

impl<'de> serde::Deserialize<'de> for AssetId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let key = <String as serde::Deserialize>::deserialize(deserializer)?;
        let kind = kind_from_canonical_str(&key);
        Ok(AssetId { kind, key })
    }
}

/// Best-effort kind recovery from a canonical id string. The historical prefixes
/// are deterministic per kind; bare strings and `url:` / `openverse:` are image
/// sources. Used by the (currently unused) serde Deserialize path and by the
/// legacy probe `ByteSource` impl, which only knows the canonical wire string.
/// Production lifecycle never reconstructs ids from strings.
pub fn kind_from_canonical_str(key: &str) -> ResourceKind {
    if let Some(rest) = key.strip_prefix("video:") {
        let _ = rest;
        ResourceKind::Video
    } else if key.starts_with("audio:") {
        ResourceKind::Audio
    } else if key.starts_with("lottie:") {
        ResourceKind::Lottie
    } else if key.starts_with("font:") {
        ResourceKind::Font
    } else if key.starts_with("script:") {
        ResourceKind::Script
    } else {
        // image path ids are bare; `url:` and `openverse:` are image sources.
        ResourceKind::Image
    }
}

pub fn asset_id_for_url(url: &str) -> AssetId {
    AssetId::new(ResourceKind::Image, format!("url:{url}"))
}

pub fn asset_id_for_query(query: &OpenverseQuery) -> AssetId {
    let key = if let Some(ar) = &query.aspect_ratio {
        format!(
            "openverse:q={};count={};aspect_ratio={}",
            query.query, query.count, ar
        )
    } else {
        format!("openverse:q={};count={}", query.query, query.count)
    };
    AssetId::new(ResourceKind::Image, key)
}

pub fn asset_id_for_audio_url(url: &str) -> AssetId {
    AssetId::new(ResourceKind::Audio, format!("audio:url:{url}"))
}

pub fn asset_id_for_video_url(url: &str) -> AssetId {
    AssetId::new(ResourceKind::Video, format!("video:url:{url}"))
}

// ---------------------------------------------------------------------------
// Unified canonical AssetId rules.
//
// Every resource `source` -> `AssetId` mapping lives here as a pure function.
// Hosts may choose fetch/cache/decode strategies freely but must not redefine
// these IDs. `Unset` sources yield `None` (no declared asset); callers treat a
// missing canonical ID as a render error.
// ---------------------------------------------------------------------------

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Canonical `AssetId` for an image source, or `None` for `Unset`.
///
/// Path variants use the logical locator string as the id (no host base join).
pub fn asset_id_for_image(src: &ImageSource) -> Option<AssetId> {
    match src {
        ImageSource::Unset => None,
        ImageSource::Path(p) => Some(AssetId::new(ResourceKind::Image, p.clone())),
        ImageSource::Url(u) => Some(asset_id_for_url(u)),
        ImageSource::Query(q) => Some(asset_id_for_query(q)),
    }
}

/// Canonical `AssetId` for a video source.
///
/// Path variants use the logical locator string under a stable `video:path:`
/// prefix (no host base join).
pub fn asset_id_for_video(src: &VideoSource) -> AssetId {
    match src {
        VideoSource::Path(p) => AssetId::new(ResourceKind::Video, format!("video:path:{p}")),
        VideoSource::Url(u) => asset_id_for_video_url(u),
    }
}

/// Canonical `AssetId` for an audio source, or `None` for `Unset`.
pub fn asset_id_for_audio(src: &AudioSource) -> Option<AssetId> {
    match src {
        AudioSource::Unset => None,
        AudioSource::Path(p) => Some(AssetId::new(
            ResourceKind::Audio,
            format!("audio:path:{}", path_str(p)),
        )),
        AudioSource::Url(u) => Some(asset_id_for_audio_url(u)),
    }
}

/// Canonical `AssetId` for a Lottie source, or `None` for `Unset`.
///
/// Bundle identity is determined by the source locator (path or URL), not by
/// the element id. Multiple `<lottie>` nodes sharing the same locator resolve
/// to the same bundle, while each retains independent render state (timing,
/// frame, transform).
pub fn asset_id_for_lottie(src: &LottieSource) -> Option<AssetId> {
    match src {
        LottieSource::Unset => None,
        LottieSource::Path(p) => Some(AssetId::new(
            ResourceKind::Lottie,
            format!("lottie:path:{p}"),
        )),
        LottieSource::Url(u) => Some(AssetId::new(
            ResourceKind::Lottie,
            format!("lottie:url:{u}"),
        )),
    }
}

pub fn stable_hash(value: &str) -> u64 {
    // std's SipHash with fixed zero keys — NOT AHasher::default(). ahash's
    // default constructor seeds randomly per process (DoS protection), so
    // `AHasher::default()` produced a DIFFERENT hash for the same key on
    // every run: the asset cache path never repeated and every render
    // re-copied every asset (one 83 MB video accumulated 524 identical
    // copies / ~43 GB in ~/.opencat/assets). Same key → same hash across
    // processes is the contract the name already promises.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}


