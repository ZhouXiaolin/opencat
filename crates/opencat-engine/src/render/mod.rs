use std::path::Path;

use anyhow::{Result, anyhow};

pub mod executor;
pub mod frame;
pub mod target;

use target::{CpuRenderTarget, FrameJob, RenderTarget, StreamRunner, make_render_target};

pub use crate::media::Mp4Config;

use crate::media::{MediaContext, encode_rgba_frames};
use opencat_core::frame_ctx::duration_secs_to_frames;
use opencat_core::ir::GeneratedImageId;
use skia_safe::Image;

use opencat_core::cache::lru::BoundedLruCache;

/// LRU of decoded Skia images for core-generated bitmaps (`canvas
/// putImageData` / surface auto-bake). The LRU evicts by ENTRY COUNT, not
/// bytes — the previous code passed a byte budget as the entry count, so a
/// per-frame bake key (a surface the script rewrites every frame) kept every
/// decoded 1920x1080 image alive and OOM-killed a 49s composition at
/// ~500 MiB/s.
/// The core `GeneratedImageTable` already watermarks per-frame keys at 3
/// entries (max_age_frames = 2 + the current frame); this cache only needs
/// to absorb repeated same-frame lookups, so a small entry count is the
/// correct bound. The byte-budget env var is kept for compatibility: it now
/// scales the entry count at 8 MB per 1920x1080 frame.
pub type GeneratedImageCache = BoundedLruCache<GeneratedImageId, Image>;

/// Entry-count bound matching the core table's 3-frame watermark, plus one
/// slot for the encode thread's in-flight frame.
pub const DEFAULT_GENERATED_IMAGE_CACHE_BYTES: usize = 64 * 1024 * 1024;

pub fn generated_image_cache_from_env() -> GeneratedImageCache {
    let budget = std::env::var("OC_GENERATED_IMAGE_CACHE_BYTES")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .map(|bytes| (bytes / (1920 * 1080 * 4)).max(4))
        .unwrap_or(4);
    GeneratedImageCache::new(budget)
}

pub enum OutputFormat {
    Mp4(Mp4Config),
    /// 分块并行渲染的一段：只渲 `[start, end)` 帧、纯视频；音轨由拼接方
    /// 统一编码（见 [`crate::media::concat_video_segments`]）。
    Mp4Segment(Mp4Config, std::ops::Range<u32>),
    /// PNG 序列（`frame_%04d.png`）；`Some(帧号表)` 只渲这些帧（一次提取，
    /// 多帧取样），`None` 全片。
    Png(Option<Vec<u32>>),
}

pub struct EncodingConfig {
    pub format: OutputFormat,
}

/// Render one pipeline frame to RGBA (CPU raster path).
///
/// `surface_w/h` is the surface + `read_pixels` size (MP4 may pass even-aligned
/// dims). Creates a fresh surface per call (acceptable cost for the offline path).
/// Generated Skia images are cached by [`GeneratedImageId`] across frames.
/// CPU 合成路径的 GPU 流执行底座（懒建，线程本地——GL 上下文非线程安全；
/// wgpu 设备同槽常驻，避免逐帧重建 adapter/device）。
thread_local! {
    static CPU_GL: std::cell::RefCell<
        Option<(
            Option<crate::gpu::session::GlSession>,
            Option<crate::gpu::stream_executor::GpuStreamExecutor>,
            crate::gpu::wgpu_executor::WgpuStreamExecutor,
        )>,
    > = const { std::cell::RefCell::new(None) };
}

/// 渲染收尾：显式拆掉本线程的常驻 GL 会话（CPU 合成流执行 + GLSL 反射器）。
/// 不能留给 TLS 析构——进程退出时其他线程已被杀，SwiftShader 的 eglTerminate
/// 在 TLS 析构里等它们收尾会永久阻塞。
pub fn release_gl_sessions() {
    let _ = CPU_GL.try_with(|cell| cell.borrow_mut().take());
    crate::script::bridge::release_reflector_session();
}

/// CPU 合成路径的 GPU 流执行：readback 喂占位条目（无零拷贝）。按 backend
/// 分派（[`execute_streams_by_backend`]）——WebGPU 流走 wgpu 执行器，不进 GL。
fn cpu_execute_gpu_streams(
    media_plan: &mut opencat_core::ir::media_plan::FrameMediaPlan,
    section: &opencat_core::ir::GpuSection,
) -> Result<()> {
    CPU_GL.with(|cell| -> Result<()> {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some((
                None,
                None,
                crate::gpu::wgpu_executor::WgpuStreamExecutor::new(),
            ));
        }
        let (session, executor, wgpu_executor) = slot.as_mut().unwrap();
        let needs_gl = section
            .streams
            .iter()
            .any(|s| s.backend == opencat_core::ir::GpuBackend::WebGL2);
        if needs_gl && session.is_none() {
            *session = Some(crate::gpu::session::GlSession::offscreen()?);
            *executor = Some(crate::gpu::stream_executor::GpuStreamExecutor::new());
        }
        if let Some(session) = session.as_ref() {
            session.make_current()?;
        }
        execute_streams_by_backend(
            session.as_ref().map(|session| session.gl()),
            executor.as_mut(),
            wgpu_executor,
            section,
            media_plan,
            None,
        )?;
        Ok(())
    })
}

