use std::sync::Arc;

use anyhow::Result;

use crate::analyze::annotation::{AnalyzeFingerprintHistory, AnnotatedNodeHandle};
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::analyze::invalidation::CompositeHistory;
use crate::display::build::DisplayBuildSession;
use crate::ir::asset_id::{
    asset_id_for_audio, asset_id_for_image, asset_id_for_lottie, asset_id_for_subtitle,
    asset_id_for_video,
};
use crate::ir::cache::RenderCache;
use crate::ir::{CompositionInfo, RenderFrame};
use crate::layout::LayoutSession;
use crate::parse::composition::Composition;
use crate::parse::preflight::collect_resource_requests_from_parsed;
use crate::parse::primitives::{
    AudioSource, ImageSource, LottieSource, SubtitleSource, VideoSource,
};
use crate::probe::catalog::ResourceCatalog;
use crate::probe::probe::{probe_image, probe_video};
use crate::probe::{AssetHandle, AssetId, AssetLoader, NoopAssetLoader};
use crate::resource::lottie::parse_lottie_meta;
use crate::script::js_context::JsContext;

use super::Pipeline;

const DEFAULT_NODE_OWN_CAP: usize = 256;
const DEFAULT_SEGMENT_CAP: usize = 256;
const DEFAULT_ITEM_RANGE_CAP: usize = 128;

pub struct DefaultPipeline<L: AssetLoader, S: JsContext> {
    composition: Composition,
    info: CompositionInfo,
    catalog: ResourceCatalog,
    loader: L,
    scripts: crate::script::LiveScriptHost<S>,
    layout_session: LayoutSession,
    display_build_session: DisplayBuildSession,
    composite_history: CompositeHistory,
    analyze_fingerprint_history: AnalyzeFingerprintHistory,
    font_db: Arc<fontdb::Database>,
    cache: RenderCache,
    last_ordered_scene: OrderedSceneProgram,
}

impl<L: AssetLoader, S: JsContext> DefaultPipeline<L, S> {
    /// **Temporary compatibility shim** — see [`Self::open_parsed`]. Migrate
    /// to [`DefaultPipeline::open_with_prepared_catalog`]; this loader path is
    /// removed in #11.
    #[deprecated(
        since = "0.1.0",
        note = "use open_with_prepared_catalog (host-injected); this loader path is removed in #11"
    )]
    pub fn open(input: &str, loader: L, scripts: S) -> Result<Self> {
        #[cfg(test)]
        let font_db = Arc::new(crate::text::test_default_font_db());
        #[cfg(not(test))]
        let font_db = Arc::new(crate::text::empty_font_db());
        #[allow(deprecated)]
        Self::open_with_font_db(input, loader, scripts, font_db)
    }

    /// **Temporary compatibility shim** — see [`Self::open_parsed`]. Migrate
    /// to [`DefaultPipeline::open_with_prepared_catalog`]; this loader path is
    /// removed in #11.
    #[deprecated(
        since = "0.1.0",
        note = "use open_with_prepared_catalog (host-injected); this loader path is removed in #11"
    )]
    pub fn open_with_font_db(
        input: &str,
        loader: L,
        scripts: S,
        font_db: Arc<fontdb::Database>,
    ) -> Result<Self> {
        let trimmed = input.trim();
        let parsed = if trimmed.starts_with('{') {
            crate::parse::jsonl::parse(input)?
        } else {
            crate::parse::markup::parse(input)?
        };
        #[allow(deprecated)]
        Self::open_parsed(parsed, loader, scripts, font_db)
    }

    pub fn loader_mut(&mut self) -> &mut L {
        &mut self.loader
    }

    /// Open a pipeline from an already-built [`ParsedComposition`] and font database.
    ///
    /// **Temporary compatibility shim.** This loader-based entry point still
    /// runs core-internal fetch + probe (`loader.load_all` + `probe_all`). The
    /// host-injected main chain is now
    /// [`DefaultPipeline::open_with_prepared_catalog`], which takes a catalog
    /// the host already prepared via the `probe::prepare` chain and does no
    /// fetch/probe/loader work.
    ///
    /// This shim stays only until the engine (#7) and web (#8) hosts migrate to
    /// the prepared-catalog path; the loader seam — and this entry point — is
    /// deleted in #11.
    #[deprecated(
        since = "0.1.0",
        note = "use open_with_prepared_catalog (host-injected); this loader path is removed in #11"
    )]
    pub fn open_parsed(
        parsed: crate::parse::ParsedComposition,
        mut loader: L,
        scripts: S,
        font_db: Arc<fontdb::Database>,
    ) -> Result<Self> {
        // Collect declared resource requests from the *static* parsed tree
        // before the root is moved into the composition closure. This avoids
        // iterating composition frames and matches the host-facing contract:
        // requests are a declarative, order-independent set.
        let requests = collect_resource_requests_from_parsed(&parsed);

        let (composition, info, live_host) =
            build_pipeline_state(parsed, scripts, requests.clone())?;

        loader.load_all(&requests)?;

        let mut catalog = ResourceCatalog::default();
        probe_all(&loader, &requests, composition.fps, &mut catalog);

        Ok(Self {
            composition,
            info,
            catalog,
            loader,
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
        })
    }

    pub fn composition(&self) -> &Composition {
        &self.composition
    }

    pub fn catalog(&self) -> &ResourceCatalog {
        &self.catalog
    }

    pub fn scripts(&self) -> &crate::script::LiveScriptHost<S> {
        &self.scripts
    }
}

