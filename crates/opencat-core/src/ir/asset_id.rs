use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::Path;

use ahash::AHasher;

use crate::parse::primitives::{
    AudioSource, ImageSource, LottieSource, OpenverseQuery, SubtitleSource, VideoSource,
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
    Subtitle,
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
    } else if key.starts_with("subtitle:") {
        ResourceKind::Subtitle
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

/// Canonical `AssetId` for a subtitle source.
pub fn asset_id_for_subtitle(src: &SubtitleSource) -> AssetId {
    match src {
        SubtitleSource::Path(p) => AssetId::new(
            ResourceKind::Subtitle,
            format!("subtitle:path:{}", path_str(p)),
        ),
        SubtitleSource::Url(u) => AssetId::new(ResourceKind::Subtitle, format!("subtitle:url:{u}")),
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
    let mut hasher = AHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_id_for_url_is_deterministic() {
        let id1 = asset_id_for_url("https://example.com/a.png");
        let id2 = asset_id_for_url("https://example.com/a.png");
        assert_eq!(id1, id2);
        assert_eq!(id1.as_str(), "url:https://example.com/a.png");
        assert_eq!(id1.kind, ResourceKind::Image);
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
        assert_eq!(id1.as_str(), "openverse:q=cat;count=3;aspect_ratio=wide");
    }

    #[test]
    fn asset_id_for_query_without_aspect_ratio_omits_field() {
        let q = OpenverseQuery {
            query: "dog".into(),
            count: 5,
            aspect_ratio: None,
        };
        let id = asset_id_for_query(&q);
        assert_eq!(id.as_str(), "openverse:q=dog;count=5");
    }

    #[test]
    fn asset_id_for_audio_url_is_deterministic() {
        let id1 = asset_id_for_audio_url("https://example.com/music.mp3");
        let id2 = asset_id_for_audio_url("https://example.com/music.mp3");
        assert_eq!(id1, id2);
        assert_eq!(id1.as_str(), "audio:url:https://example.com/music.mp3");
        assert_eq!(id1.kind, ResourceKind::Audio);
    }

    #[test]
    fn asset_id_for_video_url_is_deterministic() {
        let id1 = asset_id_for_video_url("https://example.com/clip.mp4");
        let id2 = asset_id_for_video_url("https://example.com/clip.mp4");
        assert_eq!(id1, id2);
        assert_eq!(id1.as_str(), "video:url:https://example.com/clip.mp4");
        assert_eq!(id1.kind, ResourceKind::Video);
    }

    #[test]
    fn asset_id_for_image_covers_all_variants() {
        assert_eq!(asset_id_for_image(&ImageSource::Unset), None);
        assert_eq!(
            asset_id_for_image(&ImageSource::Path("photos/a.png".into()))
                .map(|i| i.as_str().to_owned()),
            Some("photos/a.png".to_string()),
        );
        assert_eq!(
            asset_id_for_image(&ImageSource::Url("https://e.com/a.png".into()))
                .map(|i| i.as_str().to_owned()),
            Some("url:https://e.com/a.png".to_string()),
        );
        let q = OpenverseQuery {
            query: "cat".into(),
            count: 2,
            aspect_ratio: None,
        };
        assert_eq!(
            asset_id_for_image(&ImageSource::Query(q.clone())).map(|i| i.as_str().to_owned()),
            Some("openverse:q=cat;count=2".to_string()),
        );
    }

    #[test]
    fn asset_id_for_image_is_stable_across_calls() {
        let src = ImageSource::Url("https://e.com/x.png".into());
        assert_eq!(asset_id_for_image(&src), asset_id_for_image(&src));
    }

    #[test]
    fn asset_id_for_video_covers_path_and_url() {
        assert_eq!(
            asset_id_for_video(&VideoSource::Path("/c/d.mp4".into())).as_str(),
            "video:path:/c/d.mp4",
        );
        assert_eq!(
            asset_id_for_video(&VideoSource::Url("https://e.com/v.mp4".into())).as_str(),
            "video:url:https://e.com/v.mp4",
        );
    }

    #[test]
    fn asset_id_for_audio_covers_all_variants() {
        assert_eq!(asset_id_for_audio(&AudioSource::Unset), None);
        assert_eq!(
            asset_id_for_audio(&AudioSource::Path("/a/m.mp3".into()))
                .map(|i| i.as_str().to_owned()),
            Some("audio:path:/a/m.mp3".to_string()),
        );
        assert_eq!(
            asset_id_for_audio(&AudioSource::Url("https://e.com/m.mp3".into()))
                .map(|i| i.as_str().to_owned()),
            Some("audio:url:https://e.com/m.mp3".to_string()),
        );
    }

    #[test]
    fn asset_id_for_subtitle_covers_path_and_url() {
        assert_eq!(
            asset_id_for_subtitle(&SubtitleSource::Path("/a/sub.srt".into())).as_str(),
            "subtitle:path:/a/sub.srt",
        );
        assert_eq!(
            asset_id_for_subtitle(&SubtitleSource::Url("https://e.com/sub.srt".into())).as_str(),
            "subtitle:url:https://e.com/sub.srt",
        );
    }

    #[test]
    fn asset_id_for_lottie_is_source_based_and_unset_yields_none() {
        assert_eq!(asset_id_for_lottie(&LottieSource::Unset), None);
        assert_eq!(
            asset_id_for_lottie(&LottieSource::Path("anim/loader.json".into()))
                .map(|i| i.as_str().to_owned()),
            Some("lottie:path:anim/loader.json".to_string()),
        );
        assert_eq!(
            asset_id_for_lottie(&LottieSource::Url("https://e.com/a.json".into()))
                .map(|i| i.as_str().to_owned()),
            Some("lottie:url:https://e.com/a.json".to_string()),
        );
    }

    /// AC2: AssetIds of different kinds never collide, even with an identical
    /// key. A single typed resource map is therefore collision-free across
    /// image, video, audio, subtitle, lottie, font and script resources.
    #[test]
    fn asset_ids_of_different_kinds_do_not_collide() {
        let same_key = "shared";
        let image = AssetId::new(ResourceKind::Image, same_key);
        let video = AssetId::new(ResourceKind::Video, same_key);
        let audio = AssetId::new(ResourceKind::Audio, same_key);
        let subtitle = AssetId::new(ResourceKind::Subtitle, same_key);
        let lottie = AssetId::new(ResourceKind::Lottie, same_key);
        let font = AssetId::new(ResourceKind::Font, same_key);
        let script = AssetId::new(ResourceKind::Script, same_key);

        assert_ne!(image, video);
        assert_ne!(image, audio);
        assert_ne!(image, subtitle);
        assert_ne!(image, lottie);
        assert_ne!(image, font);
        assert_ne!(image, script);

        // Distinct kind + same key occupy distinct slots in one typed map.
        let mut map = std::collections::HashMap::new();
        for id in [&image, &video, &audio, &subtitle, &lottie, &font, &script] {
            map.insert(id.clone(), id.kind);
        }
        assert_eq!(map.len(), 7);

        // Same kind + same key IS the same id (stable identity).
        assert_eq!(image, AssetId::new(ResourceKind::Image, same_key));
    }

    #[test]
    fn asset_id_display_is_the_canonical_wire_string() {
        assert_eq!(
            asset_id_for_video(&VideoSource::Path("/c/d.mp4".into())).to_string(),
            "video:path:/c/d.mp4",
        );
    }
}