/// GPU 流按 backend 分派的唯一入口：WebGPU 流交 `wgpu_executor`（独立 wgpu
/// 设备，与 GL 上下文零共享状态），WebGL2 流交 GL 执行器；混合帧两者都执行。
/// `textures = Some` 时，WebGL 输出登记到该表供 Skia 直接采样；`None` 时
/// 执行器读回像素并回填 plan。返回 GL 输出（调用方合成消费后 recycle）。
/// WebGPU 的输出由 wgpu 执行器内部回填，无纹理句柄产出。
fn execute_streams_by_backend(
    gl: Option<std::sync::Arc<glow::Context>>,
    gpu_executor: Option<&mut crate::gpu::stream_executor::GpuStreamExecutor>,
    wgpu_executor: &mut crate::gpu::wgpu_executor::WgpuStreamExecutor,
    gpu: &opencat_core::ir::GpuSection,
    plan: &mut opencat_core::ir::media_plan::FrameMediaPlan,
    textures: Option<&crate::gpu::TextureRegistry>,
) -> Result<Vec<crate::gpu::stream_executor::StreamOutput>> {
    let has_webgpu = gpu
        .streams
        .iter()
        .any(|s| s.backend == opencat_core::ir::GpuBackend::WebGPU);
    if has_webgpu {
        let webgpu_streams: Vec<&opencat_core::ir::gpu_stream::GpuStream> = gpu
            .streams
            .iter()
            .filter(|s| s.backend == opencat_core::ir::GpuBackend::WebGPU)
            .collect();
        wgpu_executor.execute(&webgpu_streams, plan)?;
    }
    // GL 执行器只收 WebGL2 流；无 WebGPU 流时免 clone 直传整段（空段执行器
    // 早退，不碰 GL 状态）。
    let filtered;
    let gl_section: &opencat_core::ir::gpu_stream::GpuSection = if has_webgpu {
        filtered = opencat_core::ir::gpu_stream::GpuSection {
            streams: gpu
                .streams
                .iter()
                .filter(|s| s.backend == opencat_core::ir::GpuBackend::WebGL2)
                .cloned()
                .collect(),
        };
        &filtered
    } else {
        gpu
    };
    if gl_section.streams.is_empty() {
        return Ok(Vec::new());
    }
    let gl = gl.ok_or_else(|| anyhow!("WebGL2 stream requires a GL session"))?;
    let gpu_executor = gpu_executor
        .ok_or_else(|| anyhow!("WebGL2 stream requires a GL executor"))?;
    gpu_executor.execute(&gl, gl_section, plan, textures)
}