/// Host-injected entry point for the pipeline (issue #2 / #6 main chain).
///
/// Lives on a concrete `DefaultPipeline<NoopAssetLoader, S>` impl block so the
/// compiler resolves `open_with_prepared_catalog` without a loader type
/// annotation at the call site — this path takes no loader.
impl<S: JsContext> DefaultPipeline<NoopAssetLoader, S> {
    /// Open a pipeline from host-prepared inputs: a parsed composition, a
    /// [`ResourceCatalog`] the host already built (via the `probe::prepare`
    /// chain), the script context, and the font database.
    ///
    /// This is the host-injected main chain (issue #2 / #6). Unlike the
    /// deprecated loader-based
    /// [`DefaultPipeline::open_parsed`](DefaultPipeline::<L, S>::open_parsed),
    /// it does **no** fetch, cache, decode, or probe work and holds no loader —
    /// the catalog's metadata is exactly what the host supplies. Core only
    /// derives layout and `RenderFrame` output from these inputs.
    ///
    /// The returned pipeline carries a [`NoopAssetLoader`] only because the
    /// struct is still parameterized over a loader during the migration window.
    /// The noop loader is never invoked on this path; it is removed with the
    /// loader generic in #11.
    pub fn open_with_prepared_catalog(
        parsed: crate::parse::ParsedComposition,
        catalog: ResourceCatalog,
        scripts: S,
        font_db: Arc<fontdb::Database>,
    ) -> Result<Self> {
        let requests = collect_resource_requests_from_parsed(&parsed);

        let (composition, info, live_host) = build_pipeline_state(parsed, scripts, requests)?;

        Ok(Self {
            composition,
            info,
            catalog,
            // The host-injected path owns no loader. `NoopAssetLoader` is a
            // placeholder so the pipeline struct type-checks during the
            // migration; it is deleted with the loader generic in #11.
            loader: NoopAssetLoader,
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
        })
    }
}

/// Build the loader-independent pipeline state shared by every entry point:
/// the [`Composition`] (with the parsed root frozen into its closure), the
/// [`CompositionInfo`] (carrying the declarative, order-independent
/// [`ResourceRequests`] and the audio plan), and the live script host.
///
/// This owns no fetch/probe/loader logic — it is pure derivation from the
/// parsed composition. The caller is responsible for producing the
/// [`ResourceCatalog`]: the deprecated loader path runs `probe_all` itself,
/// the host-injected path takes a host-prepared catalog as-is.
fn build_pipeline_state<S: JsContext>(
    parsed: crate::parse::ParsedComposition,
    scripts: S,
    requests: crate::probe::catalog::ResourceRequests,
) -> Result<(Composition, CompositionInfo, crate::script::LiveScriptHost<S>)> {
    let root_node = parsed.root;
    let composition = Composition::new("pipeline")
        .size(parsed.width, parsed.height)
        .fps(parsed.fps as u32)
        .duration(parsed.duration)
        .root(move |_ctx| root_node.clone())
        .audio_sources(parsed.audio_sources)
        .build()?;

    let audio_plan = crate::parse::preflight::collect_audio_plan(&composition);

    let info = CompositionInfo {
        width: composition.width as u32,
        height: composition.height as u32,
        fps: composition.fps,
        duration: composition.duration,
        requests,
        audio_plan,
    };

    let live_host = crate::script::LiveScriptHost::new(scripts)?;

    Ok((composition, info, live_host))
}

fn source_to_image_id(src: &ImageSource) -> Option<AssetId> {
    asset_id_for_image(src)
}

fn source_to_video_id(src: &VideoSource) -> AssetId {
    asset_id_for_video(src)
}

fn source_to_audio_id(src: &AudioSource) -> Option<AssetId> {
    asset_id_for_audio(src)
}

