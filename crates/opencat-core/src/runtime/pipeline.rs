//! Per-frame pipeline — core manages the entire chain from element resolve to backend draw.

use std::any::Any;

use anyhow::Result;

use crate::display::build::build_display_tree;
use crate::element::resolve::resolve_ui_tree_with_script_cache;
use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::platform::platform::Platform;
use crate::platform::render_engine::{FrameView, RecordCtx, RenderCtx, RenderEngine};
use crate::runtime::annotation::{annotate_display_tree, compute_display_tree_fingerprints};
use crate::runtime::compositor::{OrderedSceneProgram, plan_for_scene};
use crate::runtime::invalidation::mark_display_tree_composite_dirty;
use crate::runtime::session::RenderSession;
use crate::scene::composition::Composition;
use crate::text::DefaultFontProvider;

/// Run the full per-frame pipeline: resolve → layout → display tree → annotate → plan → render.
pub fn render_frame<P: Platform>(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession<P>,
    frame_view: FrameView<'_>,
    platform_data: &mut dyn Any,
) -> Result<()> {
    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };

    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);

    // 1. element resolve (with script) — uses with_script_and_bounds to avoid borrow conflict
    let root = composition.root_node(&frame_ctx);
    let element_root = session
        .platform
        .with_script_and_bounds(|script_host, path_bounds| {
            resolve_ui_tree_with_script_cache(
                &root,
                &frame_ctx,
                &script_frame_ctx,
                &mut session.catalog,
                None,
                script_host,
                path_bounds,
            )
        })?;

    // 2. layout
    let provider = DefaultFontProvider::from_arc(session.font_db.clone());
    let (layout_tree, layout_pass) = session.layout_session.compute_layout_with_provider(
        &element_root,
        &frame_ctx,
        &provider,
    )?;

    // 3. display tree + annotation + fingerprint
    let display_tree = build_display_tree(&element_root, &layout_tree)?;
    let mut annotated = annotate_display_tree(&display_tree);
    mark_display_tree_composite_dirty(
        &mut session.composite_history,
        &mut annotated,
        layout_pass.structure_rebuild,
    );
    compute_display_tree_fingerprints(&mut annotated);

    // 4. plan
    let scene_plan = plan_for_scene(&layout_pass, annotated.contains_time_variant());

    // 5/6. snapshot cache decision
    if scene_plan.allows_scene_snapshot_cache {
        if let Some(snapshot) = session.scene_snapshots.scene_snapshot() {
            return session
                .platform
                .render_engine()
                .draw_scene_snapshot(&snapshot, frame_view);
        }
        let snapshot = session.platform.with_video_and_engine(|video, backend| {
            let mut ctx = RecordCtx {
                catalog: &session.catalog,
                frame_ctx: &frame_ctx,
                cache: &mut session.cache_registry,
                video,
                platform_data,
            };
            backend.record_display_tree_snapshot(&mut ctx, &annotated)
        })?;
        session
            .scene_snapshots
            .store_scene_snapshot(Some(snapshot.clone()));
        return session
            .platform
            .render_engine()
            .draw_scene_snapshot(&snapshot, frame_view);
    }

    // 6. ordered scene direct draw
    session.scene_snapshots.store_scene_snapshot(None);
    let ordered_scene = OrderedSceneProgram::build(&annotated);
    session.platform.with_video_and_engine(|video, backend| {
        let mut ctx = RenderCtx {
            catalog: &session.catalog,
            frame_ctx: &frame_ctx,
            display_tree: &annotated,
            ordered_scene: &ordered_scene,
            cache: &mut session.cache_registry,
            video,
            platform_data,
        };
        backend.draw_ordered_scene(&mut ctx, frame_view)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_ctx::ScriptFrameCtx;
    use crate::platform::backend::BackendTypes;
    use crate::platform::render_engine::{
        FrameView, FrameViewKind, RecordCtx, RenderCtx, RenderEngine,
    };
    use crate::platform::video::{FrameBitmap, VideoFrameProvider};
    use crate::resource::asset_id::AssetId;
    use crate::runtime::annotation::AnnotatedDisplayTree;
    use crate::scene::path_bounds::{DefaultPathBounds, PathBoundsComputer};
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

    #[test]
    fn render_frame_runs_one_frame_on_minimal_composition() {
        use crate::scene::primitives::div;
        use crate::style::ColorToken;

        let scene = div().id("root").w_full().h_full().bg(ColorToken::Black);
        let composition = Composition::new("pipeline_test")
            .size(4, 4)
            .fps(1)
            .frames(1)
            .root(move |_ctx: &FrameCtx| scene.clone().into())
            .build()
            .expect("composition should build");

        let platform = MockPlatform {
            backend: MockBackend,
            script: MockScript,
            video: MockVideo,
            path_bounds: DefaultPathBounds,
        };
        let mut session = RenderSession::new(platform);

        let mut frame_view_data: Box<dyn Any> = Box::new(());
        let frame_view = FrameView {
            width: 4,
            height: 4,
            kind: FrameViewKind::Opaque(&mut *frame_view_data),
        };
        let mut platform_data: Box<dyn Any> = Box::new(());

        let result = render_frame(
            &composition,
            0,
            &mut session,
            frame_view,
            &mut *platform_data,
        );
        assert!(
            result.is_ok(),
            "render_frame should succeed: {:?}",
            result.err()
        );
    }
}
