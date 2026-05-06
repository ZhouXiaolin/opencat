//! Platform facade trait — aggregates backend / script / video / path-bounds.
//!
//! Each platform crate (engine, web) implements `Platform`, core pipeline
//! monomorphizes via `RenderSession<P: Platform>`.

use crate::platform::backend::BackendTypes;
use crate::platform::render_engine::RenderEngine;
use crate::platform::video::VideoFrameProvider;
use crate::scene::path_bounds::PathBoundsComputer;
use crate::scene::script::ScriptHost;

/// Platform facade: each backend provides four roles.
pub trait Platform: 'static {
    type Backend: RenderEngine + BackendTypes;
    type Script: ScriptHost;
    type Video: VideoFrameProvider;

    fn render_engine(&self) -> &Self::Backend;
    fn script_host(&mut self) -> &mut Self::Script;
    fn video_source(&mut self) -> &mut Self::Video;
    fn path_bounds(&self) -> &dyn PathBoundsComputer;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_ctx::ScriptFrameCtx;
    use crate::platform::backend::BackendTypes;
    use crate::platform::render_engine::{FrameView, RecordCtx, RenderCtx};
    use crate::platform::video::{FrameBitmap, VideoFrameProvider};
    use crate::resource::asset_id::AssetId;
    use crate::runtime::annotation::AnnotatedDisplayTree;
    use crate::scene::path_bounds::DefaultPathBounds;
    use crate::scene::script::{ScriptDriverId, ScriptHost, ScriptTextSource};
    use crate::script::recorder::MutationRecorder;
    use anyhow::Result;

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

    // SAFETY: MockBackend has no interior mutability.
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

    #[test]
    fn platform_associated_types_resolve() {
        let mut p = MockPlatform {
            backend: MockBackend,
            script: MockScript,
            video: MockVideo,
            path_bounds: DefaultPathBounds,
        };
        let _backend: &MockBackend = p.render_engine();
        let _script: &mut MockScript = p.script_host();
        let _video: &mut MockVideo = p.video_source();
        let _bounds: &dyn PathBoundsComputer = p.path_bounds();
    }
}
