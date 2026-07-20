pub mod bitmap_source;
pub mod catalog;
pub mod prepare;
pub mod probe;

pub use bitmap_source::*;
pub use catalog::{
    AudioPlan, AudioSegment, ImageMeta, ResourceCatalog, ResourceRequests, VideoInfoMeta,
    VideoSource,
};
pub use prepare::{
    ByteSource, PreparedCatalog, ProbeOutcome, build_catalog, hydrate_captions,
    lottie_dependencies,
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

/// Zero-sized loader that owns no bytes.
///
/// This exists only so the host-injected pipeline entry point
/// [`DefaultPipeline::open_with_prepared_catalog`](crate::pipeline::DefaultPipeline::open_with_prepared_catalog)
/// can return a `DefaultPipeline<NoopAssetLoader, S>` while the pipeline struct
/// is still parameterized over a loader. The host-injected path never calls
/// `load_all` or `handle` — its catalog is already prepared by the host via the
/// `probe::prepare` chain.
///
/// **Temporary bridge:** removed together with the loader seam in #11, when
/// `DefaultPipeline` drops its loader generic and the legacy loader-based entry
/// points are deleted.
#[derive(Default, Clone, Copy, Debug)]
pub struct NoopAssetLoader;

#[derive(Clone, Copy, Debug)]
pub struct NoopAssetHandle;

impl AssetHandle for NoopAssetHandle {
    fn read_bytes(&self) -> Result<Cow<'_, [u8]>> {
        // A noop loader owns no bytes; callers must never reach here. The
        // host-injected render path reads metadata from the prepared catalog,
        // not from handles.
        anyhow::bail!("NoopAssetLoader owns no bytes")
    }
}

impl AssetLoader for NoopAssetLoader {
    type Handle = NoopAssetHandle;
    fn load_all(&mut self, _requests: &ResourceRequests) -> Result<()> {
        // Noop: the host has already prepared the catalog before opening the
        // pipeline. There is nothing to fetch.
        Ok(())
    }
    fn handle(&self, _id: &AssetId) -> Option<&Self::Handle> {
        None
    }
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
