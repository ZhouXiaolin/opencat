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

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use opencat_core::ir::RenderFrame;
use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError, ResourceKind};
use opencat_core::parse::preflight::collect_resource_requests_from_parsed;
use opencat_core::parse::ParsedComposition;
use opencat_core::pipeline::DefaultPipeline;
use opencat_core::fonts::font_asset_id;
use opencat_core::ir::asset_id::asset_id_for_lottie;
use opencat_core::parse::primitives::LottieSource;

use crate::EnginePipeline;
use crate::fonts::engine_default_font_faces;
use crate::js_context::V8JsContext;
use crate::resource::loader::EngineLoader;

/// Core pipeline opened by the engine through the explicit lifecycle
/// (`CompositionDraft` → `HostInputs` → `prepare` →
/// [`opencat_core::lifecycle::PreparedComposition::open_pipeline`]).
type CorePipeline = DefaultPipeline<V8JsContext>;

/// Engine host: owns the core pipeline **and** the engine resource owner.
///
/// Per issue #2 / #7, the core pipeline no longer owns an engine loader. The
/// engine fetches/caches bytes and prepares metadata itself, then opens core via
/// the lifecycle (`PreparedComposition::open_pipeline`). The [`EngineLoader`]
/// lives here so the frame consumer and audio mixer can read the cached bytes
/// for the current frame's media plan directly — they never reach through the
/// core pipeline.
pub struct EnginePipelineHost {
    /// Core pipeline (pure derivation; no loader access).
    pub pipeline: CorePipeline,
    /// Engine-owned resource owner: cached asset handles, fetcher, providers.
    pub loader: EngineLoader,
    /// 共享纹理登记处：GPU 流输出纹理与合成目标纹理的存活都归它管。
    /// 生产方（GPU 执行器）与消费方（GPU 合成器、预览器）共用同一个。
    pub textures: std::sync::Arc<opencat_gpu_native::TextureRegistry>,
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

    /// Delegate: composition info (width/height/fps/duration/audio plan).
    ///
    /// `audio_plan` on the returned [`CompositionInfo`] is the **canonical**
    /// composition-level audio schedule. All decode/mix/encode code reads from
    /// this plan; it must not re-walk the AST for segment offsets.
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

    /// Layout inspection from the same resolve/layout sessions as render.
    pub fn inspect_frame(
        &mut self,
        idx: u32,
    ) -> Result<Vec<opencat_core::pipeline::FrameElementRect>> {
        self.pipeline.inspect_frame(idx)
    }
}

/// Parse a composition and open it on the host-owned resource pipeline.
///
/// The engine completes the full host preparation chain before opening core:
/// collect declarative [`ResourceRequests`] → fetch/cache media bytes →
/// probe media metadata locally → hand typed metadata to core prepare.
/// Core alone merges fonts and opens the pipeline.
pub fn open(input: &str, mut loader: EngineLoader, scripts: V8JsContext) -> Result<EnginePipeline> {
    // Both remaining frontends (JSONL text, JSX) parse through source_io into a
    // self-contained ParsedComposition (tree extracted, driver embedded as the
    // script source). The host then fetches document font bytes + registers
    // cache handles; family index / fallback / fontdb merge happen in core
    // prepare (#19).
    let base_dir = loader
        .base_dir()
        .canonicalize()
        .unwrap_or_else(|_| loader.base_dir().to_path_buf());
    let parsed = crate::source_io::parse_with_base_dir(input, Some(&base_dir))?;
    let font_bytes = loader.load_font_manifest(&parsed.font_manifest)?;
    loader.register_font_handles(&parsed.font_manifest, &font_bytes)?;
    let host = open_parsed_host_owned_with_fonts(
        parsed,
        loader,
        scripts,
        engine_default_font_faces(),
        font_bytes,
    )?;
    Ok(host)
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
    scripts: V8JsContext,
    font_bytes: Vec<Vec<u8>>,
) -> Result<EnginePipelineHost> {
    // 调用方都是 CPU 消费器（render.rs 的测试直接调
    // `render_pipeline_frame_to_rgba`），所以 GPU 管线恒关。
    open_parsed_host_owned_with_fonts(
        parsed,
        loader,
        scripts,
        font_bytes,
        Default::default(),
    )
}

