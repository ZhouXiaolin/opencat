use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::scene::primitives::OpenverseQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AssetId(pub String);

pub fn asset_id_for_url(url: &str) -> AssetId {
    AssetId(format!("url:{url}"))
}

pub fn asset_id_for_query(query: &OpenverseQuery) -> AssetId {
    AssetId(format!(
        "openverse:q={};count={};aspect_ratio={}",
        query.query,
        query.count,
        query.aspect_ratio.as_deref().unwrap_or("")
    ))
}

pub fn asset_id_for_audio_url(url: &str) -> AssetId {
    AssetId(format!("audio:url:{url}"))
}

pub fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
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
}
