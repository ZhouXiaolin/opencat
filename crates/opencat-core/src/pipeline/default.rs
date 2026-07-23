use std::sync::Arc;

use anyhow::Result;

use crate::analyze::annotation::{AnalyzeFingerprintHistory, AnnotatedNodeHandle};
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::analyze::invalidation::CompositeHistory;
use crate::display::build::DisplayBuildSession;
use crate::ir::cache::RenderCache;
use crate::ir::{CompositionInfo, GeneratedImageTable, RenderFrame};
use crate::layout::LayoutSession;
use crate::parse::composition::Composition;
use crate::probe::catalog::PreparedResourceCatalog;
use crate::script::js_context::JsContext;

use super::Pipeline;

const DEFAULT_NODE_OWN_CAP: usize = 256;
const DEFAULT_SEGMENT_CAP: usize = 256;
const DEFAULT_ITEM_RANGE_CAP: usize = 128;

/// The core rendering pipeline.
///
/// Pure derivation kernel: host-prepared [`PreparedResourceCatalog`], font db,
/// and parsed composition in; deterministic [`RenderFrame`] out. No loader,
/// fetcher, cache, or decoder. Production hosts open via
/// [`crate::lifecycle::PreparedComposition::open_pipeline`].
pub struct DefaultPipeline<S: JsContext> {
    composition: Composition,
    info: CompositionInfo,
    catalog: PreparedResourceCatalog,
    scripts: crate::script::ScriptRealm<S>,
    layout_session: LayoutSession,
    display_build_session: DisplayBuildSession,
    composite_history: CompositeHistory,
    analyze_fingerprint_history: AnalyzeFingerprintHistory,
    font_db: Arc<fontdb::Database>,
    cache: RenderCache,
    last_ordered_scene: OrderedSceneProgram,
    /// Core-rasterized images (color-emoji bitmap glyphs) for the lifetime of
    /// this pipeline. Stable ids make fresh vs reused pipelines produce
    /// identical tables; RGBA is stored here so it is not dropped at text-render
    /// time. Hosts consume the current frame's entries via
    /// [`crate::ir::FrameMediaPlan::generated_images`], not this field.
    generated_images: GeneratedImageTable,
}

impl<S: JsContext> DefaultPipeline<S> {
    /// Open a pipeline from lifecycle-prepared inputs. Crate-private — hosts must
    /// go through [`crate::lifecycle::PreparedComposition::open_pipeline`].
    ///
    /// Does **no** fetch, cache, decode, or probe work — the catalog's metadata is
    /// exactly what prepare already validated. Core only derives layout and
    /// `RenderFrame` output from these inputs.
    pub(crate) fn open_with_prepared_catalog(
        parsed: crate::parse::ParsedComposition,
        catalog: PreparedResourceCatalog,
        scripts: S,
        font_db: Arc<fontdb::Database>,
    ) -> Result<Self> {
        let (composition, info, live_host) = build_pipeline_state(parsed, scripts)?;

        Ok(Self {
            composition,
            info,
            catalog,
            scripts: live_host,
            layout_session: LayoutSession::new(),
            display_build_session: DisplayBuildSession::new(),
            composite_history: CompositeHistory::default(),
            analyze_fingerprint_history: AnalyzeFingerprintHistory::default(),
            font_db,
            cache: RenderCache::new(
                DEFAULT_NODE_OWN_CAP,
                DEFAULT_SEGMENT_CAP,
                DEFAULT_ITEM_RANGE_CAP,
            ),
            last_ordered_scene: OrderedSceneProgram {
                root: OrderedSceneOp::LiveSubtree {
                    handle: AnnotatedNodeHandle(0),
                    children: Vec::new(),
                },
            },
            generated_images: GeneratedImageTable::new(),
        })
    }

    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    pub fn catalog(&self) -> &PreparedResourceCatalog {
        &self.catalog
    }

    pub fn scripts(&self) -> &crate::script::ScriptRealm<S> {
        &self.scripts
    }

    /// Inspect layout for a frame using the same resolve/layout sessions as
    /// [`Pipeline::render_frame`]. Does not re-seed catalogs or open a second
    /// script realm — host-prepared metadata and pipeline state are authoritative.
    pub fn inspect_frame(
        &mut self,
        frame_index: u32,
    ) -> Result<Vec<super::inspect::FrameElementRect>> {
        let evaluation = self.evaluate_frame(frame_index)?;
        super::inspect::collect_frame_element_rects(
            &evaluation.source_root,
            &evaluation.element_root,
            &evaluation.layout_tree,
        )
    }

    /// Shared resolve + layout evaluation used by both render and inspection.
    /// Crate-private: hosts use [`Self::inspect_frame`] / [`Pipeline::render_frame`].
    pub(crate) fn evaluate_frame(
        &mut self,
        frame_index: u32,
    ) -> Result<super::frame::FrameEvaluation> {
        super::frame::evaluate_frame_layout(
            &self.composition,
            frame_index,
            &mut self.layout_session,
            &self.font_db,
            &mut self.catalog,
            &mut self.scripts,
        )
    }

    /// Internal generated-image table used while building `RenderFrame`.
    ///
    /// Hosts must not depend on this accessor: full RGBA for the current frame
    /// is carried in [`crate::ir::FrameMediaPlan::generated_images`]. Kept
    /// `pub(crate)` only for core tests that inspect table collision semantics.
    #[cfg(test)]
    pub(crate) fn generated_images(&self) -> &GeneratedImageTable {
        &self.generated_images
    }
}

