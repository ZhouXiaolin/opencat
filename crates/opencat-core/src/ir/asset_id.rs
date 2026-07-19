use ahash::AHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::parse::primitives::{
    AudioSource, ImageSource, LottieSource, OpenverseQuery, SubtitleSource, VideoSource,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AssetId(pub String);

pub fn asset_id_for_url(url: &str) -> AssetId {
    AssetId(format!("url:{url}"))
}

pub fn asset_id_for_query(query: &OpenverseQuery) -> AssetId {
    if let Some(ar) = &query.aspect_ratio {
        AssetId(format!(
            "openverse:q={};count={};aspect_ratio={}",
            query.query, query.count, ar
        ))
    } else {
        AssetId(format!("openverse:q={};count={}", query.query, query.count))
    }
}

pub fn asset_id_for_audio_url(url: &str) -> AssetId {
    AssetId(format!("audio:url:{url}"))
}

pub fn asset_id_for_video_url(url: &str) -> AssetId {
    AssetId(format!("video:url:{url}"))
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
pub fn asset_id_for_image(src: &ImageSource) -> Option<AssetId> {
    match src {
        ImageSource::Unset => None,
        ImageSource::Path(p) => Some(AssetId(path_str(p))),
        ImageSource::Url(u) => Some(asset_id_for_url(u)),
        ImageSource::Query(q) => Some(asset_id_for_query(q)),
    }
}

/// Canonical `AssetId` for a video source.
pub fn asset_id_for_video(src: &VideoSource) -> AssetId {
    match src {
        VideoSource::Path(p) => AssetId(format!("video:path:{}", path_str(p))),
        VideoSource::Url(u) => asset_id_for_video_url(u),
    }
}

/// Canonical `AssetId` for an audio source, or `None` for `Unset`.
pub fn asset_id_for_audio(src: &AudioSource) -> Option<AssetId> {
    match src {
        AudioSource::Unset => None,
        AudioSource::Path(p) => Some(AssetId(format!("audio:path:{}", path_str(p)))),
        AudioSource::Url(u) => Some(asset_id_for_audio_url(u)),
    }
}

/// Canonical `AssetId` for a subtitle source.
pub fn asset_id_for_subtitle(src: &SubtitleSource) -> AssetId {
    match src {
        SubtitleSource::Path(p) => AssetId(format!("subtitle:path:{}", path_str(p))),
        SubtitleSource::Url(u) => AssetId(format!("subtitle:url:{u}")),
    }
}

/// Canonical `AssetId` for a Lottie source, or `None` for `Unset`.
///
/// `element_id` is the `<lottie id="…">` node id; the bundle id is
/// `lottie:{element_id}` regardless of where the bytes come from.
pub fn asset_id_for_lottie(element_id: &str, src: &LottieSource) -> Option<AssetId> {
    match src {
        LottieSource::Unset => None,
        LottieSource::Path(_) | LottieSource::Url(_) => Some(AssetId(format!("lottie:{element_id}"))),
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
        assert_eq!(id1.0, "url:https://example.com/a.png");
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
        assert_eq!(id1.0, "openverse:q=cat;count=3;aspect_ratio=wide");
    }

    #[test]
    fn asset_id_for_query_without_aspect_ratio_omits_field() {
        let q = OpenverseQuery {
            query: "dog".into(),
            count: 5,
            aspect_ratio: None,
        };
        let id = asset_id_for_query(&q);
        assert_eq!(id.0, "openverse:q=dog;count=5");
    }

    #[test]
    fn asset_id_for_audio_url_is_deterministic() {
        let id1 = asset_id_for_audio_url("https://example.com/music.mp3");
        let id2 = asset_id_for_audio_url("https://example.com/music.mp3");
        assert_eq!(id1, id2);
        assert_eq!(id1.0, "audio:url:https://example.com/music.mp3");
    }

    #[test]
    fn asset_id_for_video_url_is_deterministic() {
        let id1 = asset_id_for_video_url("https://example.com/clip.mp4");
        let id2 = asset_id_for_video_url("https://example.com/clip.mp4");
        assert_eq!(id1, id2);
        assert_eq!(id1.0, "video:url:https://example.com/clip.mp4");
    }

    #[test]
    fn asset_id_for_image_covers_all_variants() {
        assert_eq!(asset_id_for_image(&ImageSource::Unset), None);
        assert_eq!(
            asset_id_for_image(&ImageSource::Path("/a/b.png".into())).map(|i| i.0),
            Some("/a/b.png".to_string()),
        );
        assert_eq!(
            asset_id_for_image(&ImageSource::Url("https://e.com/a.png".into())).map(|i| i.0),
            Some("url:https://e.com/a.png".to_string()),
        );
        let q = OpenverseQuery {
            query: "cat".into(),
            count: 2,
            aspect_ratio: None,
        };
        assert_eq!(
            asset_id_for_image(&ImageSource::Query(q.clone())).map(|i| i.0),
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
            asset_id_for_video(&VideoSource::Path("/c/d.mp4".into())).0,
            "video:path:/c/d.mp4",
        );
        assert_eq!(
            asset_id_for_video(&VideoSource::Url("https://e.com/v.mp4".into())).0,
            "video:url:https://e.com/v.mp4",
        );
    }

    #[test]
    fn asset_id_for_audio_covers_all_variants() {
        assert_eq!(asset_id_for_audio(&AudioSource::Unset), None);
        assert_eq!(
            asset_id_for_audio(&AudioSource::Path("/a/m.mp3".into())).map(|i| i.0),
            Some("audio:path:/a/m.mp3".to_string()),
        );
        assert_eq!(
            asset_id_for_audio(&AudioSource::Url("https://e.com/m.mp3".into())).map(|i| i.0),
            Some("audio:url:https://e.com/m.mp3".to_string()),
        );
    }

    #[test]
    fn asset_id_for_subtitle_covers_path_and_url() {
        assert_eq!(
            asset_id_for_subtitle(&SubtitleSource::Path("/a/sub.srt".into())).0,
            "subtitle:path:/a/sub.srt",
        );
        assert_eq!(
            asset_id_for_subtitle(&SubtitleSource::Url("https://e.com/sub.srt".into())).0,
            "subtitle:url:https://e.com/sub.srt",
        );
    }

    #[test]
    fn asset_id_for_lottie_is_element_id_based_and_unset_yields_none() {
        assert_eq!(asset_id_for_lottie("hero", &LottieSource::Unset), None);
        assert_eq!(
            asset_id_for_lottie("hero", &LottieSource::Path("/a.json".into())).map(|i| i.0),
            Some("lottie:hero".to_string()),
        );
        assert_eq!(
            asset_id_for_lottie("hero", &LottieSource::Url("https://e.com/a.json".into()))
                .map(|i| i.0),
            Some("lottie:hero".to_string()),
        );
    }
}
