//! Open compositions on a host-owned resource pipeline.
//!
//! The engine is the host: it owns fetch/cache and probes media metadata, then
//! feeds metadata into core's explicit lifecycle
//! (`CompositionDraft` → `HostInputs` → `prepare` → `open_pipeline`). Core never
//! sees ordinary media bytes and never re-derives AssetIds.
//!
//! [`EnginePipelineHost`] bundles the resulting core pipeline together with the
//! engine resource owner ([`EngineLoader`]) so render/audio code can reach the
//! cached bytes for the current [`FrameMediaPlan`] without going through core.

use std::sync::Arc;

use anyhow::Result;

use opencat_core::ir::RenderFrame;
use opencat_core::ir::asset_id::AssetId;
use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError, ResourceKind};
use opencat_core::parse::ParsedComposition;
use opencat_core::parse::{BuildOptions, CanvasChildrenMode, build_parsed_document, parse_parts_with_base_dir};
use opencat_core::pipeline::DefaultPipeline;
use opencat_core::probe::prepare::build_catalog;
use opencat_core::resource::fonts::font_asset_id;

use crate::EnginePipeline;
use crate::fonts::engine_default_font_db;
use crate::js_context::RqJsContext;
use crate::resource::loader::EngineLoader;

/// Core pipeline opened by the engine on the host-injected (loader-free) path
/// via [`DefaultPipeline::open_with_prepared_catalog`].
type CorePipeline = DefaultPipeline<RqJsContext>;

/// Engine host: owns the core pipeline **and** the engine resource owner.
///
/// Per issue #2 / #7, the core pipeline no longer owns an engine loader. The
/// engine fetches/caches bytes and prepares metadata itself, then opens core via
/// [`DefaultPipeline::open_with_prepared_catalog`]. The [`EngineLoader`] lives
/// here so the frame consumer and audio mixer can read the cached bytes for the
/// current frame's media plan directly — they never reach through the core
/// pipeline.
pub struct EnginePipelineHost {
    /// Core pipeline (pure derivation; no loader access).
    pub pipeline: CorePipeline,
    /// Engine-owned resource owner: cached asset handles, fetcher, providers.
    pub loader: EngineLoader,
}

impl EnginePipelineHost {
    /// Borrow the core pipeline for frame rendering.
    pub fn pipeline(&mut self) -> &mut CorePipeline {
        &mut self.pipeline
    }

    /// Borrow the engine resource owner (cached bytes / handles / providers).
    pub fn loader(&self) -> &EngineLoader {
        &self.loader
    }

    /// Mutable borrow of the engine resource owner.
    pub fn loader_mut(&mut self) -> &mut EngineLoader {
        &mut self.loader
    }

    /// Delegate: composition info (width/height/fps/duration/requests/audio plan).
    pub fn info(&self) -> &opencat_core::ir::CompositionInfo {
        use opencat_core::pipeline::Pipeline;
        self.pipeline.info()
    }

    /// Delegate: composition (parsed tree, fps, frames).
    pub fn composition(&self) -> &opencat_core::parse::composition::Composition {
        self.pipeline.composition()
    }

    /// Delegate: render one frame to a deterministic [`RenderFrame`].
    pub fn render_frame(&mut self, idx: u32) -> Result<RenderFrame> {
        use opencat_core::pipeline::Pipeline;
        self.pipeline.render_frame(idx)
    }
}

