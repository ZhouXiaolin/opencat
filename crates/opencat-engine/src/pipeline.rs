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
use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError};
use opencat_core::parse::ParsedComposition;
use opencat_core::parse::{BuildOptions, CanvasChildrenMode, build_parsed_document, parse_parts_with_base_dir};
use opencat_core::pipeline::DefaultPipeline;
use opencat_core::probe::prepare::build_catalog;

use crate::EnginePipeline;
use crate::fonts::{engine_default_font_db, engine_font_db_with_document_fonts};
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

    /// Core-rasterized images (color-emoji bitmap glyphs) owned by the pipeline.
    /// The frame consumer reads this to resolve `ImageRef::Generated` refs into
    /// Skia images — generated RGBA never round-trips through the engine loader.
    pub fn generated_images(&self) -> &opencat_core::ir::GeneratedImageTable {
        self.pipeline.generated_images()
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
/// collect declarative [`ResourceRequests`] → fetch/cache bytes (`load_all`) →
/// run core's pure `build_catalog` over the cached bytes → hydrate captions from
/// cached SRT → build the font database. Only then does core open via
/// [`DefaultPipeline::open_with_prepared_catalog`], receiving a prepared catalog
/// and carrying no loader.
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
    let bytes = loader.load_font_manifest(&parts.font_manifest)?;
    loader.register_font_handles(&parts.font_manifest, &bytes)?;

    let mut font_db = engine_default_font_db();
    let font_index = if parts.font_manifest.is_empty() {
        None
    } else {
        let (db, index) = engine_font_db_with_document_fonts(&parts.font_manifest, &bytes)?;
        font_db = Arc::new(db);
        Some(index)
    };

    let parsed = build_parsed_document(
        parts,
        BuildOptions {
            canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
        },
        font_index.as_ref(),
    )?;

    open_parsed_host_owned(parsed, loader, scripts, font_db)
}

/// Open a [`ParsedComposition`] through the explicit lifecycle:
/// draft → host fetch → metadata HostInputs → prepare → open_pipeline.
///
/// Hosts never re-derive AssetId: every metadata insert uses the id from
/// [`CompositionDraft::requirements`]. Ordinary image bytes stay on the host;
/// prepare only consumes [`ImageMeta`] (and peers).
pub(crate) fn open_parsed_host_owned(
    parsed: ParsedComposition,
    mut loader: EngineLoader,
    scripts: RqJsContext,
    font_db: Arc<fontdb::Database>,
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
}