/// Build the pipeline state shared by every entry: the [`Composition`] (with
/// the parsed root frozen into its closure), the [`CompositionInfo`], and the
/// live script host.
///
/// This owns no fetch/probe/loader logic — it is pure derivation from the
/// parsed composition. The caller is responsible for producing the
/// [`PreparedResourceCatalog`]: the host builds it before opening the pipeline.
fn build_pipeline_state<S: JsContext>(
    parsed: crate::parse::ParsedComposition,
    scripts: S,
) -> Result<(Composition, CompositionInfo, crate::script::ScriptRealm<S>)> {
    let root_node = parsed.root;
    let composition = Composition::new("pipeline")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .duration(parsed.duration)
        .root(move |_ctx| root_node.clone())
        .audio_sources(parsed.audio_sources)
        .build()?;

    let audio_plan = crate::media::collect_audio_plan(&composition);
    let info = CompositionInfo {
        width: composition.width as u32,
        height: composition.height as u32,
        fps: composition.fps,
        duration: composition.duration,
        audio_plan,
    };

    // One script realm per pipeline: same composition shares JS state; separate
    // pipelines never share ctx / dispatcher / globals (issue #20).
    let realm = crate::script::ScriptRealm::new(scripts)?;

    Ok((composition, info, realm))
}

impl<S: JsContext> Pipeline for DefaultPipeline<S> {
    type Scripts = S;

    fn info(&self) -> &CompositionInfo {
        &self.info
    }

    fn render_frame(&mut self, frame_index: u32) -> Result<RenderFrame> {
        let (draw, media) = super::frame::render_frame_with_state(
            &self.composition,
            frame_index,
            &mut self.layout_session,
            &mut self.display_build_session,
            &mut self.composite_history,
            &mut self.analyze_fingerprint_history,
            &self.font_db,
            &mut self.catalog,
            &mut self.cache,
            &mut self.last_ordered_scene,
            &mut self.scripts,
            &mut self.generated_images,
        )?;
        Ok(RenderFrame { draw, media })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;
    use crate::ir::DrawOpFrame;
    use crate::script::js_context::JsContext;
    use crate::script::recorder::MutationStore;

    struct NoopJsContext {
        store: std::cell::RefCell<MutationStore>,
    }
    impl JsContext for NoopJsContext {
        fn new() -> Result<Self> {
            Ok(Self {
                store: MutationStore::default().into(),
            })
        }
        fn eval(&self, _code: &str) -> Result<()> {
            Ok(())
        }
        fn set_ctx_field(&self, _name: &str, _v: serde_json::Value) -> Result<()> {
            Ok(())
        }
        fn call_global_fn(&self, _name: &str) -> Result<()> {
            Ok(())
        }
        fn install_dispatcher<F>(&self, _dispatcher: F) -> Result<()>
        where
            F: Fn(&mut MutationStore, &str, &[serde_json::Value]) -> Result<serde_json::Value>
                + 'static,
        {
            Ok(())
        }
        fn rebind_dispatcher(&self) -> Result<()> {
            Ok(())
        }
        fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
            f(&mut *self.store.borrow_mut())
        }
    }