/// 调试用：dump 原始 GPU 流（诊断两链命令分歧）。全片体量过大：默认只落
/// 前 2 帧；`OC_DUMP_GPU_STREAM_ALL=1` 全量。
fn dump_gpu_streams(dir: &std::path::Path, gpu: &opencat_core::ir::GpuSection, label: u32) {
    if label >= 2 && std::env::var_os("OC_DUMP_GPU_STREAM_ALL").is_none() {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    for (si, stream) in gpu.streams.iter().enumerate() {
        let mut buf: Vec<u8> = Vec::new();
        fn w(buf: &mut Vec<u8>, chunk: &[u8]) {
            buf.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            buf.extend_from_slice(chunk);
        }
        buf.extend_from_slice(&stream.image_id.0.to_le_bytes());
        w(&mut buf, stream.target_id.as_bytes());
        buf.extend_from_slice(&stream.width.to_le_bytes());
        buf.extend_from_slice(&stream.height.to_le_bytes());
        w(&mut buf, &stream.ops.ops);
        w(&mut buf, &stream.ops.bytes);
        buf.extend_from_slice(&(stream.ops.byte_ranges.len() as u32).to_le_bytes());
        for r in &stream.ops.byte_ranges {
            buf.extend_from_slice(&r.start.to_le_bytes());
            buf.extend_from_slice(&r.len.to_le_bytes());
        }
        let _ = std::fs::write(dir.join(format!("gpu-frame{label}-s{si}.bin")), &buf);
    }
}

/// Vulkan 渲染会话：Skia 固定走 Vulkan。WebGL2 流固定由 ANGLE 执行；
/// WebGPU 流固定由 wgpu 执行。ANGLE 能交出共享 VkDevice 时，WebGL 输出
/// 经 VkImage 直接采样；否则 ANGLE 与 Skia 各自持有 Vulkan device，
/// WebGL 输出先 CPU 回读。`OC_ANGLE_SWIFTSHADER` 只切换 Vulkan device。
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct VkRenderSession {
    // 先析构 Skia 合成器，再析构 ANGLE。二者可能共用一个 VkDevice。
    compositor: crate::gpu::compositor::GpuCompositor,
    /// ANGLE Vulkan 会话：WebGL2 流的执行端。
    gl_session: crate::gpu::session::GlSession,
    gpu_executor: crate::gpu::stream_executor::GpuStreamExecutor,
    wgpu_executor: crate::gpu::wgpu_executor::WgpuStreamExecutor,
    textures: crate::gpu::TextureRegistry,
    /// 本帧 GL 流的输出，合成完成后归还复用池。
    stream_outputs: Vec<crate::gpu::stream_executor::StreamOutput>,
    /// ANGLE 的队列族索引（导入纹理的 `current_queue_family`）。
    queue_family: u32,
    /// ANGLE 与 Skia 是否共享同一个 Vulkan device。
    zero_copy: bool,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl VkRenderSession {
    /// 建 Vulkan 渲染会话。
    ///
    /// 硬件与 SwiftShader 都走 Vulkan。SwiftShader 配置通过
    /// `OC_ANGLE_SWIFTSHADER` 选择 device。
    pub fn new(width: u32, height: u32) -> Result<Self> {
        // ANGLE 是 WebGL2 的唯一执行面。硬件与 SwiftShader 切换它的
        // Vulkan device；两种配置都必须保留这个会话。
        let gl_session = crate::gpu::session::GlSession::offscreen()?;
        let handles = gl_session.angle_context().vulkan_handles();
        let (session, queue_family, zero_copy) = if let Some(handles) = handles {
            let queue_family = handles.queue_family_index;
            let session = crate::gpu::vk::VkSession::from_angle(&handles)?;
            (session, queue_family, true)
        } else {
            tracing::warn!("ANGLE 未交出 Vulkan 句柄；WebGL 保留 ANGLE 执行，输出改走 CPU 回读");
            let session = crate::gpu::vk::VkSession::new()?;
            let queue_family = session.queue_index() as u32;
            (session, queue_family, false)
        };
        let compositor = crate::gpu::compositor::GpuCompositor::new(session, width, height)?;
        Ok(Self {
            compositor,
            gl_session,
            gpu_executor: crate::gpu::stream_executor::GpuStreamExecutor::new(),
            wgpu_executor: crate::gpu::wgpu_executor::WgpuStreamExecutor::new(),
            textures: crate::gpu::TextureRegistry::new(),
            stream_outputs: Vec::new(),
            queue_family,
            zero_copy,
        })
    }

    /// 执行阶段（调用方线程 = ANGLE/Skia 上下文所在线程）：GL 流执行 +
    /// Skia 合成 + 同步读回。帧是纯数据（[`opencat_core::ir::RenderFrame`]），
    /// 与 script 阶段经 flume 通道分线程衔接。`media_ctx`（视频解码/图像
    /// 光栅缓存）和纹理登记处只属于本阶段；loader 由执行线程共享。
    /// `label` 仅作日志/dump 标签（执行语义对「第几帧」零依赖——M4-⑦ 红线）。
    #[allow(clippy::too_many_arguments)]
    pub fn execute_render(
        &mut self,
        render: opencat_core::ir::RenderFrame,
        executor: &mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
        media_ctx: &mut MediaContext,
        loader: &crate::resource::loader::EngineLoader,
        label: u32,
    ) -> Result<Vec<u8>> {
        let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
        let t0 = timing.then(std::time::Instant::now);
        self.gl_session.make_current()?;
        let t_script: Option<std::time::Duration> = None;
        let mut frame = render.draw;
        let mut media_plan = render.media;
        if let Some(gpu) = render.gpu.as_ref() {
            if let Some(dir) = std::env::var_os("OC_DUMP_GPU_STREAM") {
                dump_gpu_streams(std::path::Path::new(&dir), gpu, label);
            }
            if std::env::var_os("OC_TRACE_STREAMS").is_some() {
                eprintln!("[streams] f{label} n={}", gpu.streams.len());
                for s in &gpu.streams {
                    eprintln!(
                        "[streams]   {:?} {}x{} backend={:?}",
                        s.target_id, s.width, s.height, s.backend
                    );
                }
            }
        }
        // 流的执行必须在 ANGLE 上下文 current 时进行（produce 阶段可能
        // 切走；跨线程后 current 本就归本线程）。
        self.gl_session.make_current()?;
        if let Some(gpu) = &render.gpu {
            self.stream_outputs = execute_streams_by_backend(
                Some(self.gl_session.gl()),
                Some(&mut self.gpu_executor),
                &mut self.wgpu_executor,
                gpu,
                &mut media_plan,
                self.zero_copy.then_some(&self.textures),
            )?;
            // 共享 VkQueue 没有跨 API 隐式顺序。ANGLE 的命令完成前
            // Skia 不能采样导出纹理。无共享 device 时执行器已经同步回读。
            if self.zero_copy
                && gpu
                    .streams
                    .iter()
                    .any(|s| s.backend == opencat_core::ir::GpuBackend::WebGL2)
            {
                use glow::HasContext as _;
                unsafe {
                    self.gl_session.gl().finish();
                }
            }
        }
        let t_streams = t0.map(|t| t.elapsed());
        let gpu_handles: Vec<crate::gpu::TextureId> = media_plan
            .generated_images
            .iter()
            .filter_map(|g| g.gpu.map(|h| crate::gpu::TextureId::from_raw(h.token)))
            .collect();

        {
            let (canvas, skia) = self.compositor.split();
            // 帧内把合成 context 挂给 executor：canvas 位图/pattern 瓦片等
            // 离屏面优先建 GPU 后端，与主合成面同 context、零拷贝贴回。块尾
            // 撤下（借用只在瓦片重放期间被解引用）。
            executor.gpu_ctx = Some(skia as *mut _);
            canvas.clear(skia_safe::Color::TRANSPARENT);
            let mut resolver = VkResolver {
                skia,
                textures: &self.textures,
                queue_family: self.queue_family,
            };
            crate::render::frame::execute_render_frame_with_gpu(
                &mut frame,
                &media_plan,
                loader,
                crate::render::frame::FrameExec {
                    executor,
                    media_ctx,
                    generated_cache,
                },
                Some(&mut resolver),
                canvas,
            )
            .map_err(|e| anyhow!("{e}"))?;
            executor.gpu_ctx = None;
            let _ = skia;
        }
        let t_draw = t0.map(|t| t.elapsed());
        let rgba = self.compositor.finish_frame()?;

        // 合成完成（像素已进 Vulkan 渲染目标并读回）：输出归还复用池，归还凭据。
        let outputs = std::mem::take(&mut self.stream_outputs);
        self.gpu_executor.recycle(outputs);
        for id in gpu_handles {
            self.textures.release(id);
        }
        if let Some(t0) = t0 {
            let t_end = t0.elapsed();
            eprintln!(
                "[timing-vk] f{label} script={:.1}ms gl-stream={:.1}ms draw={:.1}ms readback={:.1}ms total={:.1}ms",
                t_script.unwrap_or_default().as_secs_f64() * 1e3,
                (t_streams.unwrap_or_default() - t_script.unwrap_or_default()).as_secs_f64() * 1e3,
                (t_draw.unwrap_or_default() - t_streams.unwrap_or_default()).as_secs_f64() * 1e3,
                (t_end - t_draw.unwrap_or_default()).as_secs_f64() * 1e3,
                t_end.as_secs_f64() * 1e3,
            );
        }
        Ok(rgba)
    }
}

/// Vulkan 路径的图像解析器：凭共享纹理表取 ANGLE 的 GL 纹理名，导出
/// VkImage 导入 Skia（零拷贝）。
#[cfg(any(target_os = "windows", target_os = "linux"))]
struct VkResolver<'a> {
    skia: &'a mut skia_safe::gpu::DirectContext,
    textures: &'a crate::gpu::TextureRegistry,
    queue_family: u32,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl crate::render::frame::GpuImageResolver for VkResolver<'_> {
    fn import(
        &mut self,
        id: crate::gpu::TextureId,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Image> {
        let native = self
            .textures
            .resolve(id)
            .ok_or_else(|| anyhow!("纹理凭据 {id:?} 不在共享表里（已被归还？）"))?;
        crate::gpu::vk::import_stream_texture(self.skia, native, width, height, self.queue_family)
    }
}

/// 打开 HTML，逐帧执行并编码。CPU 光栅和 Vulkan 都输出预乘 alpha 的 RGBA。
pub fn render_from_source(
    source: &str,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
) -> Result<()> {
    render_from_source_with_base(source, None, output_path, config)
}

/// 分块并行渲染的父进程：只提取合成、不渲染，取帧数、帧率与混好的音轨
/// （切段与拼接用；各段自己再提取一次）。
pub fn probe_composition_with_base(
    input: &str,
    base_dir: Option<&Path>,
) -> Result<(u32, u32, Option<crate::media::AudioTrack>)> {
    let pipeline = crate::pipeline::open_engine_pipeline(input, base_dir)?;
    let info = pipeline.info().clone();
    let frame_count = duration_secs_to_frames(info.duration, info.fps);
    let audio_track = crate::media::audio::build_audio_track_from_pipeline(&pipeline)?;
    Ok((frame_count, info.fps, audio_track))
}

pub fn render_from_source_with_base(
    input: &str,
    base_dir: Option<&Path>,
    output_path: impl AsRef<Path>,
    config: &EncodingConfig,
) -> Result<()> {
    let output_path = output_path.as_ref();
    let mut pipeline = crate::pipeline::open_engine_pipeline(input, base_dir)?;
    let info = pipeline.info().clone();
    let frame_count = duration_secs_to_frames(info.duration, info.fps);

    // 音轨只有整片 MP4 要：分段由拼接方统一编码，PNG 不带音频。
    let audio_track = match &config.format {
        OutputFormat::Mp4(_) => crate::media::audio::build_audio_track_from_pipeline(&pipeline)?,
        _ => None,
    };

    match &config.format {
        OutputFormat::Png(frames) => {
            let mut target = make_render_target(info.width, info.height, info.fps)?;
            let all: Vec<u32>;
            let frames = match frames {
                Some(list) => list.as_slice(),
                None => {
                    all = (0..frame_count).collect();
                    &all
                }
            };
            for &i in frames.iter().filter(|&&i| i < frame_count) {
                let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
                let ts = timing.then(std::time::Instant::now);
                let render = pipeline.render_frame(i)?;
                if let Some(t) = ts {
                    eprintln!(
                        "[timing-script] f{i} script={:.1}ms",
                        t.elapsed().as_secs_f64() * 1e3
                    );
                }
                let rgba = target.execute(FrameJob {
                    render,
                    loader: pipeline.loader(),
                    label: i,
                })?;
                let img = image::RgbaImage::from_raw(info.width, info.height, rgba)
                    .ok_or_else(|| anyhow!("failed to build PNG image from RGBA frame"))?;
                let filename = output_path.join(format!("frame_{:04}.png", i));
                img.save(&filename)?;
            }
        }
        OutputFormat::Mp4(mp4_config) | OutputFormat::Mp4Segment(mp4_config, _) => {
            let range = match &config.format {
                OutputFormat::Mp4Segment(_, range) => {
                    range.start.min(frame_count)..range.end.min(frame_count)
                }
                _ => 0..frame_count,
            };
            let aligned_info = if info.width % 2 != 0 || info.height % 2 != 0 {
                let aw = (info.width + 1) & !1;
                let ah = (info.height + 1) & !1;
                (aw, ah)
            } else {
                (info.width, info.height)
            };

            if !crate::gpu::requested() {
                // CPU 后端：无 GPU 执行线程可拆，串行 script→execute 同步走。
                let mut target = CpuRenderTarget::new(aligned_info.0, aligned_info.1, info.fps)?;
                // 段首预热一帧再丢弃：进程内第一帧的脚本相位（TLA 模块的
                // hf-seek 等）与连续渲染不同，预热后段内各帧与整片同相位。
                if range.start > 0 {
                    let warm = pipeline.render_frame(range.start - 1)?;
                    target.execute(FrameJob {
                        render: warm,
                        loader: pipeline.loader(),
                        label: range.start - 1,
                    })?;
                }
                let start = range.start;
                encode_rgba_frames(
                    output_path,
                    aligned_info.0,
                    aligned_info.1,
                    info.fps,
                    range.end - range.start,
                    mp4_config,
                    audio_track.as_ref(),
                    |_, _| {},
                    |encode_index| {
                        let label = start + encode_index;
                        let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
                        let ts = timing.then(std::time::Instant::now);
                        let render = pipeline.render_frame(label)?;
                        if let Some(t) = ts {
                            eprintln!(
                                "[timing-script] f{label} script={:.1}ms",
                                t.elapsed().as_secs_f64() * 1e3
                            );
                        }
                        let px = target.execute(FrameJob {
                            render,
                            loader: pipeline.loader(),
                            label,
                        })?;
                        Ok(Some(px))
                    },
                )?;
                return Ok(());
            }

            // ── flume 三级流水线（GPU 路径）────────────────────────────
            // script（主线程；V8 isolate 非 Send 不能搬家）→ gpu 执行线程
            // （ANGLE/Skia 上下文 + media_ctx 归它所有）→ x264（编码在
            // `encode_rgba_frames` 内与渲染交错）。帧序 = channel FIFO，
            // 各阶段内部严格串行——frame=f(t) 确定性不变。loader 是 open 后
            // 只读的 Arc 共享，纹理登记处归执行线程；背压 2 帧。
            // Skia 的 GrDirectContext 非 Send（rust-skia 保守禁跨线程）——
            // 执行线程经 StreamRunner 自建 target（会话/缓存/media_ctx 归
            // 它所有），主线程零 GPU 状态。Png、MP4 与单帧统一经工厂选择
            // Vulkan 或 CPU；硬件与 SwiftShader 不改变 backend。
            let runner = StreamRunner::spawn(
                move || make_render_target(aligned_info.0, aligned_info.1, info.fps),
                pipeline.loader_arc().clone(),
            )?;

            // 段首预热一帧再丢弃：进程内第一帧的脚本相位（TLA 模块的
            // hf-seek 等）与连续渲染不同，预热后段内各帧与整片同相位。
            // 预热帧的 script 在主线程先行产出，经通道带 discard 标志送达。
            if range.start > 0 {
                let warm = pipeline.render_frame(range.start - 1)?;
                runner.submit(range.start - 1, warm, true);
            }
            // 首真帧先行产出：编码器首轮回调里 render(下一帧) 与 gpu 线程
            // 执行本帧重叠。没有这一步 recv 等的就是自己刚 send 的帧——
            // gpu 段（~96ms）整个串行到 script 段（~270ms）之后，流水线
            // 退化为逐帧串行（370ms/帧）；错开一帧后稳态节奏回到
            // max(script, gpu) ≈ 270ms/帧。帧序 = 双通道 FIFO，确定性不变。
            {
                let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
                let ts = timing.then(std::time::Instant::now);
                match pipeline.render_frame(range.start) {
                    Ok(rf) => {
                        if let Some(t) = ts {
                            eprintln!(
                                "[timing-script] f{} script={:.1}ms",
                                range.start,
                                t.elapsed().as_secs_f64() * 1e3
                            );
                        }
                        runner.submit(range.start, rf, false);
                    }
                    Err(e) => return Err(anyhow!("script 阶段 f{}: {e:#}", range.start)),
                }
            }

            // script 推进嵌在 frame_fn 里：编码器每要一帧像素前，先在主
            // 线程 render **下一帧**（首帧已在上面先行产出）——render(N+1)
            // 与 gpu 线程执行 N 重叠，随后 recv 拿 N 的像素（此时早已完成）。
            // 若 script 独立循环先行，rx_px 窗口填满后 gpu/script 相互等待
            // 而 encode_rgba_frames 还未启动——死锁。
            let mut script_next = range.start + 1;
            let mut script_error: Option<anyhow::Error> = None;
            let range_end = range.end;
            let encode_result = encode_rgba_frames(
                output_path,
                aligned_info.0,
                aligned_info.1,
                info.fps,
                range.end - range.start,
                mp4_config,
                audio_track.as_ref(),
                |_, _| {},
                |_encode_index| {
                    if let Some(e) = script_error.as_ref() {
                        anyhow::bail!("script 阶段: {e:#}");
                    }
                    if script_next < range_end {
                        let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
                        let ts = timing.then(std::time::Instant::now);
                        match pipeline.render_frame(script_next) {
                            Ok(rf) => {
                                if let Some(t) = ts {
                                    eprintln!(
                                        "[timing-script] f{} script={:.1}ms",
                                        script_next,
                                        t.elapsed().as_secs_f64() * 1e3
                                    );
                                }
                                // gpu 线程已死时 submit 静默：错误在槽位，
                                // 下面的 recv 会立刻失败
                                runner.submit(script_next, rf, false);
                            }
                            Err(e) => {
                                script_error = Some(anyhow!("script 阶段 f{}: {e:#}", script_next));
                                anyhow::bail!("script 阶段 f{}: {e:#}", script_next);
                            }
                        }
                        script_next += 1;
                    }
                    runner.check_error()?;
                    let px = runner.recv_px()?;
                    Ok(Some(px))
                },
            );
            // 收场：script 已推进完（或中途错），帧通道随 finish 关闭，
            // gpu 线程 recv None 自然退出。根因优先：script/gpu 的错误
            // 盖过编码错误（后者多半是前者引起）。
            let stream_result = runner.finish();
            if let Some(e) = script_error {
                return Err(e);
            }
            stream_result?;
            encode_result?;
        }
    }

    Ok(())
}

/// Render a single frame from an HTML/JS dist document via EnginePipeline, returning raw RGBA bytes.
pub fn render_single_frame_from_source(
    source: &str,
    frame_index: u32,
) -> Result<(Vec<u8>, u32, u32)> {
    render_single_frame_from_source_with_base(source, None, frame_index)
}

/// Render a single frame with explicit base directory for resolving relative asset paths.
pub fn render_single_frame_from_source_with_base(
    input: &str,
    base_dir: Option<&Path>,
    frame_index: u32,
) -> Result<(Vec<u8>, u32, u32)> {
    let mut pipeline = crate::pipeline::open_engine_pipeline(input, base_dir)?;
    let info = pipeline.info().clone();
    let frame_count = duration_secs_to_frames(info.duration, info.fps);

    if frame_index >= frame_count {
        anyhow::bail!(
            "frame_index {} out of range (composition has {} frames)",
            frame_index,
            frame_count
        );
    }

    let mut target = make_render_target(info.width, info.height, info.fps)?;
    let render = pipeline.render_frame(frame_index)?;
    let rgba = match target.execute(FrameJob {
        render,
        loader: pipeline.loader(),
        label: frame_index,
    }) {
        Ok(rgba) => rgba,
        Err(e) => {
            eprintln!("render_frame_blocking 失败: {e:#}");
            return Err(e);
        }
    };

    Ok((rgba, info.width, info.height))
}

pub fn render_single_frame_png_with_base(
    input: &str,
    base_dir: Option<&Path>,
    output_path: impl AsRef<Path>,
    frame_index: u32,
) -> Result<()> {
    let output_path = output_path.as_ref();
    let (rgba, width, height) =
        render_single_frame_from_source_with_base(input, base_dir, frame_index)?;
    write_rgba_png(output_path, width, height, rgba)
}

pub fn render_single_frame_png(
    input: &str,
    output_path: impl AsRef<Path>,
    frame_index: u32,
) -> Result<()> {
    render_single_frame_png_with_base(input, None, output_path, frame_index)
}

fn write_rgba_png(output_path: &Path, width: u32, height: u32, mut rgba: Vec<u8>) -> Result<()> {
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    // PNG stores unpremultiplied RGBA, but the renderer reads premul pixels
    // (raster_n32_premul read_pixels); convert back so semi-transparent pixels
    // match sources like resvg.
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 0 {
            px[..3].fill(0);
        } else if a != 255 {
            for ch in &mut px[..3] {
                *ch = ((*ch as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }

    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| anyhow!("failed to build PNG image from RGBA frame"))?;
    image.save(output_path)?;
    Ok(())
}

impl EncodingConfig {
    pub fn mp4() -> Self {
        // 对拍 corpus 需要无损引擎腿（ref 侧 crf 0）：环境量覆盖默认
        // crf，产品默认保持 18 不动。
        let crf = std::env::var("OC_VIDEO_CRF")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(18);
        Self {
            format: OutputFormat::Mp4(Mp4Config {
                crf,
                preset: "fast".to_string(),
                threads: None,
            }),
        }
    }

    pub fn mp4_with(config: Mp4Config) -> Self {
        Self {
            format: OutputFormat::Mp4(config),
        }
    }

    /// 分块并行渲染的一段（见 [`OutputFormat::Mp4Segment`]），编码参数同整片。
    pub fn mp4_segment(range: std::ops::Range<u32>) -> Self {
        let OutputFormat::Mp4(config) = Self::mp4().format else {
            unreachable!("mp4() always builds the Mp4 format")
        };
        Self {
            format: OutputFormat::Mp4Segment(config, range),
        }
    }

    pub fn png() -> Self {
        Self {
            format: OutputFormat::Png(None),
        }
    }

    /// 只渲指定帧的 PNG 序列（一次提取、多帧取样——对齐迭代用）。
    pub fn png_frames(frames: Vec<u32>) -> Self {
        Self {
            format: OutputFormat::Png(Some(frames)),
        }
    }
}

#[cfg(test)]
mod backend_dispatch_tests {
    //! backend 分派回归（批次 A）：WebGPU 流必须进 wgpu 执行器、绝不触碰
    //! GL 执行器的 loader；执行后占位条目按 CPU readback 回填。
    use super::execute_streams_by_backend;
    use opencat_core::ir::draw_types::TableRange;
    use opencat_core::ir::generated_image::GeneratedImageId;
    use opencat_core::ir::gpu_stream::gl_opcode as G;
    use opencat_core::ir::gpu_stream::wgpu_opcode as W;
    use opencat_core::ir::gpu_stream::{GpuBackend, GpuOpFrame, GpuSection, GpuStream};
    use opencat_core::ir::media_plan::FrameMediaPlan;

    /// GPU 环境守卫：这组测试跑真实 ANGLE（WebGL2）与 Vulkan（WebGPU），
    /// 只在渲染开发机上有意义。以 `OC_ANGLE_DIR` 为"渲染开发机"的代理
    /// 信号——CI 只做编译面检查，无此变量的环境优雅跳过，不当失败。
    fn gpu_env_available(tag: &str) -> bool {
        if std::env::var_os("OC_ANGLE_DIR").is_some() {
            return true;
        }
        eprintln!("skipping {tag}: OC_ANGLE_DIR not set (compile-only env)");
        false
    }

    /// 手构一条最小 WebGPU 流：configure（bgra8unorm）→ getCurrentTexture →
    /// view → render pass（纯 clear 红）→ pass end → encoder finish → submit。
    /// 编码协议见 `webgpu_api.js`（录制端）与 `wgpu_executor.rs`（执行端），
    /// 两端唯一权威；此处的序列是协议的最小忠实样本。
    fn webgpu_clear_stream() -> GpuStream {
        let fmt = b"bgra8unorm";
        let view_desc = br#"{}"#;
        let pass_desc = br#"{"colorAttachments":[{"view":{"h":200},"clearValue":{"r":1,"g":0,"b":0,"a":1},"loadOp":"clear","storeOp":"store"}]}"#;
        // intern 池：三段变长数据按序拼接，ranges 给出各自的流内偏移。
        let mut bytes: Vec<u8> = Vec::new();
        let mut ranges: Vec<TableRange> = Vec::new();
        for seg in [fmt.as_slice(), view_desc, pass_desc] {
            ranges.push(TableRange {
                start: bytes.len() as u32,
                len: seg.len() as u32,
            });
            bytes.extend_from_slice(seg);
        }
        let (fmt_r, view_r, pass_r) = (0u32, 1u32, 2u32); // range id（数组下标）
        let le32 = |v: u32| -> Vec<u8> { v.to_le_bytes().to_vec() };
        let cat = |parts: &[&[u8]]| -> Vec<u8> { parts.concat() };

        let mut ops: Vec<u8> = Vec::new();
        let mut push = |op: u8, payload: &[u8]| {
            ops.push(op);
            ops.extend_from_slice(payload);
        };
        // CANVAS_CONFIGURE {format_range, alpha_mode(0=opaque), usage}
        push(
            W::CANVAS_CONFIGURE,
            &cat(&[&le32(fmt_r)[..], &[0][..], &le32(0)[..]]),
        );
        // CANVAS_GET_CURRENT_TEXTURE {handle}
        push(W::CANVAS_GET_CURRENT_TEXTURE, &le32(100));
        // TEXTURE_VIEW_CREATE {handle, texture, desc_range}
        push(
            W::TEXTURE_VIEW_CREATE,
            &cat(&[&le32(200)[..], &le32(100)[..], &le32(view_r)[..]]),
        );
        // ENC_BEGIN_RENDER_PASS {enc, pass, desc_range}
        push(
            W::ENC_BEGIN_RENDER_PASS,
            &cat(&[&le32(1)[..], &le32(2)[..], &le32(pass_r)[..]]),
        );
        // PASS_END {pass}
        push(W::PASS_END, &le32(2));
        // ENC_FINISH {enc, cmd_buf}
        push(W::ENC_FINISH, &cat(&[&le32(1)[..], &le32(3)[..]]));
        // QUEUE_SUBMIT（变长：count 首部）
        push(W::QUEUE_SUBMIT, &cat(&[&le32(1)[..], &le32(3)[..]]));

        GpuStream {
            image_id: GeneratedImageId::from_key("test:backend-dispatch:webgpu-clear"),
            target_id: "fx".into(),
            width: 8,
            height: 8,
            backend: GpuBackend::WebGPU,
            attrs: Default::default(),
            ops: GpuOpFrame {
                ops,
                bytes,
                byte_ranges: ranges,
            },
        }
    }

    /// 纯 WebGPU 帧不要求 GL 会话：wgpu 执行器直接回填 CPU readback。
    #[test]
    fn webgpu_stream_dispatches_to_wgpu_and_backfills_plan() {
        if !gpu_env_available("webgpu_dispatch") {
            return;
        }
        let mut wgpu_executor = crate::gpu::wgpu_executor::WgpuStreamExecutor::new();
        let stream = webgpu_clear_stream();
        let mut plan = FrameMediaPlan::default();
        plan.generated_images
            .push(opencat_core::ir::media_plan::FrameGeneratedImage {
                id: stream.image_id,
                width: stream.width,
                height: stream.height,
                rgba: std::sync::Arc::from(vec![]),
                gpu: None,
            });
        let section = GpuSection { streams: vec![stream] };

        let outputs =
            execute_streams_by_backend(None, None, &mut wgpu_executor, &section, &mut plan, None)
                .expect("webgpu-only frame must execute without a GL session");
        assert!(outputs.is_empty(), "WebGPU has no GL output handles");
        let entry = &plan.generated_images[0];
        assert_eq!(
            entry.gpu, None,
            "CPU readback path must not produce GPU handles"
        );
        assert_eq!(
            entry.rgba.len(),
            (8 * 8 * 4) as usize,
            "placeholder must be backfilled with full-frame RGBA"
        );
        // clear 红（BGRA→RGBA 已在执行端回读转换）。
        for px in entry.rgba.chunks_exact(4) {
            assert_eq!(px, &[255, 0, 0, 255], "every pixel must be the clear red");
        }
    }

    /// WebGL 的同一输出模式由 `Option<&TextureRegistry>` 表达：`None` 回填
    /// CPU 像素，`Some` 发布纹理句柄；消费后输出归还复用池，凭据从表中移除。
    #[test]
    fn webgl_clear_readback_then_publishes_and_releases_texture() {
        if !gpu_env_available("webgl_readback_publish") {
            return;
        }
        let mut session = crate::gpu::session::GlSession::offscreen()
            .expect("offscreen ANGLE session required for this test");
        session
            .make_current()
            .expect("make_current for stream execution");
        let mut gpu_executor = crate::gpu::stream_executor::GpuStreamExecutor::new();
        let mut wgpu_executor = crate::gpu::wgpu_executor::WgpuStreamExecutor::new();

        let mut ops = GpuOpFrame::default();
        let mut clear_color = Vec::with_capacity(4 * std::mem::size_of::<f32>());
        for channel in [1.0f32, 0.0, 0.0, 1.0] {
            clear_color.extend_from_slice(&channel.to_le_bytes());
        }
        ops.push_cmd(G::CLEAR_COLOR, &clear_color);
        ops.push_cmd(G::CLEAR, &[1]);
        let stream = GpuStream {
            image_id: GeneratedImageId::from_key("test:backend-dispatch:webgl-clear"),
            target_id: "fx-gl-clear".into(),
            width: 8,
            height: 8,
            backend: GpuBackend::WebGL2,
            attrs: Default::default(),
            ops,
        };
        let image_id = stream.image_id;
        let width = stream.width;
        let height = stream.height;
        let section = GpuSection {
            streams: vec![stream],
        };
        let mut plan = FrameMediaPlan::default();
        plan.generated_images
            .push(opencat_core::ir::media_plan::FrameGeneratedImage {
                id: image_id,
                width,
                height,
                rgba: std::sync::Arc::from(vec![]),
                gpu: None,
            });

        let outputs = execute_streams_by_backend(
            Some(session.gl()),
            Some(&mut gpu_executor),
            &mut wgpu_executor,
            &section,
            &mut plan,
            None,
        )
        .expect("WebGL clear must execute through CPU readback");
        assert!(outputs.is_empty(), "readback path has no GL output handles");
        {
            let entry = &plan.generated_images[0];
            assert_eq!(entry.gpu, None, "readback path must not publish a token");
            assert_eq!(entry.rgba.len(), (8 * 8 * 4) as usize);
            for px in entry.rgba.chunks_exact(4) {
                assert_eq!(px, &[255, 0, 0, 255], "every pixel must be the clear red");
            }
        }

        let textures = crate::gpu::TextureRegistry::new();
        let outputs = execute_streams_by_backend(
            Some(session.gl()),
            Some(&mut gpu_executor),
            &mut wgpu_executor,
            &section,
            &mut plan,
            Some(&textures),
        )
        .expect("WebGL clear must execute through zero-copy publication");
        let token = {
            let entry = &plan.generated_images[0];
            let handle = entry
                .gpu
                .as_ref()
                .expect("zero-copy path must publish a token");
            let token = crate::gpu::TextureId::from_raw(handle.token);
            assert!(textures.resolve(token).is_some());
            assert!(entry.rgba.is_empty(), "zero-copy path must not retain pixels");
            token
        };
        assert_eq!(outputs.len(), 1, "zero-copy path must return one GL output");

        gpu_executor.recycle(outputs);
        textures.release(token);
        assert!(textures.is_empty(), "texture registry must drain after release");
    }

    /// 混合帧：WebGPU 清红，WebGL 清绿。各流只回填自己的占位。
    #[test]
    fn mixed_frame_executes_both_executors() {
        if !gpu_env_available("mixed_frame") {
            return;
        }
        let mut session = crate::gpu::session::GlSession::offscreen()
            .expect("offscreen ANGLE session required for this test");
        session
            .make_current()
            .expect("make_current for stream execution");
        let mut gpu_executor = crate::gpu::stream_executor::GpuStreamExecutor::new();
        let mut wgpu_executor = crate::gpu::wgpu_executor::WgpuStreamExecutor::new();

        let webgpu = webgpu_clear_stream();
        let mut gl_ops = GpuOpFrame::default();
        let green: Vec<u8> = [0.0f32, 1.0, 0.0, 1.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        gl_ops.push_cmd(G::CLEAR_COLOR, &green);
        gl_ops.push_cmd(G::CLEAR, &[1]);
        let webgl = GpuStream {
            image_id: GeneratedImageId::from_key("test:backend-dispatch:webgl-empty"),
            target_id: "fx-gl".into(),
            width: 8,
            height: 8,
            backend: GpuBackend::WebGL2,
            attrs: Default::default(),
            ops: gl_ops,
        };
        let mut plan = FrameMediaPlan::default();
        for s in [&webgpu, &webgl] {
            plan.generated_images
                .push(opencat_core::ir::media_plan::FrameGeneratedImage {
                    id: s.image_id,
                    width: s.width,
                    height: s.height,
                    rgba: std::sync::Arc::from(vec![]),
                    gpu: None,
                });
        }
        let section = GpuSection {
            streams: vec![webgpu, webgl],
        };

        let outputs = execute_streams_by_backend(
            Some(session.gl()),
            Some(&mut gpu_executor),
            &mut wgpu_executor,
            &section,
            &mut plan,
            None,
        )
        .expect("mixed frame must execute on both executors");

        assert!(outputs.is_empty(), "readback must not publish handles");
        let ids: Vec<_> = section.streams.iter().map(|s| s.image_id).collect();
        let webgpu_entry = plan
            .generated_images
            .iter()
            .find(|e| e.id == ids[0])
            .expect("webgpu entry kept");
        assert_eq!(
            webgpu_entry.rgba.len(),
            (8 * 8 * 4) as usize,
            "webgpu entry must be backfilled by the wgpu executor"
        );
        for px in webgpu_entry.rgba.chunks_exact(4) {
            assert_eq!(
                px,
                &[255, 0, 0, 255],
                "clear red must land on the webgpu entry"
            );
        }
        let gl_entry = plan
            .generated_images
            .iter()
            .find(|e| e.id == ids[1])
            .expect("gl entry kept");
        assert!(
            gl_entry.rgba.is_empty() || gl_entry.rgba[0] != 255,
            "GL entry must not receive the wgpu clear red"
        );
    }
}