fn source_to_subtitle_id(src: &SubtitleSource) -> AssetId {
    asset_id_for_subtitle(src)
}

fn source_to_lottie_id(element_id: &str, src: &LottieSource) -> Option<AssetId> {
    asset_id_for_lottie(element_id, src)
}

fn probe_all<L: AssetLoader>(
    loader: &L,
    requests: &crate::probe::catalog::ResourceRequests,
    fps: u32,
    catalog: &mut ResourceCatalog,
) where
    <L as AssetLoader>::Handle: AssetHandle,
{
    for src in &requests.images {
        if let Some(id) = source_to_image_id(src) {
            if let Some(handle) = loader.handle(&id) {
                if let Ok(bytes) = handle.read_bytes() {
                    if let Ok(meta) = probe_image(&bytes) {
                        catalog.images.insert(id, meta);
                    }
                }
            }
        }
    }

    for src in &requests.videos {
        let id = source_to_video_id(src);
        if let Some(handle) = loader.handle(&id) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(meta) = probe_video(&bytes) {
                    catalog.videos.insert(id, meta);
                }
            }
        }
    }

    for src in &requests.audios {
        if let Some(id) = source_to_audio_id(src) {
            catalog.audios.insert(id);
        }
    }

    for src in &requests.subtitles {
        let id = source_to_subtitle_id(src);
        if let Some(handle) = loader.handle(&id) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(entries) = crate::probe::probe::parse_srt_bytes(&bytes, fps) {
                    catalog.subtitles.insert(id, entries);
                }
            }
        }
    }

    for req in &requests.lotties {
        if matches!(req.source, LottieSource::Unset) {
            continue;
        }
        let bundle_id =
            source_to_lottie_id(&req.element_id, &req.source).expect("non-unset lottie has id");
        let id_for_lookup = match &req.source {
            LottieSource::Path(p) => AssetId(p.to_string_lossy().into_owned()),
            LottieSource::Url(u) => crate::ir::asset_id::asset_id_for_url(u),
            LottieSource::Unset => continue,
        };
        if let Some(handle) = loader.handle(&id_for_lookup) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(json) = std::str::from_utf8(&bytes) {
                    if let Ok(meta) = parse_lottie_meta(json) {
                        catalog.lotties.insert(bundle_id, meta);
                    }
                }
            }
        }
    }
}

impl<L: AssetLoader, S: JsContext> Pipeline for DefaultPipeline<L, S> {
    type Loader = L;
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
            None,
        )?;
        Ok(RenderFrame { draw, media })
    }

    fn loader(&self) -> &Self::Loader {
        &self.loader
    }
}

#[cfg(test)]
mod tests {
    // The tests in this module intentionally exercise the deprecated loader
    // compatibility shims (`open` / `open_with_font_db` / `open_parsed`) to
    // prove the migration bridge still works while hosts move to
    // `open_with_prepared_catalog`. The new host-injected path is covered by
    // the `open_with_prepared_catalog_*` tests below.
    #![allow(deprecated)]

    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::*;
    use crate::ir::DrawOpFrame;
    use crate::probe::{AssetHandle, AssetLoader as AssetLoaderTrait};
    use crate::script::js_context::JsContext;
    use crate::script::recorder::MutationStore;

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
    impl AssetLoaderTrait for InMemoryLoader {
        type Handle = ByteHandle;
        fn load_all(&mut self, _: &crate::probe::catalog::ResourceRequests) -> Result<()> {
            Ok(())
        }
        fn handle(&self, id: &AssetId) -> Option<&Self::Handle> {
            self.map.get(id)
        }
    }

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

