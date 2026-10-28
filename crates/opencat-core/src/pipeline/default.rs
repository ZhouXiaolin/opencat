use std::sync::Arc;

use anyhow::Result;

use crate::analyze::annotation::AnnotatedNodeHandle;
use crate::analyze::compositor::{OrderedSceneOp, OrderedSceneProgram};
use crate::display::build::DisplayBuildSession;
use crate::ir::{CompositionInfo, GeneratedImageTable, RenderFrame};
use crate::layout::LayoutSession;
use crate::parse::composition::Composition;
use crate::probe::catalog::PreparedResourceCatalog;
use crate::script::js_context::JsContext;

use super::Pipeline;

/// Frames a generated image survives after no frame re-registers it. Stable
/// keys (the emoji-glyph case) refresh every frame, so this only bounds the
/// per-frame-key case (a surface the script rewrites every frame, so auto-bake
/// mints a new key each frame): those entries are dead once the frame that
/// asked for them is gone. Two frames of slack covers a host that resolves the
/// previous frame's media plan one frame late.
const DEFAULT_GENERATED_IMAGE_MAX_AGE: usize = 2;

/// The core rendering pipeline.
///
/// Pure derivation kernel: host-supplied resource metadata via
/// [`HostInputs`](crate::lifecycle::HostInputs), font db,
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
    font_db: Arc<fontdb::Database>,
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
            font_db,
            last_ordered_scene: OrderedSceneProgram {
                root: OrderedSceneOp::LiveSubtree {
                    handle: AnnotatedNodeHandle(0),
                    children: Vec::new(),
                },
            },
            generated_images: GeneratedImageTable::with_max_age(
                DEFAULT_GENERATED_IMAGE_MAX_AGE,
            ),
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

    /// Inject the document base directory (XML's parent): join base for
    /// script-side relative asset paths (`GLTFGeometry` et al).
    pub fn set_document_base(&mut self, base: &str) {
        self.scripts.set_document_base(base);
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
        let (draw, media, gpu) = super::frame::render_frame_with_state(
            &self.composition,
            frame_index,
            &mut self.layout_session,
            &mut self.display_build_session,
            &self.font_db,
            &mut self.catalog,
            &mut self.last_ordered_scene,
            &mut self.scripts,
            &mut self.generated_images,
        )?;
        // GPU 流段由 pipeline 组装时从脚本侧取走（见 frame.rs）。
        Ok(RenderFrame { draw, media, gpu })
    }
}