    /// Test helper: open a media-less pipeline via the crate-private open entry
    /// (same inputs lifecycle prepare would hand to it). Empty-byte
    /// Empty-byte catalog omits all assets — render-seam tests only, not decode.
    fn open_test_pipeline(input: &str) -> DefaultPipeline<NoopJsContext> {
        let trimmed = input.trim();
        let parsed = if trimmed.starts_with('{') {
            crate::parse::jsonl::parse(input).expect("parse input")
        } else {
            crate::parse::markup::parse(input).expect("parse input")
        };
        let catalog = crate::probe::PreparedResourceCatalog::default();
        let font_db = crate::text::font_db_from_bytes(
            &crate::test_support::test_font_faces(),
            "Noto Sans SC",
        );
        DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            NoopJsContext::new().expect("js context"),
            Arc::new(font_db),
        )
        .expect("open test pipeline")
    }

    fn test_font_db() -> Arc<fontdb::Database> {
        Arc::new(crate::text::font_db_from_bytes(
            &crate::test_support::test_font_faces(),
            "Noto Sans SC",
        ))
    }

    #[test]
    fn open_empty_composition_returns_info() {
        let jsonl = r#"{"type":"composition","width":100,"height":200,"fps":30,"duration":0.033333333333}
{"type":"div","id":"root","parentId":null}"#;

        let pipeline = open_test_pipeline(jsonl);

        assert_eq!(pipeline.info().width, 100);
        assert_eq!(pipeline.info().height, 200);
        assert_eq!(pipeline.info().fps, 30);
        assert!((pipeline.info().duration - 1.0 / 30.0).abs() < 1e-9);
    }

    #[test]
    fn render_frame_produces_draw_ops() {
        let jsonl = r##"{"type":"composition","width":320,"height":240,"fps":30,"duration":0.1}
{"type":"div","id":"root","parentId":null}
{"type":"div","id":"child","parentId":"root","bg":"#ff0000","w":100,"h":50}"##;

        let mut pipeline = open_test_pipeline(jsonl);

        let frame = pipeline.render_frame(0).expect("render frame 0");

        assert!(
            !frame.draw.ops.is_empty(),
            "render_frame should produce at least one DrawOp"
        );
        let _ = frame.media;
    }

    #[test]
    fn render_frame_video_ref_uses_media_start_time() {
        let xml = r#"<opencat width="320" height="180" fps="30" duration="4">
  <div id="root" class="w-[320px] h-[180px]">
    <video id="vid" class="w-[320px] h-[180px]" path="clip.mp4" data-start="3" data-duration="18" data-media-start="12" />
  </div>
</opencat>"#;

        let mut pipeline = open_test_pipeline(xml);

        let frame = pipeline.render_frame(90).expect("render frame 90");

        let crate::ir::draw_types::ImageRef::VideoFrame { time_micros, .. } =
            &frame.media.video_frames[0]
        else {
            panic!("expected video frame image ref");
        };
        assert_eq!(*time_micros, 12_000_000);
    }

    #[test]
    fn render_frame_video_timing_inside_later_scene_uses_scene_local_data_start() {
        let xml = r#"<opencat width="320" height="180" fps="10" duration="4.1">
  <tl id="main" class="w-[320px] h-[180px]">
    <div id="scene-1" class="w-[320px] h-[180px]" duration="2" />
    <transition from="scene-1" to="scene-2" effect="fade" duration="0.1" />
    <div id="scene-2" class="w-[320px] h-[180px]" duration="2">
      <video id="vid" class="w-[320px] h-[180px]" path="clip.mp4" data-start="0.5" data-duration="1.5" data-media-start="12" />
    </div>
  </tl>
</opencat>"#;

        let mut pipeline = open_test_pipeline(xml);

        let before_frame = pipeline
            .render_frame(25)
            .expect("render frame before data-start");
        assert!(
            before_frame.media.video_frames.is_empty(),
            "data-start is local to scene-2, so the clip should still be hidden"
        );

        let start_frame = pipeline
            .render_frame(26)
            .expect("render frame at data-start");
        let crate::ir::draw_types::ImageRef::VideoFrame {
            time_micros: start_time_micros,
            ..
        } = &start_frame.media.video_frames[0]
        else {
            panic!("expected video frame at scene-local data-start");
        };
        assert_eq!(*start_time_micros, 12_000_000);

        let later_frame = pipeline.render_frame(31).expect("render later frame");
        let crate::ir::draw_types::ImageRef::VideoFrame {
            time_micros: later_time_micros,
            ..
        } = &later_frame.media.video_frames[0]
        else {
            panic!("expected video frame after scene-local data-start");
        };
        assert_eq!(*later_time_micros, 12_500_000);
    }

    #[test]
    fn render_frame_video_data_start_hides_entire_node_before_start() {
        fn material_draw_op_count(frame: &DrawOpFrame) -> usize {
            frame
                .ops
                .iter()
                .filter(|op| {
                    matches!(
                        op,
                        crate::ir::draw_op::DrawOp::Paint { .. }
                            | crate::ir::draw_op::DrawOp::Rect { .. }
                            | crate::ir::draw_op::DrawOp::RRect { .. }
                            | crate::ir::draw_op::DrawOp::DRRect { .. }
                            | crate::ir::draw_op::DrawOp::Oval { .. }
                            | crate::ir::draw_op::DrawOp::Circle { .. }
                            | crate::ir::draw_op::DrawOp::Arc { .. }
                            | crate::ir::draw_op::DrawOp::Line { .. }
                            | crate::ir::draw_op::DrawOp::Points { .. }
                            | crate::ir::draw_op::DrawOp::DrawPath { .. }
                            | crate::ir::draw_op::DrawOp::Image { .. }
                            | crate::ir::draw_op::DrawOp::ImageRect { .. }
                            | crate::ir::draw_op::DrawOp::LottieRect { .. }
                            | crate::ir::draw_op::DrawOp::RuntimeEffect { .. }
                    )
                })
                .count()
        }

        let xml = r#"<opencat width="320" height="180" fps="30" duration="6">
  <div id="root" class="w-[320px] h-[180px]">
    <video id="vid" class="relative w-[320px] h-[180px] bg-[#ff0000] border-[4px] border-[#00ff00] shadow-[0_8px_24px_rgba(0,0,0,0.50)]" path="clip.mp4" data-start="3" data-duration="1" data-media-start="12">
      <div id="badge" class="absolute left-[8px] top-[8px] w-[40px] h-[24px] bg-[#0000ff]" />
    </video>
  </div>
</opencat>"#;

        let mut pipeline = open_test_pipeline(xml);

        let before_frame = pipeline.render_frame(0).expect("render frame 0");
        assert!(
            before_frame.media.video_frames.is_empty(),
            "video should not request a frame before data-start"
        );
        assert_eq!(
            material_draw_op_count(&before_frame.draw),
            0,
            "video node paint and children should be entirely hidden before data-start"
        );

        let after_frame = pipeline.render_frame(90).expect("render frame 90");
        assert!(
            !after_frame.media.video_frames.is_empty(),
            "video should request a frame at data-start"
        );
        assert!(
            material_draw_op_count(&after_frame.draw) > 0,
            "video node paint should be visible at data-start"
        );

        let after_duration_frame = pipeline.render_frame(150).expect("render frame 150");
        assert!(
            !after_duration_frame.media.video_frames.is_empty(),
            "data-duration should not hide the video subtree after it ends"
        );
        let crate::ir::draw_types::ImageRef::VideoFrame { time_micros, .. } =
            &after_duration_frame.media.video_frames[0]
        else {
            panic!("expected video frame image ref after data-duration");
        };
        assert_eq!(
            *time_micros, 13_000_000,
            "data-duration should clamp media time only"
        );
        assert!(
            material_draw_op_count(&after_duration_frame.draw) > 0,
            "video subtree should remain visible after data-duration ends"
        );
    }

    #[test]
    fn render_frame_multi_frame_is_deterministic() {
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":10,"duration":0.5}
{"type":"div","id":"root","parentId":null,"bg":"#00ff00","w":100,"h":100}"##;

        let mut p1 = open_test_pipeline(jsonl);
        let mut p2 = open_test_pipeline(jsonl);

        for i in 0..5 {
            let r1 = p1.render_frame(i).expect("render p1");
            let r2 = p2.render_frame(i).expect("render p2");
            assert_eq!(
                r1.draw.ops.len(),
                r2.draw.ops.len(),
                "frame {i} op count mismatch"
            );
        }
    }

    /// AC #7: the same pipeline instance must produce field-by-field identical
    /// `RenderFrame` output for a given frame whether rendered directly, after
    /// rendering other frames out of order, or repeated. Call history must not
    /// affect the deterministic per-frame contract.
    #[test]
    fn render_frame_is_order_and_repeat_invariant() {
        let xml = r#"<opencat width="320" height="180" fps="30" duration="4">
  <div id="root" class="w-[320px] h-[180px]">
    <video id="vid" class="w-[320px] h-[180px]" path="clip.mp4" data-start="3" data-duration="18" data-media-start="12" />
  </div>
</opencat>"#;

        let mut pipeline = open_test_pipeline(xml);

        let target = 90_u32; // well past data-start, so the video ref is active

        // (1) Fresh pipeline renders the target frame directly.
        let baseline = pipeline.render_frame(target).expect("baseline render");

        // (2) Same pipeline renders other frames out of order, then returns to
        //     the target frame — must equal the baseline field-by-field.
        let _ = pipeline.render_frame(10).expect("render 10");
        let _ = pipeline.render_frame(2).expect("render 2");
        let after_out_of_order = pipeline
            .render_frame(target)
            .expect("render target after out-of-order");

        assert_eq!(
            baseline.draw.ops, after_out_of_order.draw.ops,
            "draw ops must be identical regardless of call history"
        );
        assert_eq!(
            baseline.media.images, after_out_of_order.media.images,
            "media plan images must be identical regardless of call history"
        );
        assert_eq!(
            baseline.media.video_frames, after_out_of_order.media.video_frames,
            "media plan video frames must be identical regardless of call history"
        );
        assert_eq!(
            baseline.media.lottie_bundles, after_out_of_order.media.lottie_bundles,
            "media plan lottie bundles must be identical regardless of call history"
        );
        assert_eq!(
            baseline.media.runtime_effects, after_out_of_order.media.runtime_effects,
            "media plan runtime effects must be identical regardless of call history"
        );
        assert_eq!(
            baseline.media.generated_images, after_out_of_order.media.generated_images,
            "media plan generated images (id/size/RGBA) must be identical regardless of call history"
        );

        // (3) Render the target frame again immediately — must still be identical
        //     across every media-plan category, not just the draw ops.
        let repeated = pipeline.render_frame(target).expect("repeat render");
        assert_eq!(
            baseline.draw.ops, repeated.draw.ops,
            "draw ops must be identical on repeat render"
        );
        assert_eq!(
            baseline.media.images, repeated.media.images,
            "media plan images must be identical on repeat render"
        );
        assert_eq!(
            baseline.media.video_frames, repeated.media.video_frames,
            "media plan video frames must be identical on repeat render"
        );
        assert_eq!(
            baseline.media.lottie_bundles, repeated.media.lottie_bundles,
            "media plan lottie bundles must be identical on repeat render"
        );
        assert_eq!(
            baseline.media.runtime_effects, repeated.media.runtime_effects,
            "media plan runtime effects must be identical on repeat render"
        );
        assert_eq!(
            baseline.media.generated_images, repeated.media.generated_images,
            "media plan generated images must be identical on repeat render"
        );
    }

    /// #21: FrameMediaPlan carries full generated-image RGBA; fresh pipelines,
    /// out-of-order reuse, and immediate repeats are field-identical for
    /// color-emoji glyphs (stable id + size + bytes).
    #[test]
    fn render_frame_generated_images_are_complete_and_deterministic() {
        let xml = r#"<opencat width="96" height="80" fps="30" duration="0.2">
  <div id="root" class="w-[96px] h-[80px] bg-white">
    <text id="emoji" class="absolute left-[16px] top-[12px] w-[64px] h-[64px] text-[48px] text-black">😀</text>
  </div>
</opencat>"#;

        let mut fresh = open_test_pipeline(xml);
        let baseline = fresh.render_frame(0).expect("fresh frame 0");
        assert!(
            !baseline.media.generated_images.is_empty(),
            "color emoji must produce at least one generated image on RenderFrame"
        );
        for g in &baseline.media.generated_images {
            assert!(g.width > 0 && g.height > 0);
            assert_eq!(
                g.rgba.len(),
                g.width as usize * g.height as usize * 4,
                "RGBA length must match width*height*4 for {:?}",
                g.id
            );
        }

        // Second independent pipeline (fresh) must match field-by-field.
        let mut other = open_test_pipeline(xml);
        let other_frame = other.render_frame(0).expect("other fresh frame 0");
        assert_eq!(
            baseline.media.generated_images, other_frame.media.generated_images,
            "fresh pipelines must produce identical generated-image content"
        );

        // Out-of-order + repeat on the same pipeline.
        let _ = fresh.render_frame(1).expect("frame 1");
        let after = fresh.render_frame(0).expect("frame 0 after out-of-order");
        assert_eq!(
            baseline.media.generated_images, after.media.generated_images,
            "out-of-order reuse must not change generated-image content"
        );
        let again = fresh.render_frame(0).expect("frame 0 repeat");
        assert_eq!(
            baseline.media.generated_images, again.media.generated_images,
            "repeat must not change generated-image content"
        );

        // Dedup: the same glyph id appears at most once in the plan.
        let mut seen = std::collections::HashSet::new();
        for g in &baseline.media.generated_images {
            assert!(
                seen.insert(g.id),
                "generated image id {:?} must be unique in plan",
                g.id
            );
        }
    }

    /// #16: boundary / out-of-order / repeated frames all yield the same
    /// authoritative `time_micros` for a given composition frame index.
    #[test]
    fn render_frame_video_time_micros_boundary_and_repeat() {
        use crate::ir::draw_types::ImageRef;

        let xml = r#"<opencat width="320" height="180" fps="30" duration="5">
  <div id="root" class="w-[320px] h-[180px]">
    <video id="vid" class="w-[320px] h-[180px]" path="clip.mp4" data-media-start="1.5" />
  </div>
</opencat>"#;
        let mut pipeline = open_test_pipeline(xml);

        // Frame 0 → 1.5s → 1_500_000 µs
        let f0 = pipeline.render_frame(0).expect("f0");
        let ImageRef::VideoFrame {
            time_micros: t0, ..
        } = &f0.media.video_frames[0]
        else {
            panic!("expected video frame at 0");
        };
        assert_eq!(*t0, 1_500_000);

        // Frame 45 (1.5s composition) → media 3.0s
        let f45 = pipeline.render_frame(45).expect("f45");
        let ImageRef::VideoFrame {
            time_micros: t45, ..
        } = &f45.media.video_frames[0]
        else {
            panic!("expected video frame at 45");
        };
        assert_eq!(*t45, 3_000_000);

        // Out of order back to 0, then repeat — same micros.
        let f0_again = pipeline.render_frame(0).expect("f0 again");
        let ImageRef::VideoFrame {
            time_micros: t0b, ..
        } = &f0_again.media.video_frames[0]
        else {
            panic!("expected video frame at 0 again");
        };
        assert_eq!(*t0b, *t0);
        let f0_repeat = pipeline.render_frame(0).expect("f0 repeat");
        let ImageRef::VideoFrame {
            time_micros: t0c, ..
        } = &f0_repeat.media.video_frames[0]
        else {
            panic!("expected video frame on repeat");
        };
        assert_eq!(*t0c, *t0);
    }

    #[cfg(feature = "profile")]
    #[test]
    fn render_frame_emits_profile_events_for_each_frame() {
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":10,"duration":0.2}
{"type":"div","id":"root","parentId":null,"bg":"#00ff00","w":100,"h":100}"##;

        let config = crate::profile::ProfileConfig { enabled: true };
        let (_, summary) = crate::profile::profile_render(&config, || {
            let mut pipeline = open_test_pipeline(jsonl);

            for frame_index in 0..2 {
                let _ = pipeline.render_frame(frame_index)?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .expect("profile render");

        let summary = summary.expect("summary should exist");
        assert!(
            summary.frames.contains_key(&0),
            "frame 0 profile should be present, got {:?}",
            summary.frames.keys().collect::<Vec<_>>()
        );
        assert!(
            summary.frames.contains_key(&1),
            "frame 1 profile should be present, got {:?}",
            summary.frames.keys().collect::<Vec<_>>()
        );
        assert_eq!(summary.frames[&0].structure_rebuilds, 1);
        assert_eq!(summary.frames[&1].structure_rebuilds, 0);
        assert!(
            summary.frames[&1].reused_nodes > 0,
            "second frame should record layout reuse stats"
        );
    }

    #[cfg(feature = "profile")]
    #[test]
    fn profile_showcase_jsonl_records_split_merkle_profile() {
        let jsonl = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../json/profile-showcase.jsonl"
        ));

        let config = crate::profile::ProfileConfig { enabled: true };
        let (_, summary) = crate::profile::profile_render(&config, || {
            let mut pipeline = open_test_pipeline(jsonl);

            for frame_index in 0..pipeline.composition().frames {
                let _ = pipeline.render_frame(frame_index)?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .expect("profile render");

        let summary = summary.expect("summary should exist");
        let full_hit_nodes = summary
            .frames
            .values()
            .map(|frame| frame.input_merkle_full_hit_nodes)
            .sum::<usize>();
        let layout_skipped_nodes = summary
            .frames
            .values()
            .map(|frame| frame.layout_merkle_skipped_nodes)
            .sum::<usize>();
        let analyze_skipped_nodes = summary
            .frames
            .values()
            .map(|frame| frame.analyze_merkle_skipped_nodes)
            .sum::<usize>();

        let analyze_recorded_hit_nodes = summary
            .frames
            .values()
            .map(|frame| frame.analyze_recorded_hit_nodes)
            .sum::<usize>();
        let analyze_composite_blocked_nodes = summary
            .frames
            .values()
            .map(|frame| frame.analyze_composite_blocked_nodes)
            .sum::<usize>();
        let analyze_snapshot_eligibility_hit_nodes = summary
            .frames
            .values()
            .map(|frame| frame.analyze_snapshot_eligibility_hit_nodes)
            .sum::<usize>();
        let node_own_segment_hits = summary
            .frames
            .values()
            .map(|frame| frame.backend.node_own_segment_hits)
            .sum::<usize>();
        let node_own_segment_records = summary
            .frames
            .values()
            .map(|frame| frame.backend.node_own_segment_records)
            .sum::<usize>();
        let scene_snapshot_cache_hits = summary
            .frames
            .values()
            .map(|frame| frame.backend.scene_snapshot_cache_hits)
            .sum::<usize>();
        let scene_snapshot_cache_misses = summary
            .frames
            .values()
            .map(|frame| frame.backend.scene_snapshot_cache_misses)
            .sum::<usize>();
        let subtree_snapshot_request_after_analyze_fresh = summary
            .frames
            .values()
            .map(|frame| frame.backend.subtree_snapshot_request_after_analyze_fresh)
            .sum::<usize>();
        let subtree_snapshot_request_after_analyze_reused = summary
            .frames
            .values()
            .map(|frame| frame.backend.subtree_snapshot_request_after_analyze_reused)
            .sum::<usize>();
        let subtree_snapshot_request_after_analyze_composite_blocked = summary
            .frames
            .values()
            .map(|frame| {
                frame
                    .backend
                    .subtree_snapshot_request_after_analyze_composite_blocked
            })
            .sum::<usize>();

        assert!(
            full_hit_nodes > 0,
            "profile-showcase jsonl should exercise full input Merkle hits"
        );
        assert!(
            layout_skipped_nodes >= full_hit_nodes,
            "layout Merkle skip should include full hits and layout-only clean subtrees"
        );
        assert!(
            analyze_skipped_nodes > 0,
            "profile-showcase jsonl should exercise analyze Merkle fingerprint skips"
        );
        assert!(
            analyze_recorded_hit_nodes > 0,
            "analyze_recorded_hit_nodes should be > 0 in the showcase scene"
        );
        assert!(
            node_own_segment_hits + node_own_segment_records > 0,
            "node_own_segment_hits + node_own_segment_records should be > 0 in the showcase scene"
        );
        // Scene snapshot cache should fire at least once on idle stretches.
        assert!(
            scene_snapshot_cache_hits > 0,
            "scene_snapshot_cache_hits should be > 0 in the showcase scene"
        );
        assert!(
            scene_snapshot_cache_misses > 0,
            "scene_snapshot_cache_misses should be > 0 in the showcase scene"
        );
        assert!(
            subtree_snapshot_request_after_analyze_fresh > 0,
            "subtree_snapshot_request_after_analyze_fresh should be > 0 in the showcase scene"
        );
        // `request_after_reused` requires render dispatch to read AnalyzeReuse marks.
        // In the test environment (crate-private open, no assets), the showcase collapses
        // to a near-static scene where scene_snapshot_hit short-circuits most frames,
        // leaving subtree dispatch only on the first few "warm-up" misses where every
        // subtree is fresh. Keep the read so the counter is exercised end-to-end.
        let _ = subtree_snapshot_request_after_analyze_reused;

        assert_eq!(
            analyze_recorded_hit_nodes,
            analyze_snapshot_eligibility_hit_nodes + analyze_composite_blocked_nodes,
            "recorded_hit_nodes should equal snapshot_eligibility_hit_nodes + composite_blocked_nodes"
        );

        let _ = subtree_snapshot_request_after_analyze_composite_blocked;
    }

    #[cfg(feature = "profile")]
    #[test]
    fn cache_hits_scene_snapshot_on_static_repeat() {
        // Static composition with no animations: frame 1 should reuse the
        // entire DrawOpFrame recorded on frame 0.
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":10,"duration":0.2}
{"type":"div","id":"root","parentId":null,"bg":"#00ff00","w":100,"h":100}"##;

        let config = crate::profile::ProfileConfig { enabled: true };
        let (_, summary) = crate::profile::profile_render(&config, || {
            let mut pipeline = open_test_pipeline(jsonl);

            for frame_index in 0..2 {
                let _ = pipeline.render_frame(frame_index)?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .expect("profile render");

        let summary = summary.expect("summary should exist");
        assert_eq!(
            summary.frames[&0].backend.scene_snapshot_cache_misses, 1,
            "frame 0 should miss scene snapshot cache (structure rebuild)"
        );
        assert_eq!(
            summary.frames[&0].backend.scene_snapshot_cache_hits, 0,
            "frame 0 should not hit scene snapshot cache"
        );
        assert_eq!(
            summary.frames[&1].backend.scene_snapshot_cache_hits, 1,
            "frame 1 should hit scene snapshot cache (static repeat)"
        );
        assert_eq!(
            summary.frames[&1].backend.scene_snapshot_cache_misses, 0,
            "frame 1 should not miss when scene is identical"
        );
    }

    #[cfg(feature = "profile")]
    #[test]
    fn cache_misses_during_native_transition_fade() {
        // Two-scene composition with a native fade transition. Every frame
        // inside the transition window has a different transition progress,
        // so the root subtree fingerprint differs frame-to-frame and the
        // scene snapshot cache must miss across all of them.
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":10,"duration":1}
{"type":"div","id":"root","parentId":null}
{"type":"tl","id":"tl","parentId":"root"}
{"type":"div","id":"scene_a","parentId":"tl","bg":"#ff0000","w":100,"h":100,"duration":0.1}
{"type":"transition","parentId":"tl","from":"scene_a","to":"scene_b","effect":"fade","duration":0.133333333333,"timing":"linear"}
{"type":"div","id":"scene_b","parentId":"tl","bg":"#00ff00","w":100,"h":100,"duration":0.1}"##;

        let config = crate::profile::ProfileConfig { enabled: true };
        let (_, summary) = crate::profile::profile_render(&config, || {
            let mut pipeline = open_test_pipeline(jsonl);

            for frame_index in 0..pipeline.composition().frames {
                let _ = pipeline.render_frame(frame_index)?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .expect("profile render");

        let summary = summary.expect("summary should exist");

        // Once the transition completes the next scene stays stable, so a
        // post-transition frame should be eligible to hit the cache.
        assert!(
            summary.frames[&8].backend.scene_snapshot_cache_hits
                + summary.frames[&9].backend.scene_snapshot_cache_hits
                >= 1,
            "frames after the transition completes should be able to hit the cache"
        );
    }

    #[test]
    fn open_from_xml() {
        let xml = r#"<opencat width="200" height="100" fps="30" duration="0.033333333333">
  <div id="root" />
</opencat>"#;
        let pipeline = open_test_pipeline(xml);
        assert_eq!(pipeline.info().width, 200);
        assert_eq!(pipeline.info().height, 100);
        assert_eq!(pipeline.info().fps, 30);
        assert!((pipeline.info().duration - 1.0 / 30.0).abs() < 1e-9);
    }

    // ---- Crate-private open entry (lifecycle hands the same inputs) -------------
    //
    // Production hosts use PreparedComposition::open_pipeline. These tests drive
    // the crate-private open helper at the highest render seam
    // (`render_frame -> RenderFrame`) with a host-supplied PreparedResourceCatalog.

    use crate::fonts::FontManifest;
    use crate::ir::asset_id::asset_id_for_image;
    use crate::parse::preflight::collect_resource_requests_from_parsed;
    use crate::parse::primitives::image;
    use crate::probe::PreparedResourceCatalog as ProbeResourceCatalog;
    use crate::probe::catalog::ImageMeta;

    /// Build a minimal `ParsedComposition` from a root node, for tests that
    /// drive the pipeline without going through markup/jsonl parsing.
    fn parsed_from_root(
        root: crate::Node,
        width: i32,
        height: i32,
        fps: u32,
        duration: f64,
    ) -> crate::parse::ParsedComposition {
        crate::parse::ParsedComposition {
            width,
            height,
            fps: fps as i32,
            duration,
            root,
            script: None,
            audio_sources: vec![],
            font_manifest: FontManifest::default(),
        }
    }

    /// AC #1, #2, #4, #5: crate-private open renders a frame with no loader.
    /// Empty catalog from prepare-over-no-bytes; `render_frame` still produces ops.
    #[test]
    fn crate_private_open_renders_without_a_loader() {
        // Host side: parse a tree, collect declarative requests, build a catalog
        // with no bytes (every asset omitted).
        let root: crate::Node = crate::parse::primitives::div().id("root").into();
        let parsed = parsed_from_root(root, 320, 240, 30, 0.1);

        let prepared = crate::probe::PreparedResourceCatalog::default();
        // Empty composition declares nothing, so the catalog is empty and the
        // pipeline must still open and render.
        assert!(prepared.images.is_empty());

        let ctx = NoopJsContext::new().expect("js context");
        let mut pipeline =
            DefaultPipeline::open_with_prepared_catalog(parsed, prepared, ctx, test_font_db())
                .expect("open crate-private pipeline");

        let frame = pipeline.render_frame(0).expect("render frame 0");
        assert!(
            !frame.draw.ops.is_empty(),
            "crate-private open should still produce DrawOps"
        );
        let _ = frame.media;
    }

    /// AC #2: the host-supplied catalog is the source of truth. A host that
    /// probed image bytes and stored metadata under the canonical `AssetId`
    /// must see that exact metadata surface from `pipeline.catalog()` — the
    /// pipeline did not re-probe, re-fetch, or invent anything.
    #[test]
    fn crate_private_open_uses_host_supplied_metadata() {
        // A composition that declares one image source.
        let img = image().id("hero").path("/tmp/hero.png");
        let root: crate::Node = crate::parse::primitives::div().id("root").child(img).into();
        let parsed = parsed_from_root(root, 320, 240, 30, 0.1);

        // Host collects the declarative request and derives the canonical id.
        let requests = collect_resource_requests_from_parsed(&parsed);
        let declared = requests.images.iter().next().expect("one image declared");
        let canonical = asset_id_for_image(declared).expect("path source has an id");

        // Host supplies a catalog with metadata it probed itself. The pipeline
        // must take it as-is rather than running the internal probe path.
        let mut catalog = ProbeResourceCatalog::default();
        catalog.images.insert(
            canonical.clone(),
            ImageMeta {
                width: 42,
                height: 17,
            },
        );

        let ctx = NoopJsContext::new().expect("js context");
        let pipeline =
            DefaultPipeline::open_with_prepared_catalog(parsed, catalog, ctx, test_font_db())
                .expect("open crate-private pipeline");

        let stored = pipeline
            .catalog()
            .images
            .get(&canonical)
            .expect("host metadata must survive opening unchanged");
        assert_eq!((stored.width, stored.height), (42, 17));
    }

    /// AC #5: crate-private open is order- and repeat-invariant.
    /// Call history must not leak into the per-frame contract.
    #[test]
    fn crate_private_open_is_order_and_repeat_invariant() {
        let root: crate::Node = crate::parse::primitives::div()
            .id("root")
            .w(100.0)
            .h(100.0)
            .bg_red()
            .into();

        let open_fresh = || -> DefaultPipeline<NoopJsContext> {
            let parsed = parsed_from_root(root.clone(), 100, 100, 10, 0.5);
            let requests = collect_resource_requests_from_parsed(&parsed);
            let catalog = crate::probe::PreparedResourceCatalog::default();
            DefaultPipeline::open_with_prepared_catalog(
                parsed,
                catalog,
                NoopJsContext::new().expect("js context"),
                test_font_db(),
            )
            .expect("open")
        };

        let mut pipeline = open_fresh();
        let target = 4_u32;

        // (1) Fresh pipeline renders the target frame directly.
        let baseline = pipeline.render_frame(target).expect("baseline render");

        // (2) Same pipeline renders other frames out of order, then returns.
        let _ = pipeline.render_frame(2).expect("render 2");
        let _ = pipeline.render_frame(0).expect("render 0");
        let after_out_of_order = pipeline
            .render_frame(target)
            .expect("render target after out-of-order");

        assert_eq!(
            baseline.draw.ops, after_out_of_order.draw.ops,
            "draw ops must be identical regardless of call history"
        );
        assert_eq!(
            baseline.media.images, after_out_of_order.media.images,
            "media plan images must be identical regardless of call history"
        );

        // (3) Repeat the target frame immediately — still identical.
        let repeated = pipeline.render_frame(target).expect("repeat render");
        assert_eq!(
            baseline.draw.ops, repeated.draw.ops,
            "draw ops must be identical on repeat render"
        );
    }

    /// AC #4: crate-private open carries the host font database so shaping/
    /// layout stay usable. Rendering a text frame must not panic.
    #[test]
    fn crate_private_open_carries_font_db() {
        let root: crate::Node = crate::parse::primitives::div()
            .id("root")
            .child(crate::parse::primitives::text("Hi").id("label"))
            .into();
        let parsed = parsed_from_root(root, 200, 80, 30, 0.1);

        let catalog = crate::probe::PreparedResourceCatalog::default();

        let ctx = NoopJsContext::new().expect("js context");
        let mut pipeline =
            DefaultPipeline::open_with_prepared_catalog(parsed, catalog, ctx, test_font_db())
                .expect("open");

        let frame = pipeline.render_frame(0).expect("render text frame");
        assert!(
            !frame.draw.ops.is_empty(),
            "text frame should still emit draw ops under crate-private open"
        );
    }

    // ---- Real pipeline inspection (issue #23) ------------------------------------

    fn sample_card_tree() -> crate::Node {
        use crate::parse::primitives::{div, text};
        div()
            .id("root")
            .w_full()
            .h_full()
            .child(
                div()
                    .id("card")
                    .w(120.0)
                    .h(80.0)
                    .child(text("hi").id("label")),
            )
            .into()
    }

    fn open_inspect_sample() -> DefaultPipeline<NoopJsContext> {
        let parsed = parsed_from_root(sample_card_tree(), 320, 180, 30, 0.1);
        let catalog = crate::probe::PreparedResourceCatalog::default();
        DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            NoopJsContext::new().expect("js"),
            test_font_db(),
        )
        .expect("open")
    }

    /// Same pipeline instance: inspect rects equal collect(evaluate_frame),
    /// and render_frame still succeeds on that instance (shared sessions).
    #[test]
    fn inspect_frame_matches_same_instance_evaluate_intermediates() {
        use crate::pipeline::inspect::collect_frame_element_rects;

        let mut pipeline = open_inspect_sample();
        let evaluation = pipeline.evaluate_frame(0).expect("evaluate");
        let from_intermediates = collect_frame_element_rects(
            &evaluation.source_root,
            &evaluation.element_root,
            &evaluation.layout_tree,
        )
        .expect("collect");

        // Second evaluate on the same pipeline (layout session reused) must
        // still produce rects identical to inspect_frame.
        let inspected = pipeline.inspect_frame(0).expect("inspect");
        assert_eq!(from_intermediates, inspected);

        let card = inspected.iter().find(|r| r.id == "card").expect("card");
        assert!((card.width - 120.0).abs() < 0.5);
        assert!((card.height - 80.0).abs() < 0.5);
        let mut orders: Vec<_> = inspected.iter().map(|r| r.draw_order).collect();
        orders.sort_unstable();
        for (i, order) in orders.iter().enumerate() {
            assert_eq!(*order as usize, i);
        }

        let frame = pipeline.render_frame(0).expect("render after inspect");
        assert!(!frame.draw.ops.is_empty());
    }

    /// Inspection is deterministic across fresh pipelines with the same host inputs.
    #[test]
    fn inspect_frame_is_deterministic_and_matches_layout_sizes() {
        let mut pipeline = open_inspect_sample();
        let inspected = pipeline.inspect_frame(0).expect("inspect");
        let mut pipeline2 = open_inspect_sample();
        let inspected2 = pipeline2.inspect_frame(0).expect("inspect2");
        assert_eq!(inspected, inspected2);
    }

    /// Host-supplied image metadata (not file reads) drives inspect layout sizes.
    #[test]
    fn inspect_frame_uses_host_catalog_not_file_seed() {
        use crate::parse::primitives::{ImageSource, div, image};

        let root: crate::Node = div()
            .id("root")
            .w_full()
            .h_full()
            .child(
                image()
                    .path("missing-on-purpose.png")
                    .id("hero")
                    .w(64.0)
                    .h(32.0),
            )
            .into();
        let parsed = parsed_from_root(root, 200, 100, 30, 0.1);
        let mut catalog = crate::probe::PreparedResourceCatalog::default();
        if let Some(id) = asset_id_for_image(&ImageSource::Path("missing-on-purpose.png".into())) {
            catalog.images.insert(
                id,
                ImageMeta {
                    width: 64,
                    height: 32,
                },
            );
        }

        let mut pipeline = DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            NoopJsContext::new().expect("js"),
            test_font_db(),
        )
        .expect("open");

        let rects = pipeline.inspect_frame(0).expect("inspect with host meta");
        let hero = rects.iter().find(|r| r.id == "hero").expect("hero");
        assert!((hero.width - 64.0).abs() < 0.5);
        assert!((hero.height - 32.0).abs() < 0.5);
        assert_eq!(hero.kind, "image");
    }

    /// Script-bearing nodes keep script_source on inspect rects from the same
    /// pipeline state used for render (no second script realm).
    #[test]
    fn inspect_frame_carries_script_source_from_source_tree() {
        use crate::parse::primitives::div;
        use crate::script::ScriptDriver;

        let driver = ScriptDriver::from_source("/* noop */").expect("script");
        let root: crate::Node = div()
            .id("root")
            .w_full()
            .h_full()
            .script_driver(driver)
            .child(div().id("box").w(10.0).h(10.0))
            .into();
        let parsed = parsed_from_root(root, 64, 64, 30, 0.1);
        let catalog = crate::probe::PreparedResourceCatalog::default();
        let mut pipeline = DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            NoopJsContext::new().expect("js"),
            test_font_db(),
        )
        .expect("open");

        let rects = pipeline.inspect_frame(0).expect("inspect");
        let root_rect = rects.iter().find(|r| r.id == "root").expect("root");
        assert_eq!(
            root_rect.script_source.as_deref(),
            Some("/* noop */"),
            "script source must come from pipeline source tree"
        );
        let _ = pipeline.render_frame(0).expect("render with script node");
    }

    /// Timeline fixture: inspect and render share the same prepared pipeline
    /// (no private catalog seed).
    #[test]
    fn inspect_frame_timeline_uses_pipeline_catalog() {
        use crate::parse::easing::Easing;
        use crate::parse::primitives::div;
        use crate::parse::transition::{fade, timeline};
        use crate::style::ColorToken;

        let root: crate::Node = div()
            .id("root")
            .w_full()
            .h_full()
            .child(
                timeline()
                    .sequence(
                        0.1,
                        div()
                            .id("scene-a")
                            .w_full()
                            .h_full()
                            .bg(ColorToken::Black)
                            .into(),
                    )
                    .transition(fade().timing(Easing::Linear, 0.05))
                    .sequence(
                        0.1,
                        div()
                            .id("scene-b")
                            .w_full()
                            .h_full()
                            .bg(ColorToken::White)
                            .into(),
                    ),
            )
            .into();
        let parsed = parsed_from_root(root, 80, 80, 30, 0.3);
        let catalog = crate::probe::PreparedResourceCatalog::default();
        let mut pipeline = DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            NoopJsContext::new().expect("js"),
            test_font_db(),
        )
        .expect("open");

        let rects = pipeline.inspect_frame(0).expect("inspect frame 0");
        assert!(
            rects.iter().any(|r| r.id == "scene-a"),
            "timeline inspect should surface active scene: {:?}",
            rects.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
        let frame = pipeline.render_frame(0).expect("render timeline");
        assert!(!frame.draw.ops.is_empty());
    }
}