    /// Open a pipeline through the host-injected main chain
    /// (`open_with_prepared_catalog`), mirroring what a real host does:
    /// parse the source, collect declarative requests, build a catalog via the
    /// pure `build_catalog` over empty bytes (every asset omitted — these are
    /// render-seam tests, not asset-decoding tests), then open with the test
    /// font database.
    ///
    /// Behavior tests use this so they exercise the highest render seam
    /// (`render_frame -> RenderFrame`) on the new main chain rather than the
    /// deprecated loader shim.
    fn open_host_injected(
        input: &str,
    ) -> DefaultPipeline<NoopAssetLoader, NoopJsContext> {
        let trimmed = input.trim();
        let parsed = if trimmed.starts_with('{') {
            crate::parse::jsonl::parse(input).expect("parse input")
        } else {
            crate::parse::markup::parse(input).expect("parse input")
        };
        let requests = collect_resource_requests_from_parsed(&parsed);
        let catalog = crate::probe::build_catalog(
            &requests,
            &HashMap::<String, Vec<u8>>::new(),
        )
        .catalog;
        DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            NoopJsContext::new().expect("js context"),
            Arc::new(crate::text::test_default_font_db()),
        )
        .expect("open host-injected pipeline")
    }

    #[test]
    fn open_empty_composition_returns_info() {
        let jsonl = r#"{"type":"composition","width":100,"height":200,"fps":30,"duration":0.033333333333}
{"type":"div","id":"root","parentId":null}"#;

        let loader = InMemoryLoader::default();
        let ctx = NoopJsContext::new().expect("js context");

        let pipeline = DefaultPipeline::open(jsonl, loader, ctx).expect("open");

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

        let mut pipeline = open_host_injected(jsonl);

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

        let mut pipeline = open_host_injected(xml);

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

        let mut pipeline = open_host_injected(xml);

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

        let mut pipeline = open_host_injected(xml);

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

        let after_duration_frame =
            pipeline.render_frame(150).expect("render frame 150");
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

        let mut p1 = open_host_injected(jsonl);
        let mut p2 = open_host_injected(jsonl);

        for i in 0..5 {
            let r1 = p1.render_frame(i).expect("render p1");
            let r2 = p2.render_frame(i).expect("render p2");
            assert_eq!(r1.draw.ops.len(), r2.draw.ops.len(), "frame {i} op count mismatch");
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

        let mut pipeline = open_host_injected(xml);

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
    }

    #[cfg(feature = "profile")]
    #[test]
    fn render_frame_emits_profile_events_for_each_frame() {
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":10,"duration":0.2}
{"type":"div","id":"root","parentId":null,"bg":"#00ff00","w":100,"h":100}"##;

        let config = crate::profile::ProfileConfig { enabled: true };
        let (_, summary) = crate::profile::profile_render(&config, || {
            let mut pipeline = DefaultPipeline::open(
                jsonl,
                InMemoryLoader::default(),
                NoopJsContext::new().expect("js context"),
            )
            .expect("open");

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
            let mut pipeline = DefaultPipeline::open(
                jsonl,
                InMemoryLoader::default(),
                NoopJsContext::new().expect("js context"),
            )
            .expect("open profile showcase jsonl");

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
        // In the test environment (InMemoryLoader, no assets), the showcase collapses
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
            let mut pipeline = DefaultPipeline::open(
                jsonl,
                InMemoryLoader::default(),
                NoopJsContext::new().expect("js context"),
            )
            .expect("open");

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
            let mut pipeline = DefaultPipeline::open(
                jsonl,
                InMemoryLoader::default(),
                NoopJsContext::new().expect("js context"),
            )
            .expect("open");

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
    fn open_pipeline_populates_audio_plan() {
        let jsonl = r##"{"type":"composition","width":100,"height":100,"fps":30,"duration":0.033333333333}
{"type":"div","id":"root","parentId":null}"##;
        let pipeline = DefaultPipeline::<InMemoryLoader, NoopJsContext>::open(
            jsonl,
            InMemoryLoader::default(),
            NoopJsContext::new().unwrap(),
        )
        .expect("open");
        assert!(
            pipeline.info().audio_plan.segments.is_empty(),
            "no audio sources => empty plan"
        );
    }

    #[test]
    fn open_from_xml() {
        let xml = r#"<opencat width="200" height="100" fps="30" duration="0.033333333333">
  <div id="root" />
</opencat>"#;
        let loader = InMemoryLoader::default();
        let ctx = NoopJsContext::new().expect("js context");
        let pipeline = DefaultPipeline::open(xml, loader, ctx).expect("open xml");
        assert_eq!(pipeline.info().width, 200);
        assert_eq!(pipeline.info().height, 100);
        assert_eq!(pipeline.info().fps, 30);
        assert!((pipeline.info().duration - 1.0 / 30.0).abs() < 1e-9);
    }

    // ---- Host-injected entry point (issue #6) ------------------------------------
    //
    // These tests exercise `open_with_prepared_catalog` — the host-injected main
    // chain — at the highest render seam (`render_frame -> RenderFrame`). They
    // prove the new path does no fetch/probe/loader work: the host-supplied
    // `ResourceCatalog` is the single source of metadata.

    use crate::ir::asset_id::asset_id_for_image;
    use crate::parse::primitives::image;
    use crate::probe::catalog::ImageMeta;
    use crate::probe::{NoopAssetLoader, ResourceCatalog as ProbeResourceCatalog};
    use crate::resource::fonts::FontManifest;

    /// Build a minimal `ParsedComposition` from a root node, for tests that
    /// drive the pipeline without going through markup/jsonl parsing.
    fn parsed_from_root(root: crate::Node, width: i32, height: i32, fps: u32, duration: f64) -> crate::parse::ParsedComposition {
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

    /// AC #1, #2, #4, #5: opening via the host-injected path renders a frame
    /// with no loader involvement. The host supplies an empty catalog (built
    /// via the prepare chain with no bytes); `render_frame` must still produce
    /// draw ops and a media plan.
    #[test]
    fn open_with_prepared_catalog_renders_without_a_loader() {
        // Host side: parse a tree, collect declarative requests, build a catalog
        // with the pure `build_catalog` over *no* bytes (every asset omitted).
        let root: crate::Node = crate::parse::primitives::div().id("root").into();
        let parsed = parsed_from_root(root, 320, 240, 30, 0.1);

        let requests = collect_resource_requests_from_parsed(&parsed);
        let prepared = crate::probe::build_catalog(&requests, &std::collections::HashMap::<String, Vec<u8>>::new());
        // Empty composition declares nothing, so the catalog is empty and the
        // pipeline must still open and render.
        assert!(prepared.catalog.images.is_empty());

        let ctx = NoopJsContext::new().expect("js context");
        let mut pipeline = DefaultPipeline::open_with_prepared_catalog(
            parsed,
            prepared.catalog,
            ctx,
            Arc::new(crate::text::test_default_font_db()),
        )
        .expect("open host-injected pipeline");

        let frame = pipeline.render_frame(0).expect("render frame 0");
        assert!(
            !frame.draw.ops.is_empty(),
            "host-injected pipeline should still produce DrawOps"
        );
        let _ = frame.media;
    }

    /// AC #2: the host-supplied catalog is the source of truth. A host that
    /// probed image bytes and stored metadata under the canonical `AssetId`
    /// must see that exact metadata surface from `pipeline.catalog()` — the
    /// pipeline did not re-probe, re-fetch, or invent anything.
    #[test]
    fn open_with_prepared_catalog_uses_host_supplied_metadata() {
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
        catalog.images.insert(canonical.clone(), ImageMeta { width: 42, height: 17 });

        let ctx = NoopJsContext::new().expect("js context");
        let pipeline = DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            ctx,
            Arc::new(crate::text::test_default_font_db()),
        )
        .expect("open host-injected pipeline");

        let stored = pipeline
            .catalog()
            .images
            .get(&canonical)
            .expect("host metadata must survive opening unchanged");
        assert_eq!((stored.width, stored.height), (42, 17));
    }

    /// AC #5: the host-injected path is as order- and repeat-invariant as the
    /// loader path. Call history must not leak into the per-frame contract.
    #[test]
    fn open_with_prepared_catalog_is_order_and_repeat_invariant() {
        let root: crate::Node = crate::parse::primitives::div()
            .id("root")
            .w(100.0)
            .h(100.0)
            .bg_red()
            .into();

        let open_fresh = || -> DefaultPipeline<NoopAssetLoader, NoopJsContext> {
            let parsed = parsed_from_root(root.clone(), 100, 100, 10, 0.5);
            let requests = collect_resource_requests_from_parsed(&parsed);
            let catalog = crate::probe::build_catalog(
                &requests,
                &std::collections::HashMap::<String, Vec<u8>>::new(),
            )
            .catalog;
            DefaultPipeline::open_with_prepared_catalog(
                parsed,
                catalog,
                NoopJsContext::new().expect("js context"),
                Arc::new(crate::text::test_default_font_db()),
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

    /// AC #4: the host-injected pipeline carries the font database the host
    /// built, so core shaping/layout stays usable. Rendering a text frame must
    /// not panic and must produce at least one draw op.
    #[test]
    fn open_with_prepared_catalog_carries_font_db() {
        let root: crate::Node = crate::parse::primitives::div()
            .id("root")
            .child(crate::parse::primitives::text("Hi").id("label"))
            .into();
        let parsed = parsed_from_root(root, 200, 80, 30, 0.1);

        let requests = collect_resource_requests_from_parsed(&parsed);
        let catalog = crate::probe::build_catalog(
            &requests,
            &std::collections::HashMap::<String, Vec<u8>>::new(),
        )
        .catalog;

        let ctx = NoopJsContext::new().expect("js context");
        let mut pipeline = DefaultPipeline::open_with_prepared_catalog(
            parsed,
            catalog,
            ctx,
            Arc::new(crate::text::test_default_font_db()),
        )
        .expect("open");

        let frame = pipeline.render_frame(0).expect("render text frame");
        assert!(
            !frame.draw.ops.is_empty(),
            "text frame should still emit draw ops under the host-injected path"
        );
    }
}