/// Parse a composition and open it on the host-owned resource pipeline.
///
/// The engine completes the full host preparation chain before opening core:
/// collect declarative [`ResourceRequests`] → fetch/cache media + subtitle +
/// document font bytes → run core's pure `build_catalog` over media bytes →
/// hand base font db + document font bytes + SRT text to core prepare. Core
/// alone merges fonts, hydrates captions, and opens the pipeline.
pub fn open(input: &str, mut loader: EngineLoader, scripts: RqJsContext) -> Result<EnginePipeline> {
    if input.trim().starts_with('{') {
        let base_dir = loader
            .base_dir()
            .canonicalize()
            .unwrap_or_else(|_| loader.base_dir().to_path_buf());
        let parsed = crate::source_io::parse_with_base_dir(input, Some(&base_dir))?;
        let host = open_parsed_host_owned(parsed, loader, scripts, engine_default_font_db())?;
        return Ok(host);
    }

    let base_dir = loader.base_dir();
    let parts = parse_parts_with_base_dir(input, Some(base_dir))?;
    // Host only fetches document font bytes + registers cache handles. Family
    // index / fallback / fontdb merge happen in core prepare (#19).
    let font_bytes = loader.load_font_manifest(&parts.font_manifest)?;
    loader.register_font_handles(&parts.font_manifest, &font_bytes)?;

    let parsed = build_parsed_document(
        parts,
        BuildOptions {
            canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
        },
        None,
    )?;

    open_parsed_host_owned_with_fonts(
        parsed,
        loader,
        scripts,
        engine_default_font_db(),
        font_bytes,
    )
}

/// Open a [`ParsedComposition`] through the explicit lifecycle:
/// draft → host fetch → metadata HostInputs → prepare → open_pipeline.
///
/// Hosts never re-derive AssetId: every metadata insert uses the id from
/// [`CompositionDraft::requirements`]. Ordinary image bytes stay on the host;
/// prepare only consumes [`ImageMeta`] (and peers). Document font bytes are
/// content-level inputs keyed by the stable font AssetId.
pub(crate) fn open_parsed_host_owned(
    parsed: ParsedComposition,
    loader: EngineLoader,
    scripts: RqJsContext,
    font_db: Arc<fontdb::Database>,
) -> Result<EnginePipelineHost> {
    open_parsed_host_owned_with_fonts(parsed, loader, scripts, font_db, Default::default())
}

fn open_parsed_host_owned_with_fonts(
    parsed: ParsedComposition,
    mut loader: EngineLoader,
    scripts: RqJsContext,
    font_db: Arc<fontdb::Database>,
    font_bytes_by_face_id: std::collections::HashMap<String, Vec<u8>>,
) -> Result<EnginePipelineHost> {
    let draft = CompositionDraft::from_parsed(parsed);
    let requests = draft.requirements().resource_requests().clone();

    // Host fetch/cache under canonical AssetIds from core.
    loader.load_all(&requests)?;

    // Probe bytes → metadata (pure). Host keeps the bytes; core sees only meta.
    let bytes = loader.collect_probe_bytes_by_asset_id(&requests);
    let probed = build_catalog(&requests, &bytes).catalog;
    let srt = loader.srt_text_by_subtitle_id(&requests);

    let mut inputs = HostInputs::empty().with_font_db(font_db);
    inputs
        .fill_from_prepared_catalog(draft.requirements(), &probed, &srt)
        .map_err(prepare_err)?;

    // External scripts: host reads file text against loader base_dir and
    // injects via HostInputs — core never rewrites the input string (#20).
    crate::source_io::fill_script_texts_from_disk(
        &mut inputs,
        draft.requirements(),
        Some(loader.base_dir()),
    )?;

    // Document fonts: map face-id bytes (from load_font_manifest) onto the
    // stable font AssetId from requirements. Core merges; host does not.
    for req in draft.requirements().requests() {
        if req.kind != ResourceKind::Font {
            continue;
        }
        let face_id = draft
            .parsed()
            .font_manifest
            .faces
            .iter()
            .find(|f| font_asset_id(&f.source) == req.asset_id.0)
            .map(|f| f.id.as_str());
        let Some(face_id) = face_id else {
            continue;
        };
        let Some(font_bytes) = font_bytes_by_face_id.get(face_id) else {
            // Missing bytes will fail prepare with MissingInput for this asset.
            continue;
        };
        inputs
            .insert_document_font(AssetId(req.asset_id.0.clone()), font_bytes.clone())
            .map_err(prepare_err)?;
    }

    let prepared = draft.prepare(inputs).map_err(prepare_err)?;
    let pipeline = prepared.open_pipeline(scripts)?;

    let composition = pipeline.composition().clone();
    loader.register_canvas_asset_aliases(&composition);

    Ok(EnginePipelineHost { pipeline, loader })
}

