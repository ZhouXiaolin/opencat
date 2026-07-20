//! Open compositions on a host-owned resource pipeline.
//!
//! The engine is the host: it owns fetch/cache, runs the pure metadata probes,
//! hydrates subtitles, and builds the font database — then hands a prepared
//! [`ResourceCatalog`] to core's [`DefaultPipeline::open_with_prepared_catalog`].
//! Core derives only layout and `RenderFrame` output; it carries a
//! [`NoopAssetLoader`] and never touches the file system.
//!
//! [`EnginePipelineHost`] bundles the resulting core pipeline together with the
//! engine resource owner ([`EngineLoader`]) so render/audio code can reach the
//! cached bytes for the current [`FrameMediaPlan`] without going through core.

use std::sync::Arc;

use anyhow::Result;

use opencat_core::ir::RenderFrame;
use opencat_core::parse::ParsedComposition;
use opencat_core::parse::preflight::{collect_external_manifest, collect_resource_requests_from_parsed};
use opencat_core::parse::{BuildOptions, CanvasChildrenMode, build_parsed_document, parse_parts_with_base_dir};
use opencat_core::pipeline::DefaultPipeline;
use opencat_core::probe::AssetLoader;
use opencat_core::probe::NoopAssetLoader;
use opencat_core::probe::catalog::ResourceCatalog;
use opencat_core::probe::prepare::{build_catalog, hydrate_captions};

use crate::EnginePipeline;
use crate::fonts::{engine_default_font_db, engine_font_db_with_document_fonts};
use crate::js_context::RqJsContext;
use crate::resource::loader::EngineLoader;

/// Core pipeline monomorphised on the host-injected (loader-free) path.
///
/// The loader generic is still present on the struct during the migration window
/// (removed in #11); on this path it is always [`NoopAssetLoader`] and is never
/// invoked.
type CorePipeline = DefaultPipeline<NoopAssetLoader, RqJsContext>;

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
    let font_manifest = parts.font_manifest.clone();
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

    let mut host = open_parsed_host_owned(parsed, loader, scripts, font_db)?;

    let (_, external_manifest) = collect_external_manifest(host.composition(), &font_manifest);
    host.loader_mut()
        .build_resource_provider(&external_manifest);

    Ok(host)
}

/// Open a [`ParsedComposition`] through the host-owned chain: fetch/cache,
/// pure-catalog build, caption hydration, then core's
/// [`DefaultPipeline::open_with_prepared_catalog`].
///
/// This is the shared host preparation used by both the JSONL and markup open
/// paths. It must run before core consumes `parsed`, because core moves the
/// parsed root into the composition closure.
pub(crate) fn open_parsed_host_owned(
    mut parsed: ParsedComposition,
    mut loader: EngineLoader,
    scripts: RqJsContext,
    font_db: Arc<fontdb::Database>,
) -> Result<EnginePipelineHost> {
    // 1. Declarative, order-independent resource requests from the static tree.
    let requests = collect_resource_requests_from_parsed(&parsed);

    // 2. Host fetch/cache: every declared asset is copied into the cache dir and
    //    registered under its canonical AssetId handle. (Host-owned; core never
    //    fetches.)
    loader.load_all(&requests)?;

    // 3. Pure catalog build from cached bytes. The map keys are canonical
    //    AssetId strings; build_catalog runs core's pure image/video/Lottie
    //    probes over them. Missing/unparseable assets are omitted (probe-failure
    //    boundary), they are not host errors here.
    let bytes = loader.collect_probe_bytes_by_asset_id(&requests);
    let catalog: ResourceCatalog = build_catalog(&requests, &bytes).catalog;

    // 4. Hydrate captions from cached SRT text. Core's pure hydrate_captions
    //    parses the SRT and writes entries into caption nodes; existing entries
    //    are never overwritten and missing text stays empty. Done in place on
    //    parsed.root before core moves it into the composition closure.
    let srt = loader.srt_text_by_subtitle_id(&requests);
    parsed.root = hydrate_captions(parsed.root, parsed.fps as u32, &srt)?.0;

    // 5. Open core on the host-injected path: prepared catalog + font db, no
    //    loader. Core only derives layout and RenderFrame output.
    let pipeline = DefaultPipeline::open_with_prepared_catalog(parsed, catalog, scripts, font_db)?;

    // 6. Register canvas aliases (user-facing id -> cached content handle) on
    //    the engine loader so `ctx.getImage("hero")` resolves at render time.
    let composition = pipeline.composition().clone();
    loader.register_canvas_asset_aliases(&composition);

    Ok(EnginePipelineHost { pipeline, loader })
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
}