fn open_parsed_host_owned_with_fonts(
    parsed: ParsedComposition,
    mut loader: EngineLoader,
    scripts: V8JsContext,
    font_bytes: Vec<Vec<u8>>,
    font_bytes_by_face_id: std::collections::HashMap<String, Vec<u8>>,
) -> Result<EnginePipelineHost> {
    let draft = CompositionDraft::from_parsed(parsed);
    let requests = collect_resource_requests_from_parsed(draft.parsed());

    // Host fetch/cache under canonical AssetIds from core.
    loader.load_all(&requests)?;

    // Host probes bytes directly (issue #40) and inserts metadata into HostInputs.
    let mut inputs = HostInputs::empty()
        .with_base_font_faces(font_bytes)
        .with_sans_serif_family("Noto Sans SC")
        .with_font_fallback_dirs(crate::fonts::engine_font_fallback_dirs());

    // Image: read bytes → probe locally → insert metadata.
    for src in &requests.images {
        let Some(id) = opencat_core::ir::asset_id::asset_id_for_image(src) else {
            continue;
        };
        if let Some(handle) = loader.handle(&id) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(meta) = crate::probe::probe_image(&bytes) {
                    inputs.insert_image(id, meta).map_err(prepare_err)?;
                }
            }
        }
    }

    // Video: read bytes → probe locally → insert metadata.
    for src in &requests.videos {
        let id = opencat_core::ir::asset_id::asset_id_for_video(src);
        if let Some(handle) = loader.handle(&id) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(meta) = crate::probe::probe_video(&bytes) {
                    inputs.insert_video(id, meta).map_err(prepare_err)?;
                }
            }
        }
    }

    // Lottie: read primary JSON bytes → parse locally → insert metadata.
    for req in &requests.lotties {
        let Some(bundle_id) = asset_id_for_lottie(&req.source) else {
            continue;
        };
        // Lottie primary JSON is cached under the source key (path or url id).
        let source_id = match &req.source {
            LottieSource::Path(p) => {
                opencat_core::ir::asset_id::AssetId::new(
                    opencat_core::ir::asset_id::ResourceKind::Image,
                    p.clone(),
                )
            }
            LottieSource::Url(u) => opencat_core::ir::asset_id::asset_id_for_url(u),
            LottieSource::Unset => continue,
        };
        if let Some(handle) = loader.handle(&source_id) {
            if let Ok(bytes) = handle.read_bytes() {
                if let Ok(json) = std::str::from_utf8(&bytes) {
                    if let Ok(meta) = crate::probe::parse_lottie_meta(json) {
                        inputs.insert_lottie(bundle_id, meta).map_err(prepare_err)?;
                    }
                }
            }
        }
    }

    // Audio: register presence only.
    for src in &requests.audios {
        if let Some(id) = opencat_core::ir::asset_id::asset_id_for_audio(src) {
            inputs.insert_audio(id).map_err(prepare_err)?;
        }
    }

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
            .find(|f| font_asset_id(&f.source) == req.asset_id.key)
            .map(|f| f.id.as_str());
        let Some(face_id) = face_id else {
            continue;
        };
        let Some(font_bytes) = font_bytes_by_face_id.get(face_id) else {
            // Missing bytes will fail prepare with MissingInput for this asset.
            continue;
        };
        inputs
            .insert_document_font(req.asset_id.clone(), font_bytes.clone())
            .map_err(prepare_err)?;
    }

    let prepared = draft.prepare(inputs).map_err(prepare_err)?;
    let mut pipeline = prepared.open_pipeline(scripts)?;
    // 共享纹理登记处：GPU 流输出纹理与合成目标纹理都登记在这里，宿主凭
    // 句柄取用、归还。谁保活、谁归还全在这一处定（见 opencat-gpu-native）。
    let textures = std::sync::Arc::new(opencat_gpu_native::TextureRegistry::new());
    // 文档 base 目录：脚本内相对资产路径对它 join——three.js GLTFLoader
    // 相对 URL 语义的 opencat 对应物。
    pipeline.set_document_base(&loader.base_dir().to_string_lossy());

    let composition = pipeline.composition().clone();
    loader.register_canvas_asset_aliases(&composition);

    Ok(EnginePipelineHost {
        pipeline,
        loader,
        textures,
    })
}

fn prepare_err(err: PrepareError) -> anyhow::Error {
    anyhow::anyhow!(err)
}


