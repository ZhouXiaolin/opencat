pub mod bitmap_source;
pub mod catalog;
pub mod probe;

pub use bitmap_source::*;
pub use catalog::{
    AudioPlan, AudioSegment, ImageMeta, ResourceCatalog, ResourceRequests, VideoInfoMeta,
    VideoSource,
};

use std::borrow::Cow;
use std::path::Path;

use anyhow::Result;

pub use crate::ir::asset_id::AssetId;
pub use crate::parse::primitives::{AudioSource, ImageSource, SubtitleSource};

pub trait AssetHandle: Clone + 'static {
    fn read_bytes(&self) -> Result<Cow<'_, [u8]>>;
    fn local_path(&self) -> Option<&Path> {
        None
    }
}

pub trait AssetLoader {
    type Handle: AssetHandle;
    fn load_all(&mut self, requests: &ResourceRequests) -> Result<()>;
    fn handle(&self, id: &AssetId) -> Option<&Self::Handle>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[derive(Clone)]
    struct ByteHandle(Arc<Vec<u8>>);
    impl AssetHandle for ByteHandle {
        fn read_bytes(&self) -> Result<Cow<'_, [u8]>> {
            Ok(Cow::Borrowed(&self.0))
        }
    }

    #[derive(Default)]
    struct InMemoryLoader {
        map: HashMap<AssetId, ByteHandle>,
    }
    impl AssetLoader for InMemoryLoader {
        type Handle = ByteHandle;
        fn load_all(&mut self, _: &ResourceRequests) -> Result<()> {
            Ok(())
        }
        fn handle(&self, id: &AssetId) -> Option<&Self::Handle> {
            self.map.get(id)
        }
    }

    #[test]
    fn handle_read_bytes_returns_payload() {
        let h = ByteHandle(Arc::new(b"hello".to_vec()));
        assert_eq!(&*h.read_bytes().unwrap(), b"hello");
        assert!(h.local_path().is_none());
    }

    #[test]
    fn loader_handle_lookup_roundtrips() {
        let mut l = InMemoryLoader::default();
        let id = AssetId("a".into());
        l.map.insert(id.clone(), ByteHandle(Arc::new(vec![1])));
        assert!(l.handle(&id).is_some());
        assert!(l.handle(&AssetId("missing".into())).is_none());
    }
}
