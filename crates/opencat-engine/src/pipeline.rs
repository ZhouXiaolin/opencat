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
use opencat_core::ir::scene3::Scene3Raster;
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
    /// 3D 渲染器（[`Self::scene3_raster`] 的底层实例）。GPU 合成器靠它取
    /// 3D 输出纹理；CPU 路径不用。
    pub scene3_renderer: std::sync::Arc<opencat_three::LazyRenderer3d>,
    /// 3D 光栅化器：core 导出的 Scene3 段在 render_frame 之后由它渲染，
    /// 结果回填 media plan 占位条目（见 [`Self::render_scene3_section`]）。
    pub scene3_raster: std::sync::Arc<TexturedRaster3d>,
    /// 共享纹理登记处：3D 输出纹理与合成目标纹理的存活都归它管。
    /// 生产方（`scene3_renderer`）与消费方（GPU 合成器、预览器）共用同一个。
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

    /// 渲染本帧的 Scene3 段并回填 `media_plan` 的占位图条目。
    ///
    /// core 为每个 3D 场景录一条空 RGBA 的 generated-image 占位（同时把
    /// `DrawOp::Image{ImageRef::Generated}` 录进目标 canvas 保 z 序），这里
    /// 逐 commit 光栅化快照，把像素/GPU 句柄填回同 id 条目——下游
    /// `prepare_frame` 与 draw 执行器对 3D 无感知，签名零改动。
    pub fn render_scene3_section(
        &self,
        media_plan: &mut opencat_core::ir::media_plan::FrameMediaPlan,
        section: &opencat_core::ir::draw_frame::Scene3Section,
    ) -> Result<()> {
        for commit in &section.commits {
            // 占位不在本帧 plan 中 = 那条 `DrawOp::Image` 被裁掉了（元素透明/
            // 完全离屏）：没有消费者，光栅化纯属浪费，直接跳过。
            let Some(entry) = media_plan
                .generated_images
                .iter_mut()
                .find(|e| e.id == commit.image_id)
            else {
                continue;
            };
            let rasterized = self.scene3_raster.render(
                &commit.scene,
                &section.geometries,
                entry.width,
                entry.height,
            )?;
            entry.rgba = rasterized.rgba;
            entry.gpu = rasterized.gpu;
        }
        Ok(())
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
        crate::gpu_pipeline::enabled(),
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
        false,
    )
}

fn open_parsed_host_owned_with_fonts(
    parsed: ParsedComposition,
    mut loader: EngineLoader,
    scripts: V8JsContext,
    font_bytes: Vec<Vec<u8>>,
    font_bytes_by_face_id: std::collections::HashMap<String, Vec<u8>>,
    gpu_mode: bool,
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
    // 3D 渲染器（opencat-three，wgpu，懒初始化）：core 每帧把 Scene3 段
    // 原样导出进 RenderFrame.scene3，engine 在 render_frame 之后用
    // [`TexturedRaster3d`] 光栅化并回填 media plan 占位条目——纹理**来源**
    // （视频帧/位图路径）用 MediaContext 解码成位图表再交给 wgpu，core 与
    // opencat-three 都不碰媒体解码，像素不过 JSON 桥。无 3D 场景的合成不
    // 付任何 GPU 成本，GPU 不可用时只在真正提交 3D 场景的帧上报错（见
    // LazyRenderer3d）。
    // 共享纹理登记处：3D 输出纹理与合成目标纹理都登记在这里，宿主凭句柄
    // 取用、归还。谁保活、谁归还全在这一处定（见 opencat-gpu-native）。
    let textures = std::sync::Arc::new(opencat_gpu_native::TextureRegistry::new());
    let scene3_renderer = std::sync::Arc::new(opencat_three::LazyRenderer3d::new(
        textures.clone(),
    ));
    let scene3_raster = std::sync::Arc::new(TexturedRaster3d {
        inner: scene3_renderer.clone(),
        resolve_tx: std::sync::OnceLock::new(),
        gpu_mode,
    });
    // 文档 base 目录：脚本内相对资产路径（GLTFGeometry 等）对它 join——
    // three.js GLTFLoader 相对 URL 语义的 opencat 对应物。
    pipeline.set_document_base(&loader.base_dir().to_string_lossy());

    let composition = pipeline.composition().clone();
    loader.register_canvas_asset_aliases(&composition);

    Ok(EnginePipelineHost {
        pipeline,
        loader,
        scene3_renderer,
        scene3_raster,
        textures,
    })
}