fn prepare_err(err: PrepareError) -> anyhow::Error {
    anyhow::anyhow!(err)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use opencat_core::{
        frame_ctx::FrameCtx,
        parse::{
            node::{Node, NodeKind},
            primitives::CaptionNode,
        },
        script::js_context::JsContext,
    };

    use super::open;

    fn find_caption<'a>(node: &'a Node, id: &str) -> Option<&'a CaptionNode> {
        match node.kind() {
            NodeKind::Caption(caption) if caption.style_ref().id == id => Some(caption),
            NodeKind::Div(div) => div
                .children_ref()
                .iter()
                .find_map(|child| find_caption(child, id)),
            _ => None,
        }
    }

    #[test]
    fn jsonl_caption_path_resolves_relative_to_loader_base_dir() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let fixture_dir = std::path::PathBuf::from(format!("target/opencat-jsonl-caption-{nanos}"));
        let cache_dir = fixture_dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir");
        std::fs::write(
            fixture_dir.join("sub.srt"),
            "1\n00:00:00,000 --> 00:00:00,500\nHello CLI\n",
        )
        .expect("srt fixture");

        let jsonl = r#"{"type":"composition","width":320,"height":180,"fps":30,"duration":1}
{"id":"root","parentId":null,"type":"div","className":"relative w-[320px] h-[180px]"}
{"id":"subs","parentId":"root","type":"caption","className":"absolute left-[0px] top-[0px] text-white","path":"sub.srt"}"#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::RqJsContext::new().expect("js context");
        let host = open(jsonl, loader, ctx).expect("pipeline");

        // Caption hydration is now part of the host-owned open chain: the SRT
        // file was fetched by the loader, parsed by core's pure hydrate_captions,
        // and the entries written into the caption node before core opened.
        let root = host.composition().root_node(&FrameCtx {
            frame: 0,
            fps: 30,
            width: 320,
            height: 180,
            frames: 30,
        });
        let caption = find_caption(&root, "subs").expect("caption node");

        assert_eq!(caption.entries_ref().len(), 1);
        assert_eq!(caption.active_text(0), Some("Hello CLI"));

        std::fs::remove_dir_all(&fixture_dir).ok();
    }

    #[test]
    fn static_image_lifecycle_uses_request_asset_id() {
        use opencat_core::ir::draw_types::ImageRef;
        use opencat_core::pipeline::Pipeline;
        use opencat_core::resource::probe::probe_image_dims;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let fixture_dir =
            std::path::PathBuf::from(format!("target/opencat-lifecycle-image-{nanos}"));
        let cache_dir = fixture_dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir");

        // Minimal 1×1 PNG
        const PNG_1X1: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x05, 0xFE,
            0xD4, 0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        std::fs::write(fixture_dir.join("hero.png"), PNG_1X1).expect("png");

        let jsonl = r#"{"type":"composition","width":64,"height":64,"fps":30,"duration":0.1}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"pic","parentId":"root","type":"image","path":"hero.png","className":"w-[32px] h-[32px]"}"#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::RqJsContext::new().expect("js context");
        let mut host = open(jsonl, loader, ctx).expect("open via lifecycle");

        // Host must key handles by the request AssetId (logical path), not a re-derived id.
        let handle = host
            .loader
            .handle(&opencat_core::AssetId("hero.png".into()));
        assert!(
            handle.is_some(),
            "engine loader must register request AssetId hero.png"
        );

        let frame = host.pipeline.render_frame(0).expect("render");
        let has = frame.media.images.iter().any(|img| match img {
            ImageRef::Static { asset_id } => asset_id == "hero.png",
            _ => false,
        });
        assert!(
            has,
            "FrameMediaPlan must use request AssetId; got {:?}",
            frame.media.images
        );

        // Sanity: probed dims match the fixture.
        let dims = probe_image_dims(PNG_1X1).expect("dims");
        assert_eq!((dims.width, dims.height), (1, 1));

        std::fs::remove_dir_all(&fixture_dir).ok();
    }

    #[test]
    fn lottie_lifecycle_uses_request_bundle_asset_id() {
        use opencat_core::ir::draw_op::DrawOp;
        use opencat_core::pipeline::Pipeline;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let fixture_dir =
            std::path::PathBuf::from(format!("target/opencat-lifecycle-lottie-{nanos}"));
        let cache_dir = fixture_dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir");

        // Minimal Bodymovin root with one external dependency name.
        let lottie_json = r#"{"w":40,"h":30,"fr":25,"ip":0,"op":10,"assets":[{"u":"images/dep.png","e":"images/"}]}"#;
        std::fs::write(fixture_dir.join("loader.json"), lottie_json).expect("lottie json");

        let markup = r#"
            <opencat width="64" height="64" fps="25" duration="0.4">
              <div id="root" class="w-full h-full">
                <lottie id="loader" path="loader.json" class="w-[40px] h-[30px]" />
              </div>
            </opencat>
        "#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::RqJsContext::new().expect("js context");
        let mut host = open(markup, loader, ctx).expect("open via lifecycle");

        // Host registers under logical path key (probe) and canonical bundle id.
        assert!(
            host.loader
                .handle(&opencat_core::AssetId("loader.json".into()))
                .is_some(),
            "engine must cache primary JSON under logical locator"
        );
        assert!(
            host.loader
                .handle(&opencat_core::AssetId("lottie:loader".into()))
                .is_some(),
            "engine must also key primary JSON under request bundle AssetId"
        );

        // Composition still declares the lottie request under raw requests.
        assert!(
            host.pipeline
                .info()
                .requests
                .lotties
                .iter()
                .any(|r| r.element_id == "loader"),
            "composition must declare lottie request"
        );

        let frame = host.pipeline.render_frame(0).expect("render");
        assert!(
            frame
                .media
                .lottie_bundles
                .iter()
                .any(|b| b == "lottie:loader"),
            "FrameMediaPlan must list bundle id; got {:?}",
            frame.media.lottie_bundles
        );
        assert!(
            frame.media.images.is_empty(),
            "Lottie must not be disguised as image; got {:?}",
            frame.media.images
        );
        let has_op = frame.draw.ops.iter().any(|op| {
            matches!(op, DrawOp::LottieRect { bundle_id, .. } if bundle_id == "lottie:loader")
        });
        assert!(has_op, "draw must emit LottieRect; ops={:?}", frame.draw.ops);

        std::fs::remove_dir_all(&fixture_dir).ok();
    }

    #[test]
    fn markup_document_font_is_merged_by_core_prepare() {
        use opencat_core::pipeline::Pipeline;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let fixture_dir =
            std::path::PathBuf::from(format!("target/opencat-lifecycle-font-{nanos}"));
        let cache_dir = fixture_dir.join("cache");
        std::fs::create_dir_all(&cache_dir).expect("cache dir");

        // Copy a real face into the fixture so the engine loader can read it
        // under the logical path relative to base_dir.
        let face_src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/NotoSansSC-Regular.otf");
        std::fs::copy(&face_src, fixture_dir.join("doc-sans.otf")).expect("copy font");

        let markup = r#"
            <opencat width="320" height="180" fps="30" duration="0.1">
              <fonts default="doc">
                <font id="doc" family="Noto Sans SC" path="doc-sans.otf" role="sans" />
              </fonts>
              <div id="root" class="w-full h-full">
                <text id="t" class="font-sans text-white text-[24px]" data-text="你好" />
              </div>
            </opencat>
        "#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::RqJsContext::new().expect("js context");
        let mut host = open(markup, loader, ctx).expect("open with document font via core prepare");

        // font-sans must shape with document face; render must succeed.
        let frame = host.pipeline.render_frame(0).expect("render");
        assert!(
            !frame.draw.ops.is_empty(),
            "document font path must produce draw ops"
        );

        std::fs::remove_dir_all(&fixture_dir).ok();
    }
}
