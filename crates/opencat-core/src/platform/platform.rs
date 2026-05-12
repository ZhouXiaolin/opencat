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

    /// Combined accessor that resolves the borrow conflict between script_host (mut)
    /// and path_bounds (immutable). Default impl borrows sequentially via UnsafeCell
    /// semantics — implementors should override if they have interior mutability.
    ///
    /// SAFETY: The default impl is safe because `script_host` and `path_bounds`
    /// access disjoint fields. The reborrow through raw pointers is valid because:
    /// 1. `self` is exclusively borrowed for the duration of the callback.
    /// 2. `script` and `path_bounds` do not overlap in memory.
    /// 3. No other code can access `self` during the callback.
    fn with_script_and_bounds<R>(
        &mut self,
        f: impl FnOnce(&mut Self::Script, &dyn PathBoundsComputer) -> R,
    ) -> R {
        // SAFETY: We need to work around Rust's borrow checker not knowing that
        // script_host and path_bounds access disjoint fields. We use raw pointers
        // to obtain both references simultaneously.
        let this = self as *mut Self;
        let script = unsafe { &mut *this }.script_host();
        let path_bounds = unsafe { &*this }.path_bounds();
        f(script, path_bounds)
    }

    /// Combined accessor for video_source (mut) and render_engine (immutable).
    fn with_video_and_engine<R>(
        &mut self,
        f: impl FnOnce(&mut Self::Video, &Self::Backend) -> R,
    ) -> R {
        self.with_render_context(|video, engine, _| f(video, engine))
    }

    /// Combined accessor providing video, engine, and platform-specific extra data for backends.
    ///
    /// The default implementation passes `()` as platform_data. Platforms that need to pass
    /// additional data to their backend (e.g. engine's `AssetCatalog`) should override this.
    fn with_render_context<R>(
        &mut self,
        f: impl FnOnce(&mut Self::Video, &Self::Backend, &mut dyn std::any::Any) -> R,
    ) -> R {
        let this = self as *mut Self;
        let video = unsafe { &mut *this }.video_source();
        let backend = unsafe { &*this }.render_engine();
        f(video, backend, &mut ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_ctx::ScriptFrameCtx;
    use crate::platform::backend::BackendTypes;
    use crate::platform::render_engine::{FrameView, GlyphPaint, RecordCtx, RenderCtx};
    use crate::platform::video::{FrameBitmap, VideoFrameProvider};
    use crate::resource::asset_id::AssetId;
    use crate::runtime::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle};
    use crate::display::list::{DisplayItem, DisplayRect};
    use crate::text::GlyphData;
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
        fn draw_scene_snapshot(&self, _: &Self::Picture, _: FrameView<'_>) -> Result<()> {
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
        fn draw_ordered_scene(&self, _: &mut RenderCtx<'_, Self>, _: FrameView<'_>) -> Result<()>
        where
            Self: Sized,
        {
            Ok(())
        }
        fn record_subtree_snapshot(
            &self, _: &mut RecordCtx<'_, Self>, _: &AnnotatedDisplayTree, _: AnnotatedNodeHandle,
        ) -> Result<Self::Picture> { Ok("subtree_snap".into()) }
        fn record_subtree_image(&self, _: &Self::Picture, _: DisplayRect) -> Result<Self::Image> {
            Ok("subtree_img".into())
        }
        fn draw_subtree_snapshot(
            &self, _: &Self::Picture, _: f32, _: Option<f32>, _: DisplayRect, _: FrameView<'_>,
        ) -> Result<()> { Ok(()) }
        fn draw_subtree_image(
            &self, _: &Self::Image, _: f32, _: Option<f32>, _: DisplayRect, _: FrameView<'_>,
        ) -> Result<()> { Ok(()) }
        fn record_item_picture(
            &self, _: &mut RecordCtx<'_, Self>, _: &DisplayItem,
        ) -> Result<Self::Picture> { Ok("item_pic".into()) }
        fn draw_item_picture(
            &self, _: &Self::Picture, _: (f32, f32), _: FrameView<'_>,
        ) -> Result<()> { Ok(()) }
        fn rasterize_glyph_path(&self, _: &GlyphData) -> Result<Self::GlyphPath> { Ok("glyph_path".into()) }
        fn rasterize_glyph_image(&self, _: &GlyphData) -> Result<Self::GlyphImage> { Ok("glyph_img".into()) }
        fn draw_glyph_path(&self, _: &Self::GlyphPath, _: &GlyphPaint, _: FrameView<'_>) -> Result<()> { Ok(()) }
        fn draw_glyph_image(&self, _: &Self::GlyphImage, _: DisplayRect, _: FrameView<'_>) -> Result<()> { Ok(()) }
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