fn prepare_err(err: PrepareError) -> anyhow::Error {
    anyhow::anyhow!(err)
}

/// 引擎侧 3D 光栅化器：[`opencat_three::LazyRenderer3d`] + 视频帧/位图
/// 解码。纹理槽表按「逐 mesh 先 `textures` 槽表后 `map`，场景序」的遍历
/// 规则构建（与 opencat-three 的槽位推导规则一致）。
///
/// 解码在专属 worker 线程上进行：`MediaContext`（ffmpeg 解码缓存）不是
/// `Send`，不能跨线程移动——它在 worker 线程内构造并终身驻留，channel 只
/// 携带 `Texture3`（来源，Send）进、`TextureBitmap3`（像素，Send）出。
/// 逐帧顺序渲染下 worker 内的解码缓存与 2D 路径同温（顺序解码，不回 seek）。
struct TexturedRaster3d {
    /// 3D 渲染器。宿主与光栅器共享同一实例：GPU 管线要靠它把 3D 渲染
    /// 目标登记到共享纹理表，消费方凭凭据取用。
    inner: std::sync::Arc<opencat_three::LazyRenderer3d>,
    /// 纹理解析 worker 的发送端（懒建，首个带纹理的场景才起线程）。
    resolve_tx: std::sync::OnceLock<std::sync::mpsc::Sender<TextureResolveJob>>,
    /// GPU 管线开关（默认开，`OC_GPU_PIPELINE=0` 关）：3D 输出留在显存，
    /// 交给 Skia 零拷贝导入，跳过 wgpu→CPU 的整帧回读。
    gpu_mode: bool,
}

/// 一帧的纹理解析请求（进 worker：来源列表；出 worker：按同序的位图表）。
struct TextureResolveJob {
    sources: Vec<opencat_core::ir::scene3::Texture3>,
    /// `Texture3::Surface` 的像素随 job 一起过来（surface 注册表在脚本线程，
    /// worker 线程取不到）。
    surfaces: Vec<(String, u32, u32, Arc<Vec<u8>>)>,
    reply: std::sync::mpsc::SyncSender<anyhow::Result<opencat_three::TextureTable3>>,
}

/// 把来源列表里的 `Texture3::Surface` 像素抓成本帧快照。surface 注册表是
/// 脚本线程的 thread-local，渲染线程/worker 取不到，必须在这里过手。
fn collect_surfaces(
    sources: &[opencat_core::ir::scene3::Texture3],
) -> anyhow::Result<Vec<(String, u32, u32, Arc<Vec<u8>>)>> {
    let mut out = Vec::new();
    for src in sources {
        if let opencat_core::ir::scene3::Texture3::Surface { id } = src {
            let rgba = opencat_core::text::surface::surface_rgba_arc(id)?;
            let (w, h) = opencat_core::text::surface::surface_dimensions(id)?;
            out.push((id.clone(), w, h, Arc::new(rgba.to_vec())));
        }
    }
    Ok(out)
}

