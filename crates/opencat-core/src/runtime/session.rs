//! Generic per-render session: holds backend-agnostic state + platform state.
//!
//! engine / web each have a type alias monomorphizing this session.

use std::sync::Arc;

use crate::layout::LayoutSession;
use crate::platform::platform::Platform;
use crate::platform::scene_snapshot::SceneSnapshotCache;
use crate::resource::hash_map_catalog::HashMapResourceCatalog;
use crate::runtime::cache::{CacheCaps, CacheRegistry};
use crate::runtime::invalidation::CompositeHistory;
use crate::text::default_font_db;

pub struct RenderSession<P: Platform> {
    /// per-render layout accumulator (node id -> measure cache)
    pub layout_session: LayoutSession,

    /// cross-frame composite dirty history
    pub composite_history: CompositeHistory,

    /// fontdb (platform-agnostic, cosmic-text reuses)
    pub font_db: Arc<fontdb::Database>,

    /// resource metadata (preflight writes; render reads only)
    pub catalog: HashMapResourceCatalog,

    /// last preflight root pointer, for skipping duplicate preflight
    pub prepared_root_ptr: Option<usize>,

    /// backend-typed caches (subtree snapshot / item picture / glyph etc)
    pub cache_registry: CacheRegistry<P::Backend>,

    /// single-slot scene snapshot cross-frame cache
    pub scene_snapshots: SceneSnapshotCache<P::Backend>,

    /// platform's own stuff (script runtime, video source, render engine ref, IO etc)
    pub platform: P,
}

impl<P: Platform> RenderSession<P> {
    pub fn new(platform: P) -> Self {
        Self::with_cache_caps(platform, CacheCaps::default())
    }

    pub fn with_cache_caps(platform: P, caps: CacheCaps) -> Self {
        let font_db = Arc::new(default_font_db(&[]));
        Self {
            layout_session: LayoutSession::new(),
            composite_history: CompositeHistory::default(),
            font_db,
            catalog: HashMapResourceCatalog::from_json("{}")
                .expect("empty catalog must parse"),
            prepared_root_ptr: None,
            cache_registry: CacheRegistry::new(caps),
            scene_snapshots: SceneSnapshotCache::new(),
            platform,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_ctx::ScriptFrameCtx;
    use crate::platform::backend::BackendTypes;
    use crate::platform::render_engine::{FrameView, RecordCtx, RenderCtx, RenderEngine};
    use crate::platform::video::{FrameBitmap, VideoFrameProvider};
    use crate::resource::asset_id::AssetId;
    use crate::runtime::annotation::AnnotatedDisplayTree;
    use crate::scene::path_bounds::{DefaultPathBounds, PathBoundsComputer};
    use crate::scene::script::{ScriptDriverId, ScriptHost, ScriptTextSource};
    use crate::script::recorder::MutationRecorder;
    use anyhow::Result;

    // --- mock types (mirror platform.rs tests) ---

    struct MockBackend;
    impl BackendTypes for MockBackend {
        type Picture = String;
        type Image = String;
        type GlyphPath = String;
        type GlyphImage = String;
    }
    impl RenderEngine for MockBackend {
        fn target_frame_view_kind(&self) -> &'static str {
            "mock"
        }
        fn draw_scene_snapshot(
            &self,
            _: &Self::Picture,
            _: FrameView<'_>,
        ) -> Result<()> {
            Ok(())
        }
        fn record_display_tree_snapshot(
            &self,
            _: &mut RecordCtx<'_, Self>,
            _: &AnnotatedDisplayTree,
        ) -> Result<Self::Picture>
        where
            Self: Sized,
        {
            Ok("snap".into())
        }
        fn draw_ordered_scene(
            &self,
            _: &mut RenderCtx<'_, Self>,
            _: FrameView<'_>,
        ) -> Result<()>
        where
            Self: Sized,
        {
            Ok(())
        }
    }
    unsafe impl Send for MockBackend {}
    unsafe impl Sync for MockBackend {}

    struct MockScript;
    impl ScriptHost for MockScript {
        fn install(&mut self, _: &str) -> Result<ScriptDriverId> {
            Ok(ScriptDriverId(1))
        }
        fn register_text_source(&mut self, _: &str, _: ScriptTextSource) {}
        fn clear_text_sources(&mut self) {}
        fn run_frame(
            &mut self,
            _: ScriptDriverId,
            _: &ScriptFrameCtx,
            _: Option<&str>,
            _: &mut dyn MutationRecorder,
        ) -> Result<()> {
            Ok(())
        }
    }

    struct MockVideo;
    impl VideoFrameProvider for MockVideo {
        fn frame_rgba(&mut self, _: &AssetId, _: u32) -> Result<FrameBitmap> {
            Ok(FrameBitmap {
                data: std::sync::Arc::new(vec![0; 4]),
                width: 1,
                height: 1,
            })
        }
    }

    struct MockPlatform {
        backend: MockBackend,
        script: MockScript,
        video: MockVideo,
        path_bounds: DefaultPathBounds,
    }
    impl Platform for MockPlatform {
        type Backend = MockBackend;
        type Script = MockScript;
        type Video = MockVideo;

        fn render_engine(&self) -> &Self::Backend {
            &self.backend
        }
        fn script_host(&mut self) -> &mut Self::Script {
            &mut self.script
        }
        fn video_source(&mut self) -> &mut Self::Video {
            &mut self.video
        }
        fn path_bounds(&self) -> &dyn PathBoundsComputer {
            &self.path_bounds
        }
    }

    fn make_mock_platform() -> MockPlatform {
        MockPlatform {
            backend: MockBackend,
            script: MockScript,
            video: MockVideo,
            path_bounds: DefaultPathBounds,
        }
    }

    #[test]
    fn render_session_constructs_with_default_caps() {
        let session = RenderSession::new(make_mock_platform());
        assert!(session.prepared_root_ptr.is_none());
        assert!(session.scene_snapshots.scene_snapshot().is_none());
    }

    #[test]
    fn render_session_with_custom_cache_caps() {
        let caps = CacheCaps {
            images: 1,
            subtree_snapshots: 2,
            subtree_images: 3,
            item_pictures: 4,
            video_frames: 5,
            glyph_paths: 6,
            glyph_images: 7,
        };
        let session = RenderSession::with_cache_caps(make_mock_platform(), caps);
        assert_eq!(session.cache_registry.image_cache().borrow().capacity(), 1);
        assert_eq!(
            session.cache_registry.glyph_path_cache().borrow().capacity(),
            6
        );
    }
}
