use ahash::AHasher;
use std::hash::{Hash, Hasher};

use crate::parse::primitives::OpenverseQuery;

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
}