fn spawn_texture_worker() -> std::sync::mpsc::Sender<TextureResolveJob> {
    let (tx, rx) = std::sync::mpsc::channel::<TextureResolveJob>();
    std::thread::Builder::new()
        .name("scene3-textures".into())
        .spawn(move || {
            // MediaContext 诞生并终身驻留在本线程（!Send 依赖不跨线程）。
            let mut media = crate::media::MediaContext::new();
            for job in rx {
                let out = (|| -> anyhow::Result<opencat_three::TextureTable3> {
                    let mut table = opencat_three::TextureTable3::new();
                    for source in &job.sources {
                        let bitmap = match source {
                            opencat_core::ir::scene3::Texture3::VideoFrame { path, time } => {
                                let bmp = media.frame_rgba_at_time_by_path(
                                    std::path::Path::new(path),
                                    *time as f64,
                                )?;
                                opencat_three::TextureBitmap3 {
                                    rgba: bmp.data,
                                    width: bmp.width,
                                    height: bmp.height,
                                }
                            }
                            // canvas → 纹理：脚本侧离屏 surface 的当前像素。
                            // surface 注册表是脚本线程的 thread-local，宿主在
                            // 光栅化前把整幅像素取一次随 job 传进来。
                            opencat_core::ir::scene3::Texture3::Surface { id } => {
                                let (rgba, w, h) = job
                                    .surfaces
                                    .iter()
                                    .find(|(sid, ..)| sid == id)
                                    .map(|(_, w, h, rgba)| (rgba.clone(), *w, *h))
                                    .ok_or_else(|| {
                                        anyhow::anyhow!(
                                            "surface 纹理 '{id}' 没有随本帧上传像素"
                                        )
                                    })?;
                                opencat_three::TextureBitmap3 {
                                    rgba,
                                    width: w,
                                    height: h,
                                }
                            }
                            opencat_core::ir::scene3::Texture3::Bitmap { path } => {
                                let (data, w, h) =
                                    media.get_bitmap(std::path::Path::new(path), None)?;
                                opencat_three::TextureBitmap3 {
                                    rgba: data,
                                    width: w,
                                    height: h,
                                }
                            }
                        };
                        table.push((source.table_key(), bitmap));
                    }
                    Ok(table)
                })();
                if job.reply.send(out).is_err() {
                    break; // 渲染侧已放弃本帧（管线关闭），收尾退出。
                }
            }
        })
        .expect("spawn scene3-texture worker");
    tx
}

impl opencat_core::ir::scene3::Scene3Raster for TexturedRaster3d {
    fn render(
        &self,
        scene: &opencat_core::ir::scene3::Scene3,
        geometries: &opencat_core::ir::scene3::GeometryRegistry,
        width: u32,
        height: u32,
    ) -> anyhow::Result<opencat_core::ir::scene3::Rasterized3d> {
        // 遍历规则即槽位序：textures 在前（马赛克槽表）、map 在后。
        let mut sources: Vec<opencat_core::ir::scene3::Texture3> = Vec::new();
        for mesh in &scene.meshes {
            for tex in mesh
                .material
                .textures
                .iter()
                .chain(mesh.material.map.iter())
                .chain(mesh.material.base_color_map.iter())
                .chain(mesh.material.metallic_roughness_map.iter())
                .chain(mesh.material.normal_map.iter())
            {
                let key = tex.table_key();
                if sources.iter().any(|s| s.table_key() == key) {
                    continue;
                }
                sources.push(tex.clone());
            }
        }
        if sources.len() > opencat_three::MAX_TEXTURE_SLOTS {
            anyhow::bail!(
                "3D 纹理来源 {} 路超过上限 {}（马赛克槽表 + map 合计）",
                sources.len(),
                opencat_three::MAX_TEXTURE_SLOTS
            );
        }
        let tx = self.resolve_tx.get_or_init(spawn_texture_worker);
        // 先把本帧所有 surface 来源的像素取出来（脚本线程的 thread-local）。
        let surfaces = collect_surfaces(&sources)?;
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        tx.send(TextureResolveJob {
            sources,
            surfaces,
            reply: reply_tx,
        })
        .map_err(|_| anyhow::anyhow!("scene3-texture worker 已退出"))?;
        let table = reply_rx
            .recv()
            .map_err(|_| anyhow::anyhow!("scene3-texture worker 丢失应答"))??;
        // 场景环境（three.js scene.environment，js:23-26）：equirect 位图
        // 走同一 worker 解码，交给渲染器做一次性 PMREM 预滤波
        //（LazyRenderer3d::set_environment 内按 Arc 指针缓存）。
        if let Some(env) = &scene.environment {
            let (env_tx, env_rx) = std::sync::mpsc::sync_channel(1);
            let env_sources = vec![env.clone()];
            let env_surfaces = collect_surfaces(&env_sources)?;
            tx.send(TextureResolveJob {
                sources: env_sources,
                surfaces: env_surfaces,
                reply: env_tx,
            })
            .map_err(|_| anyhow::anyhow!("scene3-texture worker 已退出"))?;
            let env_table = env_rx
                .recv()
                .map_err(|_| anyhow::anyhow!("scene3-texture worker 丢失应答"))??;
            let (_, env_bitmap) = env_table
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("环境纹理来源解析为空表"))?;
            self.inner
                .set_environment(&env_bitmap, scene.environment_srgb)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        // GPU 模式：渲染到显存并登记句柄，Skia 侧零拷贝导入（见
        // [`crate::gpu_pipeline`]）。CPU 模式：读回像素走原有路径。
        if self.gpu_mode {
            let handle = self
                .inner
                .render_to_gpu(scene, geometries, &table, width, height)?;
            return Ok(opencat_core::ir::scene3::Rasterized3d {
                rgba: std::sync::Arc::from(Vec::new()),
                gpu: Some(handle),
            });
        }
        Ok(opencat_core::ir::scene3::Rasterized3d::cpu(
            self.inner
                .render_with_textures(scene, geometries, &table, width, height)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use opencat_core::{frame_ctx::FrameCtx, script::js_context::JsContext};

    use super::open;

    /// Test fixture helper: line-shape JSON through the JSX extract entry (the
    /// product frontend) — core has no JSONL text entry point.
    fn draft_from_lines(
        lines: serde_json::Value,
    ) -> opencat_core::lifecycle::CompositionDraft {
        let extract = serde_json::json!({
            "meta": {"width": 10.0, "height": 10.0, "fps": 30.0, "duration": 0.1},
            "lines": lines,
        });
        let parsed = opencat_core::parse::jsx::from_extract(&extract, "", None)
            .expect("extract parses");
        opencat_core::lifecycle::CompositionDraft::from_parsed(parsed)
    }

    #[test]
    fn static_image_lifecycle_uses_request_asset_id() {
        use opencat_core::ir::draw_types::ImageRef;
        use opencat_core::pipeline::Pipeline;

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

        let jsx = r#"export const meta = { width: 64, height: 64, fps: 30, duration: 0.1 };
export default function App() {
  return (
    <div id="root" class="w-full h-full">
      <image id="pic" path="hero.png" class="w-[32px] h-[32px]" />
    </div>
  );
}"#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::V8JsContext::new().expect("js context");
        let mut host = open(jsx, loader, ctx).expect("open via lifecycle");

        // Host must key handles by the request AssetId (logical path), not a re-derived id.
        let handle = host.loader.handle(&opencat_core::AssetId::new(
            opencat_core::ir::asset_id::ResourceKind::Image,
            "hero.png",
        ));
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
        let dims = crate::probe::probe_image(PNG_1X1).expect("dims");
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

        let jsx = r#"export const meta = { width: 64, height: 64, fps: 25, duration: 0.4 };
export default function App() {
  return (
    <div id="root" class="w-full h-full">
      <lottie id="loader" path="loader.json" class="w-[40px] h-[30px]" />
    </div>
  );
}"#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::V8JsContext::new().expect("js context");
        let mut host = open(jsx, loader, ctx).expect("open via lifecycle");

        // Host registers under logical path key (probe) and canonical bundle id.
        assert!(
            host.loader
                .handle(&opencat_core::AssetId::new(
                    opencat_core::ir::asset_id::ResourceKind::Image,
                    "loader.json",
                ))
                .is_some(),
            "engine must cache primary JSON under logical locator"
        );
        assert!(
            host.loader
                .handle(&opencat_core::AssetId::new(
                    opencat_core::ir::asset_id::ResourceKind::Lottie,
                    "lottie:path:loader.json",
                ))
                .is_some(),
            "engine must also key primary JSON under request bundle AssetId"
        );

        // Pipeline catalog carries the Lottie bundle metadata.
        assert!(
            host.pipeline
                .catalog()
                .lotties
                .contains_key(&opencat_core::AssetId::new(
                    opencat_core::ir::asset_id::ResourceKind::Lottie,
                    "lottie:path:loader.json",
                )),
            "catalog must contain lottie bundle metadata for loader"
        );

        let frame = host.pipeline.render_frame(0).expect("render");
        assert!(
            frame
                .media
                .lottie_bundles
                .iter()
                .any(|b| b == "lottie:path:loader.json"),
            "FrameMediaPlan must list bundle id; got {:?}",
            frame.media.lottie_bundles
        );
        assert!(
            frame.media.images.is_empty(),
            "Lottie must not be disguised as image; got {:?}",
            frame.media.images
        );
        let has_op = frame.draw.ops.iter().any(|op| {
            matches!(op, DrawOp::LottieRect { bundle_id, .. } if bundle_id == "lottie:path:loader.json")
        });
        assert!(has_op, "draw must emit LottieRect; ops={:?}", frame.draw.ops);

        std::fs::remove_dir_all(&fixture_dir).ok();
    }

    #[test]
    fn jsx_document_font_is_merged_by_core_prepare() {
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

        let jsx = r#"
export const fonts = {
  default: "doc",
  faces: [{ id: "doc", family: "Noto Sans SC", path: "doc-sans.otf", role: "sans" }],
};
export default function App() {
  return (
    <div id="root" class="w-full h-full">
      <text id="t" class="font-sans text-white text-[24px]">你好</text>
    </div>
  );
}
"#;

        let loader = crate::resource::loader::EngineLoader::new(fixture_dir.clone(), cache_dir)
            .expect("loader");
        let ctx = crate::js_context::V8JsContext::new().expect("js context");
        let mut host = open(jsx, loader, ctx).expect("open with document font via core prepare");

        // font-sans must shape with document face; render must succeed.
        let frame = host.pipeline.render_frame(0).expect("render");
        assert!(
            !frame.draw.ops.is_empty(),
            "document font path must produce draw ops"
        );

        std::fs::remove_dir_all(&fixture_dir).ok();
    }

    // --- absent contract (#62) --------------------------------------------------
    //
    // Engine-level tests verifying that marking an asset absent produces
    // AbsentInput (not MissingInput) and that the reason string is preserved
    // through the host boundary.

    #[test]
    fn absent_input_surfaces_as_pipeline_open_error_at_engine_level() {
        use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError, ResourceKind};

        let draft = draft_from_lines(serde_json::json!([
            {"type":"div","id":"root","parentId":null},
            {"type":"image","id":"pic","parentId":"root","path":"photos/a.png"},
        ]));
        let id = draft.requirements().requests()[0].asset_id.clone();
        let mut inputs = HostInputs::empty();
        inputs.mark_absent(id, "engine-level not found").unwrap();
        let err = draft
            .prepare(inputs)
            .expect_err("absent must fail at engine level");
        assert!(
            matches!(err, PrepareError::AbsentInput { kind: ResourceKind::Image, .. }),
            "expected AbsentInput for engine-level absent, got {err:?}"
        );
    }

    #[test]
    fn absent_input_reason_preserved_through_engine_boundary() {
        use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError};

        let draft = draft_from_lines(serde_json::json!([
            {"type":"div","id":"root","parentId":null},
            {"type":"image","id":"pic","parentId":"root","path":"photos/a.png"},
        ]));
        let id = draft.requirements().requests()[0].asset_id.clone();
        let mut inputs = HostInputs::empty();
        inputs.mark_absent(id, "engine-level reason").unwrap();
        let err = draft.prepare(inputs).expect_err("absent must fail");
        match err {
            PrepareError::AbsentInput { reason, .. } => {
                assert_eq!(reason, "engine-level reason");
            }
            other => panic!("expected AbsentInput with reason, got {other:?}"),
        }
    }

    #[test]
    fn missing_input_still_distinct_from_absent_at_engine_level() {
        use opencat_core::lifecycle::{CompositionDraft, HostInputs, PrepareError, ResourceKind};

        let draft = draft_from_lines(serde_json::json!([
            {"type":"div","id":"root","parentId":null},
            {"type":"image","id":"pic","parentId":"root","path":"photos/a.png"},
        ]));
        // No mark_absent, no image metadata — prepare must fail with MissingInput.
        let inputs = HostInputs::empty();
        let err = draft.prepare(inputs).expect_err("missing must fail");
        assert!(
            matches!(err, PrepareError::MissingInput { kind: ResourceKind::Image, .. }),
            "unresponsive host must still get MissingInput; got {err:?}"
        );
    }
}
