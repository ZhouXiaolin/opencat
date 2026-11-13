use std::path::Path;

use anyhow::{Result, anyhow};
use skia_safe::{AlphaType, ColorType, ImageInfo, image::CachingHint, surfaces};

pub mod executor;
pub mod frame;

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
/// CPU 合成路径的 GPU 流执行底座（懒建，线程本地——GL 上下文非线程安全）。
thread_local! {
    static CPU_GL: std::cell::RefCell<
        Option<(crate::gpu::session::GlSession, crate::gpu::stream_executor::GpuStreamExecutor)>,
    > = const { std::cell::RefCell::new(None) };
}

/// 渲染收尾：显式拆掉本线程的常驻 GL 会话（CPU 合成流执行 + GLSL 反射器）。
/// 不能留给 TLS 析构——进程退出时其他线程已被杀，SwiftShader 的 eglTerminate
/// 在 TLS 析构里等它们收尾会永久阻塞。
pub fn release_gl_sessions() {
    let _ = CPU_GL.try_with(|cell| cell.borrow_mut().take());
    crate::script::bridge::release_reflector_session();
}

/// CPU 合成路径的 GPU 流执行：readback 喂占位条目（无零拷贝）。
fn cpu_execute_gpu_streams(
    textures: &std::sync::Arc<crate::gpu::TextureRegistry>,
    media_plan: &mut opencat_core::ir::media_plan::FrameMediaPlan,
    section: &opencat_core::ir::GpuSection,
) -> Result<()> {
    CPU_GL.with(|cell| -> Result<()> {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some((
                crate::gpu::session::GlSession::offscreen()?,
                crate::gpu::stream_executor::GpuStreamExecutor::new(),
            ));
        }
        let (session, executor) = slot.as_mut().unwrap();
        session.make_current()?;
        executor.execute(&session.gl(), section, media_plan, textures, false)?;
        Ok(())
    })
}

/// 调试用：dump 原始 GPU 流（诊断两链命令分歧）。全片体量过大：默认只落
/// 前 2 帧；`OC_DUMP_GPU_STREAM_ALL=1` 全量。
fn dump_gpu_streams(dir: &std::path::Path, gpu: &opencat_core::ir::GpuSection, frame_index: u32) {
    if frame_index >= 2 && std::env::var_os("OC_DUMP_GPU_STREAM_ALL").is_none() {
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
        let _ = std::fs::write(
            dir.join(format!("gpu-frame{frame_index}-s{si}.bin")),
            &buf,
        );
    }
}

fn render_pipeline_frame_to_rgba(
    pipeline: &mut crate::EnginePipeline,
    media_ctx: &mut MediaContext,
    executor: &mut crate::render::executor::EngineDrawExecutor,
    generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
    surface_w: u32,
    surface_h: u32,
    frame_index: u32,
) -> Result<Vec<u8>> {
    let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
    let t0 = timing.then(std::time::Instant::now);
    let render = pipeline.render_frame(frame_index)?;
    let t_script = t0.map(|t| t.elapsed());
    let mut frame = render.draw;
    let mut media_plan = render.media;
    if let Some(gpu) = &render.gpu {
        if let Some(dir) = std::env::var_os("OC_DUMP_GPU_STREAM") {
            dump_gpu_streams(std::path::Path::new(&dir), gpu, frame_index);
        }
        cpu_execute_gpu_streams(&pipeline.textures, &mut media_plan, gpu)?;
    }
    let t_gpu = t0.map(|t| t.elapsed());

    let mut surface = surfaces::raster_n32_premul((surface_w as i32, surface_h as i32))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;

    // SAFETY: skia_safe::Canvas wraps a C++ ref-counted object with interior mutability.
    // All draw methods take &self at the Rust level while mutating internal C++ state.
    // The surface owns the canvas and no other references exist at this point.
    #[allow(invalid_reference_casting)]
    let canvas: &mut skia_safe::Canvas =
        unsafe { &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas) };

    crate::render::frame::execute_render_frame(
        &mut frame,
        &media_plan,
        pipeline.loader(),
        crate::render::frame::FrameExec {
            executor,
            media_ctx,
            generated_cache,
        },
        canvas,
    )?;
    let t_draw = t0.map(|t| t.elapsed());

    let image = surface.image_snapshot();
    let image_info = ImageInfo::new(
        (surface_w as i32, surface_h as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut rgba = vec![0u8; (surface_w as usize) * (surface_h as usize) * 4];
    let read_ok = image.read_pixels(
        &image_info,
        rgba.as_mut_slice(),
        surface_w as usize * 4,
        (0, 0),
        CachingHint::Allow,
    );
    if !read_ok {
        return Err(anyhow!("failed to read pixels from skia surface"));
    }
    if let Some(t0) = t0 {
        let t_end = t0.elapsed();
        eprintln!(
            "[timing-cpu] f{frame_index} script={:.1}ms gpu-stream={:.1}ms draw={:.1}ms readback={:.1}ms total={:.1}ms",
            t_script.unwrap_or_default().as_secs_f64() * 1e3,
            (t_gpu.unwrap_or_default() - t_script.unwrap_or_default()).as_secs_f64() * 1e3,
            (t_draw.unwrap_or_default() - t_gpu.unwrap_or_default()).as_secs_f64() * 1e3,
            (t_end - t_draw.unwrap_or_default()).as_secs_f64() * 1e3,
            t_end.as_secs_f64() * 1e3,
        );
    }
    Ok(rgba)
}

/// GPU 合成路径的会话状态（Linux=EGL 的桌面 GL；Windows 三栈统一后无 GL
/// Skia 符号，走 [`VkRenderSession`]）。
///
/// 持有离屏 GL 会话与 Skia GPU 合成器。**线程纪律**：GL 上下文非线程安全——
/// 会话在线程 A 建立就在线程 A 使用（离线渲染的建与用同线程），每帧前
/// `make_current` 一次；无锁。
#[cfg(target_os = "linux")]
pub struct GpuRenderSession {
    compositor: crate::gpu::compositor::GpuCompositor,
    /// 合成目标纹理：drop 早于 `gl_session`（字段顺序即 drop 顺序）。
    composite_tex: CompositeTexture,
    gl_session: crate::gpu::session::GlSession,
    /// GPU 命令流执行器（内容缓存跨帧复用）。
    gpu_executor: crate::gpu::stream_executor::GpuStreamExecutor,
    /// WebGPU 流执行器（wgpu 一对一映射；独立设备，与 GL 上下文零共享）。
    wgpu_executor: crate::gpu::wgpu_executor::WgpuStreamExecutor,
    /// 共享纹理登记处（GPU 流输出纹理的取用与归还）。
    textures: std::sync::Arc<crate::gpu::TextureRegistry>,
}

/// 合成目标纹理的 RAII（GL 纹理名的删除需要 current 上下文，所以先于
/// [`GpuRenderSession::gl_session`] drop——字段声明顺序即 drop 顺序）。
#[cfg(target_os = "linux")]
struct CompositeTexture {
    gl: std::sync::Arc<glow::Context>,
    tex: crate::gpu::NativeTexture,
}

#[cfg(target_os = "linux")]
impl Drop for CompositeTexture {
    fn drop(&mut self) {
        crate::gpu::gl::delete_texture(&self.gl, self.tex);
    }
}

#[cfg(target_os = "linux")]
impl GpuRenderSession {
    /// 建一个 `width × height` 的 GPU 合成会话。
    pub fn new(width: u32, height: u32) -> Result<Self> {
        // 离屏 GL 会话：构造即 make current。
        let gl_session = crate::gpu::session::GlSession::offscreen()?;
        let gl = gl_session.gl();
        // 设备指纹（并行渲染排障用）：确认 ANGLE 后端实际落在哪个渲染器上。
        if std::env::var_os("OC_TRACE_GL_INFO").is_some()
            && let Some((vendor, renderer, version)) = crate::gpu::angle::gl_info()
        {
            eprintln!("[glinfo] vendor={vendor} renderer={renderer} version={version}");
        }
        // 合成目标纹理：RGBA8 + FBO，Skia 的 wrap_backend_render_target 用。
        let native = crate::gpu::gl::create_texture_rgba8(&gl, width, height)
            .ok_or_else(|| anyhow!("创建合成目标纹理失败"))?;
        let composite_tex = CompositeTexture { gl: gl.clone(), tex: native };

        // Skia 合成器（共享同一 GL 上下文；入口解析跟会话同源）。
        let compositor = crate::gpu::compositor::GpuCompositor::new(
            crate::gpu::compositor::CompositorBackend::Gl {
                loader: gl_session.skia_loader(),
            },
            Some(gl.clone()),
            native,
            width,
            height,
        )?;

        Ok(Self {
            compositor,
            composite_tex,
            gl_session,
            gpu_executor: crate::gpu::stream_executor::GpuStreamExecutor::new(),
            wgpu_executor: crate::gpu::wgpu_executor::WgpuStreamExecutor::new(),
            textures: std::sync::Arc::new(crate::gpu::TextureRegistry::new()),
        })
    }

    /// 渲染一帧到 GPU 合成目标并读回 RGBA。
    ///
    /// 读回走 PBO 双缓冲（[`GpuCompositor::finish_frame`]），所以返回的是
    /// **上一帧**的像素；首帧返回 `None`，末帧由 `encode_rgba_frames` 多渲染
    /// 一帧排空。要立刻拿到本帧，用 [`Self::render_frame_blocking`]。
    pub fn render_frame(
        &mut self,
        pipeline: &mut crate::EnginePipeline,
        media_ctx: &mut MediaContext,
        executor: &mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
        frame_index: u32,
    ) -> Result<Option<Vec<u8>>> {
        self.render_frame_inner(
            pipeline,
            media_ctx,
            executor,
            generated_cache,
            frame_index,
            true,
        )
    }

    /// 排空本会话在途的末帧：同步读回 surface 现有内容（不再画新东西）。
    /// 并行管线的 worker 收场用——异步 PBO 的最后一帧没有「下一帧」来
    /// 发起读回，由这里直接同步取走。调用方须在其 GL 线程上。
    pub fn flush_blocking(&mut self) -> Result<Vec<u8>> {
        self.gl_session.make_current()?;
        self.compositor.finish_frame_blocking()
    }

    /// 渲染一帧并**同步**读回本帧像素（无延迟）。
    ///
    /// 单帧 / PNG 序列这类一次一帧的调用方用它：PBO 的一帧延迟会打乱它们
    /// 的输出。代价是读回留在关键路径上，逐帧循环请改用
    /// [`Self::render_frame`]。
    pub fn render_frame_blocking(
        &mut self,
        pipeline: &mut crate::EnginePipeline,
        media_ctx: &mut MediaContext,
        executor: &mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
        frame_index: u32,
    ) -> Result<Vec<u8>> {
        self.render_frame_inner(
            pipeline,
            media_ctx,
            executor,
            generated_cache,
            frame_index,
            false,
        )
        .map(|frame| frame.expect("同步读回必定产出本帧"))
    }

    fn render_frame_inner(
        &mut self,
        pipeline: &mut crate::EnginePipeline,
        media_ctx: &mut MediaContext,
        executor: &mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
        frame_index: u32,
        deferred: bool,
    ) -> Result<Option<Vec<u8>>> {
        let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
        let t0 = timing.then(std::time::Instant::now);
        let frame_wall = std::time::Instant::now();
        self.gl_session.make_current()?;
        // 前端（core 顺序产帧）：script/layout/display + op 流与媒体计划。
        let render = pipeline.render_frame(frame_index)?;
        let t_script = t0.map(|t| t.elapsed());
        let loader = pipeline.loader();
        let out = self.execute_frame(
            render,
            loader,
            media_ctx,
            executor,
            generated_cache,
            frame_index,
            deferred,
            t0,
            t_script,
        )?;
        if std::env::var_os("OC_TRACE_FRAME_TIME").is_some() {
            eprintln!("[frametime] f{frame_index} wall={}ms", frame_wall.elapsed().as_millis());
        }
        Ok(out)
    }

    /// 后端：执行一帧**已产出**的 [`RenderFrame`]（GL/wgpu 命令流 + Skia
    /// 合成 + 读回）。与前端（`pipeline.render_frame`，顺序依赖的 JS 状态）
    /// 分离，帧是纯数据 op 流。
    ///
    /// `t0` / `t_script` 是前端传入的计时基准（`OC_FRAME_TIMING`
    /// 的 `[timing-gpu]` 分段）。
    ///
    /// 返回 `None` 还在 `deferred=true`（PBO 双缓冲交上一帧）时首帧发生。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[allow(clippy::too_many_arguments)]
    pub fn execute_frame(
        &mut self,
        render: opencat_core::ir::RenderFrame,
        loader: &crate::resource::loader::EngineLoader,
        media_ctx: &mut MediaContext,
        executor: &mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
        frame_index: u32,
        deferred: bool,
        t0: Option<std::time::Instant>,
        t_script: Option<std::time::Duration>,
    ) -> Result<Option<Vec<u8>>> {
        // 脚本执行期会把 EGL current context 切走（GLSL 反射器建自己的离屏
        // 上下文；`script::bridge::CurrentContextGuard` 在 Windows 上仍走 WGL
        // 存取，ANGLE 迁移后存不到也还不回）。流的执行与 Skia 合成必须在**本
        // 会话**的上下文里：否则纹理/FBO 名字与合成目标撞号，合成目标变成
        // INCOMPLETE_ATTACHMENT，绘制被静默丢弃（表现为随机尺寸的裁剪或全黑）。
        self.gl_session.make_current()?;
        let mut frame = render.draw;
        let mut media_plan = render.media;
        // 调试用:dump 原始 GPU 流(测试进程加载后离线复现执行器+flush 崩溃)。
        if let Some(dir) = std::env::var_os("OC_DUMP_GPU_STREAM")
            && let Some(gpu) = &render.gpu
        {
            dump_gpu_streams(std::path::Path::new(&dir), gpu, frame_index);
        }
        if std::env::var_os("OC_TRACE_STREAMS").is_some() {
            match &render.gpu {
                Some(gpu) => {
                    eprintln!("[streams] f{frame_index} n={}", gpu.streams.len());
                    for s in &gpu.streams {
                        eprintln!(
                            "[streams]   {:?} {}x{} backend={:?}",
                            s.target_id,
                            s.width,
                            s.height,
                            s.backend
                        );
                    }
                }
                None => eprintln!("[streams] f{frame_index} none"),
            }
        }
        let mut stream_outputs: Vec<crate::gpu::stream_executor::StreamOutput> = Vec::new();
        // 流执行过 = GL 上下文被改过（哪怕占位不可见、输出被丢弃——three
        // 这类持久 renderer 的不可见帧照常执行命令），Skia 合成前就必须
        // reset。判据不是 gpu_handles：输出被丢弃的帧 handle 为空。
        let mut gpu_stream_ran = false;
        let stream_wall = std::time::Instant::now();
        if std::env::var_os("OC_DISABLE_GPU_STREAM").is_none()
            && let Some(gpu) = &render.gpu
        {
            // backend 分流：WebGPU 流进 wgpu 执行器（独立设备，与 GL 零共享
            // 状态）；WebGL2 流走 glow 执行器。语料一帧只有一种 backend
            //（一个 canvas 一个 renderer），混合帧为防御性支持。
            let has_webgpu = gpu
                .streams
                .iter()
                .any(|s| s.backend == opencat_core::ir::GpuBackend::WebGPU);
            let has_gl = gpu
                .streams
                .iter()
                .any(|s| s.backend == opencat_core::ir::GpuBackend::WebGL2);
            if has_webgpu {
                let webgpu_streams: Vec<&opencat_core::ir::gpu_stream::GpuStream> = gpu
                    .streams
                    .iter()
                    .filter(|s| s.backend == opencat_core::ir::GpuBackend::WebGPU)
                    .collect();
                self.wgpu_executor.execute(&webgpu_streams, &mut media_plan)?;
            }
            if has_gl {
                gpu_stream_ran = true;
                if has_webgpu {
                    let gl_section = opencat_core::ir::gpu_stream::GpuSection {
                        streams: gpu
                            .streams
                            .iter()
                            .filter(|s| s.backend == opencat_core::ir::GpuBackend::WebGL2)
                            .cloned()
                            .collect(),
                    };
                    stream_outputs = self.gpu_executor.execute(
                        &self.gl_session.gl(),
                        &gl_section,
                        &mut media_plan,
                        &self.textures,
                        true,
                    )?;
                } else {
                    let filtered;
                    let section: &opencat_core::ir::gpu_stream::GpuSection = match std::env::var("OC_ONLY_STREAM") {
                        Ok(only) => {
                            filtered = opencat_core::ir::gpu_stream::GpuSection {
                                streams: gpu
                                    .streams
                                    .iter()
                                    .filter(|s| s.target_id == only)
                                    .cloned()
                                    .collect(),
                            };
                            &filtered
                        }
                        Err(_) => gpu,
                    };
                    stream_outputs = self.gpu_executor.execute(
                        &self.gl_session.gl(),
                        section,
                        &mut media_plan,
                        &self.textures,
                        true,
                    )?;
                }
            }
        }

        // 收集本帧引用的 GPU 纹理凭据：合成后统一归还。
        let t_gl = t0.map(|t| t.elapsed());
        let gpu_handles: Vec<crate::gpu::TextureId> = media_plan
            .generated_images
            .iter()
            .filter_map(|g| g.gpu.map(|h| crate::gpu::TextureId::from_raw(h.token)))
            .collect();

        let compositor = &mut self.compositor;
        let textures = &self.textures;

        // 本帧有 GPU 流执行过 pass（命令流改动了 GL 状态），Skia 缓存的状态
        // 已失效：两者共享同一 GL 上下文，合成前必须让 Skia 重新查询。
        if std::env::var_os("OC_TRACE_GLSTATE").is_some() {
            use glow::HasContext as _;
            let gl = self.gl_session.gl();
            unsafe {
                let caps = [
                    ("DEPTH_TEST", glow::DEPTH_TEST),
                    ("STENCIL_TEST", glow::STENCIL_TEST),
                    ("BLEND", glow::BLEND),
                    ("CULL_FACE", glow::CULL_FACE),
                    ("SCISSOR_TEST", glow::SCISSOR_TEST),
                    ("RASTERIZER_DISCARD", glow::RASTERIZER_DISCARD),
                    ("FRAMEBUFFER_SRGB", glow::FRAMEBUFFER_SRGB),
                    ("POLYGON_OFFSET_FILL", glow::POLYGON_OFFSET_FILL),
                    ("SAMPLE_ALPHA_TO_COVERAGE", glow::SAMPLE_ALPHA_TO_COVERAGE),
                    ("SAMPLE_COVERAGE", glow::SAMPLE_COVERAGE),
                ];
                let mut on = Vec::new();
                for (n, c) in caps {
                    if gl.is_enabled(c) { on.push(n); }
                }
                let mut vp = [0i32; 4];
                gl.get_parameter_i32_slice(glow::VIEWPORT, &mut vp);
                let mut sc = [0i32; 4];
                gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut sc);
                let fbo = gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING);
                let prog = gl.get_parameter_i32(glow::CURRENT_PROGRAM);
                let err = gl.get_error();
                if let Some(nz) = std::num::NonZeroU32::new(compositor.fbo()) {
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(glow::NativeFramebuffer(nz)));
                }
                let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                eprintln!("[glstate] f{frame_index} on={on:?} viewport={vp:?} scissor={sc:?} fbo={fbo} prog={prog} err={err:?} compFbo={} status={status:?}", compositor.fbo());
            }
        }
        if gpu_stream_ran {
            compositor.reset_backend_state();
                    use glow::HasContext as _;
                let gl = self.gl_session.gl();
                unsafe {
                    gl.disable(glow::SCISSOR_TEST);
                    gl.disable(glow::RASTERIZER_DISCARD);
                    gl.disable(glow::DEPTH_TEST);
                    gl.disable(glow::STENCIL_TEST);
                    gl.disable(glow::BLEND);
                    gl.disable(glow::CULL_FACE);
                    gl.disable(glow::DITHER);
                    gl.disable(glow::POLYGON_OFFSET_FILL);
                    gl.disable(glow::SAMPLE_ALPHA_TO_COVERAGE);
                    gl.disable(glow::SAMPLE_COVERAGE);
                    gl.bind_vertex_array(None);
                    gl.bind_buffer(glow::ARRAY_BUFFER, None);
                    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
                    gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                    gl.bind_renderbuffer(glow::RENDERBUFFER, None);
                    gl.use_program(None);
                    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
                    gl.pixel_store_i32(glow::PACK_ALIGNMENT, 4);
                    gl.depth_mask(true);
                    gl.stencil_mask(0xFFFFFFFF);
                    gl.color_mask(true, true, true, true);
                    for u in 0..8 {
                        gl.active_texture(glow::TEXTURE0 + u);
                        gl.bind_texture(glow::TEXTURE_2D, None);
                        gl.bind_texture(glow::TEXTURE_CUBE_MAP, None);
                        gl.bind_sampler(u, None);
                    }
                    gl.active_texture(glow::TEXTURE0);
                    gl.blend_equation(glow::FUNC_ADD);
                    gl.blend_func(glow::ONE, glow::ZERO);
                    gl.viewport(0, 0, compositor.width() as i32, compositor.height() as i32);
                    gl.color_mask(true, true, true, true);
            }
        }

        {
            let (canvas, skia) = compositor.split();
            // 帧间清屏：合成目标纹理跨帧持久（GpuRenderSession::new 建一次），
            // 而浏览器语义是每帧在空白画布上重绘——没有全帧不透明背景的合成
            // （页面根声明 `background: transparent`，或半透明淡出层）会把上
            // 一帧残影透进当前帧：liquid 整体淡出段 lockup 永不消失、探针里
            // 半透明矩形叠出「旧色×(1-α) + 新色×α」的根因。CPU 路径每帧新建
            // surface（零初始化=透明），这里对齐同一语义。
            canvas.clear(skia_safe::Color::TRANSPARENT);
            let mut resolver = crate::gpu::compositor::CompositorResolver { skia, textures };
            if std::env::var_os("OC_DEBUG_PRERECT").is_some() {
                let paint = skia_safe::Paint::new(skia_safe::Color4f::new(0.0, 0.0, 1.0, 1.0), None);
                canvas.draw_rect(skia_safe::Rect::new(0.0, 0.0, 1920.0, 1080.0), &paint);
            }
            if std::env::var_os("OC_DEBUG_SKIP_FRAME").is_none() {
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
            }
            if std::env::var_os("OC_DEBUG_FULLRECT").is_some() {
                let paint = skia_safe::Paint::new(skia_safe::Color4f::new(1.0, 0.0, 0.0, 1.0), None);
                canvas.draw_rect(
                    skia_safe::Rect::new(0.0, 0.0, 1920.0, 1080.0),
                    &paint,
                );
            }
            if std::env::var_os("OC_TRACE_CANVAS").is_some() {
                eprintln!(
                    "[canvas] clip={:?} matrix={:?} saves={}",
                    canvas.local_clip_bounds(),
                    canvas.total_matrix(),
                    canvas.save_count()
                );
            }
        }
        if std::env::var_os("OC_TRACE_DEFAULT_FB").is_some() {
            use glow::HasContext as _;
            let gl = self.gl_session.gl();
            unsafe {
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);
                let mut px = [0u8; 4];
                gl.read_pixels(0, 0, 1, 1, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut px[..]));
                let mut px2 = [0u8; 4];
                gl.read_pixels(960, 540, 1, 1, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut px2[..]));
                let max_tex = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE);
                let max_rb = gl.get_parameter_i32(glow::MAX_RENDERBUFFER_SIZE);
                let err = gl.get_error();
                eprintln!("[defaultfb] px00={px:?} pxcenter={px2:?} maxTex={max_tex} maxRb={max_rb} err={err:?}");
            }
        }
        let rgba = if deferred {
            let r = compositor.finish_frame()?;
            r
        } else {
            let r = compositor.finish_frame_blocking().map(Some)?;
            r
        };
        if let Some(t0) = t0 {
            let t_end = t0.elapsed();
            let t_script = t_script.unwrap_or_default();
            let t_gl = t_gl.unwrap_or_default();
            eprintln!(
                "[timing-gpu] f{frame_index} script={:.1}ms gl-stream={:.1}ms compose+readback={:.1}ms total={:.1}ms",
                t_script.as_secs_f64() * 1e3,
                (t_gl - t_script).as_secs_f64() * 1e3,
                (t_end - t_gl).as_secs_f64() * 1e3,
                t_end.as_secs_f64() * 1e3,
            );
        }
        if std::env::var_os("OC_TRACE_FRAME_TIME").is_some() {
            eprintln!(
                "[frametime] f{frame_index} exec_total={}ms",
                stream_wall.elapsed().as_millis(),
            );
        }

        // 合成完成：像素已被 Skia 采样进合成帧，输出归还复用池（免逐帧
        // 建销 1080p 纹理+深度缓冲），凭据归还。KEEP_STREAM_TEX 是调试
        // 语义（纹理保留不回收），outputs 直接丢弃。
        if std::env::var_os("OC_KEEP_STREAM_TEX").is_none() {
            self.gpu_executor.recycle(stream_outputs);
        }
        for id in gpu_handles {
            textures.release(id);
        }
        Ok(rgba)
    }
}

/// Vulkan 渲染会话：`OC_ANGLE_SWIFTSHADER` 走独立 VkSession 与 GL 流 CPU
/// 回读；否则 Skia 与 ANGLE 共用 VkDevice，ANGLE 的 WebGL2 流输出纹理经
/// VkImage 直接采样。最终合成帧仍同步读回 RGBA 给编码器。
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct VkRenderSession {
    compositor: crate::gpu::compositor::GpuCompositor,
    /// Vulkan 会话：零拷贝时建在 ANGLE 的 device/queue 上，否则独立。
    session: std::sync::Arc<crate::gpu::vk::VkSession>,
    /// GL 会话（ANGLE Vulkan）：WebGL2 流的执行端。
    gl_session: Option<crate::gpu::session::GlSession>,
    gpu_executor: crate::gpu::stream_executor::GpuStreamExecutor,
    wgpu_executor: crate::gpu::wgpu_executor::WgpuStreamExecutor,
    textures: std::sync::Arc<crate::gpu::TextureRegistry>,
    /// 本帧 GL 流的输出，合成完成后归还复用池。
    stream_outputs: Vec<crate::gpu::stream_executor::StreamOutput>,
    /// ANGLE 的队列族索引（导入纹理的 `current_queue_family`）。
    queue_family: u32,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl VkRenderSession {
    /// 建 Vulkan 渲染会话。
    ///
    /// 调用方须先设好 `VK_ICD_FILENAMES`——它是进程级的，Skia / ANGLE /
    /// wgpu 三家必须看到同一个 ICD。
    pub fn new(width: u32, height: u32) -> Result<Self> {
        // 后端选择：SwiftShader 当前走独立 VkSession + GL 流 CPU 回读；
        // llvmpipeline 走共享 VkDevice + 纹理零拷贝。
        let zero_copy = std::env::var_os("OC_ANGLE_SWIFTSHADER").is_none();
        let (session, gl_session, queue_family) = if zero_copy {
            let gl_session = crate::gpu::session::GlSession::offscreen()?;
            let angle = gl_session.angle_context().ok_or_else(|| {
                anyhow!(
                    "OC_SKIA_BACKEND=vulkan 需要 ANGLE（OC_ANGLE_DIR）：\
                     原生 EGL 拿不到 Vulkan 句柄，零拷贝不成立"
                )
            })?;
            let handles = angle.vulkan_handles().ok_or_else(|| {
                anyhow!("ANGLE 未交出 Vulkan 句柄（EGL_ANGLE_device_vulkan 缺失或查询失败）")
            })?;
            let queue_family = handles.queue_family_index;
            let session = std::sync::Arc::new(crate::gpu::vk::VkSession::from_angle(&handles)?);
            (session, Some(gl_session), queue_family)
        } else {
            (
                std::sync::Arc::new(crate::gpu::vk::VkSession::new()?),
                None,
                0,
            )
        };
        let compositor = crate::gpu::compositor::GpuCompositor::new(
            crate::gpu::compositor::CompositorBackend::Vulkan {
                session: session.clone(),
            },
            None,
            crate::gpu::NativeTexture::NONE,
            width,
            height,
        )?;
        Ok(Self {
            compositor,
            session,
            gl_session,
            gpu_executor: crate::gpu::stream_executor::GpuStreamExecutor::new(),
            wgpu_executor: crate::gpu::wgpu_executor::WgpuStreamExecutor::new(),
            textures: std::sync::Arc::new(crate::gpu::TextureRegistry::new()),
            stream_outputs: Vec::new(),
            queue_family,
        })
    }

    /// 渲染一帧并读回 RGBA（单帧/PNG/预热路径的整帧薄壳）。
    pub fn render_frame(
        &mut self,
        pipeline: &mut crate::EnginePipeline,
        media_ctx: &mut MediaContext,
        executor: &mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
        frame_index: u32,
    ) -> Result<Vec<u8>> {
        let render = pipeline.render_frame(frame_index)?;
        self.execute_render(
            render,
            executor,
            generated_cache,
            media_ctx,
            &pipeline.textures,
            pipeline.loader_arc(),
            frame_index,
        )
    }

    /// 执行阶段（调用方线程 = ANGLE/Skia 上下文所在线程）：GL 流执行 +
    /// Skia 合成 + 同步读回。帧是纯数据（[`opencat_core::ir::RenderFrame`]），
    /// 与 script 阶段经 flume 通道分线程衔接。`media_ctx`（视频解码/图像
    /// 光栅缓存）只属于本阶段；loader/textures 是 open 后只读的 Arc 共享。
    #[allow(clippy::too_many_arguments)]
    pub fn execute_render(
        &mut self,
        render: opencat_core::ir::RenderFrame,
        executor: &mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &mut BoundedLruCache<GeneratedImageId, Image>,
        media_ctx: &mut MediaContext,
        pipeline_textures: &std::sync::Arc<crate::gpu::TextureRegistry>,
        loader: &std::sync::Arc<crate::resource::loader::EngineLoader>,
        frame_index: u32,
    ) -> Result<Vec<u8>> {
        let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
        let t0 = timing.then(std::time::Instant::now);
        if let Some(gl) = &self.gl_session {
            gl.make_current()?;
        }
        let t_script: Option<std::time::Duration> = None;
        let mut frame = render.draw;
        let mut media_plan = render.media;
        if let Some(gpu) = render.gpu.as_ref() {
            if let Some(dir) = std::env::var_os("OC_DUMP_GPU_STREAM") {
                dump_gpu_streams(std::path::Path::new(&dir), gpu, frame_index);
            }
            if std::env::var_os("OC_TRACE_STREAMS").is_some() {
                eprintln!("[streams] f{frame_index} n={}", gpu.streams.len());
                for s in &gpu.streams {
                    eprintln!(
                        "[streams]   {:?} {}x{} backend={:?}",
                        s.target_id, s.width, s.height, s.backend
                    );
                }
            }
        }
        if let Some(gl) = &self.gl_session {
            // 流的执行必须在 ANGLE 上下文 current 时进行（produce 阶段可能
            // 切走；跨线程后 current 本就归本线程，不跨线程 make/还）。
            gl.make_current()?;
            if let Some(gpu) = &render.gpu {
                let has_webgpu = gpu
                    .streams
                    .iter()
                    .any(|s| s.backend == opencat_core::ir::GpuBackend::WebGPU);
                let has_gl = gpu
                    .streams
                    .iter()
                    .any(|s| s.backend == opencat_core::ir::GpuBackend::WebGL2);
                if has_webgpu {
                    let webgpu_streams: Vec<&opencat_core::ir::gpu_stream::GpuStream> = gpu
                        .streams
                        .iter()
                        .filter(|s| s.backend == opencat_core::ir::GpuBackend::WebGPU)
                        .collect();
                    self.wgpu_executor.execute(&webgpu_streams, &mut media_plan)?;
                }
                if has_gl {
                    let gl_streams: Vec<opencat_core::ir::gpu_stream::GpuStream> = gpu
                        .streams
                        .iter()
                        .filter(|s| s.backend == opencat_core::ir::GpuBackend::WebGL2)
                        .cloned()
                        .collect();
                    self.stream_outputs = self.gpu_executor.execute(
                        &gl.gl(),
                        &opencat_core::ir::gpu_stream::GpuSection { streams: gl_streams },
                        &mut media_plan,
                        &self.textures,
                        true,
                    )?;
                    // 共享 VkQueue 没有跨 API 隐式顺序。ANGLE 的命令完成
                    // 前 Skia 不能采样导出纹理；finish 建立这个同步点。
                    use glow::HasContext as _;
                    unsafe {
                        gl.gl().finish();
                    }
                }
            }
        } else if let Some(gpu) = &render.gpu {
            // 零拷贝关闭：GL 流由 ANGLE 执行后立刻回读位图（过渡态）。
            crate::render::cpu_execute_gpu_streams(pipeline_textures, &mut media_plan, gpu)?;
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
        let rgba = self.compositor.finish_frame_blocking()?;

        // 合成完成（像素已进 Vulkan 渲染目标并读回）：输出归还复用池，归还凭据。
        if let Some(gl) = &self.gl_session {
            let outputs = std::mem::take(&mut self.stream_outputs);
            self.gpu_executor.recycle(outputs);
            let _ = gl;
        } else {
            debug_assert!(self.stream_outputs.is_empty());
        }
        for id in gpu_handles {
            self.textures.release(id);
        }
        if let Some(t0) = t0 {
            let t_end = t0.elapsed();
            eprintln!(
                "[timing-vk] f{frame_index} script={:.1}ms gl-stream={:.1}ms draw={:.1}ms readback={:.1}ms total={:.1}ms",
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

/// 逐帧渲染器：CPU 光栅，或（桌面 GL 平台）GPU 合成。
///
/// 选择只发生在会话建立时，两种路径逐帧产出同样的 RGBA 尺寸与 alpha 约定
/// （premul），所以编码分支无需分叉。协作者（pipeline / media_ctx /
/// executor / generated_cache）构造时借入，渲染调用只传帧号——调用方不再
/// 逐帧搬运四个 `&mut`；PBO 末帧排空的钳位语义也下沉在这里。
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
struct FrameRenderer<'a> {
    backend: FrameBackend,
    pipeline: &'a mut crate::EnginePipeline,
    media_ctx: &'a mut MediaContext,
    executor: &'a mut crate::render::executor::EngineDrawExecutor,
    generated_cache: &'a mut GeneratedImageCache,
    frame_count: u32,
    width: u32,
    height: u32,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
enum FrameBackend {
    Cpu,
    #[cfg(target_os = "linux")]
    Gpu(Box<GpuRenderSession>),
    /// Vulkan 路径：Skia 的光栅走 Vulkan backend（Windows=硬件/ANGLE 共享
    /// device；Linux=SwiftShader/lvp）。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Vk(Box<VkRenderSession>),
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
impl<'a> FrameRenderer<'a> {
    fn new(
        pipeline: &'a mut crate::EnginePipeline,
        media_ctx: &'a mut MediaContext,
        executor: &'a mut crate::render::executor::EngineDrawExecutor,
        generated_cache: &'a mut GeneratedImageCache,
        frame_count: u32,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        // `OC_SKIA_BACKEND=vulkan` 切到 Vulkan 路径（Skia 的光栅走
        // Vulkan backend）。**必须先于 GpuRenderSession 分支判别**：后者
        // 会建自己的 ANGLE 会话，若随后被 Vk 分支遮蔽丢弃，其 Drop 调
        // eglTerminate 终止共享 display——零拷贝路径的 make_current 从此
        // 拿 EGL_NOT_INITIALIZED，ANGLE 的 VkDevice 连带销毁，Skia 的
        // VMA 析构撞死 device（lvp 实测双症状同源）。
        #[cfg(target_os = "linux")]
        let backend = if std::env::var("OC_SKIA_BACKEND").as_deref() == Ok("vulkan")
            && crate::gpu::requested()
        {
            FrameBackend::Vk(Box::new(VkRenderSession::new(width, height)?))
        } else if crate::gpu::requested() {
            FrameBackend::Gpu(Box::new(GpuRenderSession::new(width, height)?))
        } else {
            FrameBackend::Cpu
        };
        #[cfg(target_os = "windows")]
        let backend = if crate::gpu::requested() {
            FrameBackend::Vk(Box::new(VkRenderSession::new(width, height)?))
        } else {
            FrameBackend::Cpu
        };
        #[cfg(target_os = "macos")]
        let backend = FrameBackend::Cpu;
        Ok(Self {
            backend,
            pipeline,
            media_ctx,
            executor,
            generated_cache,
            frame_count,
            width,
            height,
        })
    }

    /// 编码轨一帧。GPU 路径经 PBO 双缓冲读回，交出的是**上一帧**的像素：
    /// 首帧返回 `None`，`encode_rgba_frames` 多渲染一帧补齐。多出来的那一帧
    /// 索引越界：这里钳回末帧，核心把 `frame_index` 钳到末帧，重复渲染一次
    /// 末帧，只为把在途的末帧读回排空。CPU 路径恒 `Some`。
    fn render_for_encode(&mut self, encode_index: u32) -> Result<Option<Vec<u8>>> {
        let frame_index = encode_index.min(self.frame_count.saturating_sub(1));
        match &mut self.backend {
            FrameBackend::Cpu => render_pipeline_frame_to_rgba(
                self.pipeline,
                self.media_ctx,
                self.executor,
                self.generated_cache,
                self.width,
                self.height,
                frame_index,
            )
            .map(Some),
            #[cfg(target_os = "linux")]
            FrameBackend::Gpu(session) => session.render_frame(
                self.pipeline,
                self.media_ctx,
                self.executor,
                self.generated_cache,
                frame_index,
            ),
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            FrameBackend::Vk(session) => session
                .render_frame(
                    self.pipeline,
                    self.media_ctx,
                    self.executor,
                    self.generated_cache,
                    frame_index,
                )
                .map(Some),
        }
    }

    /// 渲染一帧并**同步**取回本帧像素（无延迟）。单帧 / PNG 序列这类一次
    /// 一帧的调用方用它——PBO 的一帧延迟会打乱它们的输出。逐帧编码循环请
    /// 改用 [`Self::render_for_encode`]。
    fn render_frame_blocking(&mut self, frame_index: u32) -> Result<Vec<u8>> {
        match &mut self.backend {
            FrameBackend::Cpu => render_pipeline_frame_to_rgba(
                self.pipeline,
                self.media_ctx,
                self.executor,
                self.generated_cache,
                self.width,
                self.height,
                frame_index,
            ),
            #[cfg(target_os = "linux")]
            FrameBackend::Gpu(session) => session.render_frame_blocking(
                self.pipeline,
                self.media_ctx,
                self.executor,
                self.generated_cache,
                frame_index,
            ),
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            FrameBackend::Vk(session) => session.render_frame(
                self.pipeline,
                self.media_ctx,
                self.executor,
                self.generated_cache,
                frame_index,
            ),
        }
    }
}

/// Primary render entry: parse JSX → EnginePipeline → encode frames.
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
    use crate::script::V8JsContext;
    use opencat_core::pipeline::Pipeline;
    use opencat_core::script::js_context::JsContext;

    let cache_base =
        dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let loader = crate::resource::loader::EngineLoader::new(
        base_dir.unwrap_or(&cache_base).to_path_buf(),
        cache_base.join(".opencat").join("assets"),
    )?;
    let ctx = V8JsContext::new()?;
    crate::script::bridge::install_glsl_reflector(&ctx);
    crate::script::bridge::install_asset_bridge(&ctx, loader.base_dir());
    let pipeline = crate::pipeline::open(input, loader, ctx)?;
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
    use crate::script::V8JsContext;
    use opencat_core::pipeline::Pipeline;
    use opencat_core::script::js_context::JsContext;

    let output_path = output_path.as_ref();
    let cache_base =
        dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let cache_dir = cache_base.join(".opencat").join("assets");

    let loader = crate::resource::loader::EngineLoader::new(
        base_dir.unwrap_or(&cache_base).to_path_buf(),
        cache_dir,
    )?;
    let ctx = V8JsContext::new()?;
    crate::script::bridge::install_glsl_reflector(&ctx);
    crate::script::bridge::install_asset_bridge(&ctx, loader.base_dir());
    let mut pipeline = crate::pipeline::open(input, loader, ctx)?;
    let info = pipeline.info().clone();
    let frame_count = duration_secs_to_frames(info.duration, info.fps);

    let mut media_ctx = MediaContext::new();
    media_ctx.set_composition_fps(info.fps);
    let mut generated_cache = generated_image_cache_from_env();

    // 音轨只有整片 MP4 要：分段由拼接方统一编码，PNG 不带音频。
    let audio_track = match &config.format {
        OutputFormat::Mp4(_) => crate::media::audio::build_audio_track_from_pipeline(&pipeline)?,
        _ => None,
    };

    match &config.format {
        OutputFormat::Png(frames) => {
            let mut executor = crate::render::executor::EngineDrawExecutor::new();
            let mut renderer = FrameRenderer::new(
                &mut pipeline,
                &mut media_ctx,
                &mut executor,
                &mut generated_cache,
                frame_count,
                info.width,
                info.height,
            )?;
            let all: Vec<u32>;
            let frames = match frames {
                Some(list) => list.as_slice(),
                None => {
                    all = (0..frame_count).collect();
                    &all
                }
            };
            for &i in frames.iter().filter(|&&i| i < frame_count) {
                let rgba = renderer.render_frame_blocking(i)?;
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
                // CPU 后端：无 GPU 执行线程可拆，走串行 FrameRenderer。
                let mut executor = crate::render::executor::EngineDrawExecutor::new();
                let mut renderer = FrameRenderer::new(
                    &mut pipeline,
                    &mut media_ctx,
                    &mut executor,
                    &mut generated_cache,
                    frame_count,
                    aligned_info.0,
                    aligned_info.1,
                )?;
                // 段首预热一帧再丢弃：进程内第一帧的脚本相位（TLA 模块的
                // hf-seek 等）与连续渲染不同，预热后段内各帧与整片同相位。
                if range.start > 0 {
                    renderer.render_frame_blocking(range.start - 1)?;
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
                    |encode_index| renderer.render_for_encode(start + encode_index),
                )?;
                return Ok(());
            }

            // ── flume 三级流水线（GPU 路径）────────────────────────────
            // script（主线程；V8 isolate 非 Send 不能搬家）→ gpu 执行线程
            // （ANGLE/Skia 上下文 + media_ctx 归它所有）→ x264（编码在
            // `encode_rgba_frames` 内与渲染交错）。帧序 = channel FIFO，
            // 各阶段内部严格串行——frame=f(t) 确定性不变。loader/textures
            // 是 open 后只读的 Arc 共享；背压 2 帧：script 最多超前执行 2 帧。
            let loader_arc = pipeline.loader_arc().clone();
            let textures = pipeline.textures.clone();
            let composition_fps = info.fps;
            // Skia 的 GrDirectContext 非 Send（rust-skia 保守禁跨线程）——
            // session/缓存全部在 gpu 线程内创建，主线程零 GPU 状态。
            let (tx_frame, rx_frame) =
                flume::bounded::<(u32, opencat_core::ir::RenderFrame, bool)>(2);
            let (tx_px, rx_px) = flume::bounded::<Vec<u8>>(2);
            let gpu_error: std::sync::Arc<std::sync::Mutex<Option<String>>> =
                std::sync::Arc::default();

            // 段首预热一帧再丢弃：进程内第一帧的脚本相位（TLA 模块的
            // hf-seek 等）与连续渲染不同，预热后段内各帧与整片同相位。
            // 预热帧的 script 在主线程先行产出，经通道带 discard 标志送达。
            if range.start > 0 {
                let warm = pipeline.render_frame(range.start - 1)?;
                tx_frame.send((range.start - 1, warm, true))?;
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
                        tx_frame.send((range.start, rf, false))?;
                    }
                    Err(e) => return Err(anyhow!("script 阶段 f{}: {e:#}", range.start)),
                }
            }

            let gpu_err_slot = gpu_error.clone();
            let gpu_handle = std::thread::Builder::new()
                .name("oc-gpu-exec".into())
                .spawn(move || {
                    let mut session = match VkRenderSession::new(aligned_info.0, aligned_info.1)
                    {
                        Ok(s) => s,
                        Err(e) => {
                            *gpu_err_slot.lock().unwrap() = Some(format!("会话创建: {e:#}"));
                            return;
                        }
                    };
                    let mut media_ctx = MediaContext::new();
                    media_ctx.set_composition_fps(composition_fps);
                    let mut executor = crate::render::executor::EngineDrawExecutor::new();
                    let mut generated_cache = generated_image_cache_from_env();
                    while let Ok((f, rf, discard)) = rx_frame.recv() {
                        let outcome = session.execute_render(
                            rf,
                            &mut executor,
                            &mut generated_cache,
                            &mut media_ctx,
                            &textures,
                            &loader_arc,
                            f,
                        );
                        match outcome {
                            Ok(px) => {
                                if !discard && tx_px.send(px).is_err() {
                                    break; // 编码端提前收场
                                }
                            }
                            Err(e) => {
                                *gpu_err_slot.lock().unwrap() = Some(format!("f{f}: {e:#}"));
                                break;
                            }
                        }
                    }
                    // tx_px 随闭包结束 drop：script 端 send 失败、编码端
                    // recv 失败，都能感知收场。
                })?;

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
                                if tx_frame.send((script_next, rf, false)).is_err() {
                                    // gpu 线程已死：错误在槽位，recv 会立刻失败
                                }
                            }
                            Err(e) => {
                                script_error = Some(anyhow!("script 阶段 f{}: {e:#}", script_next));
                                anyhow::bail!("script 阶段 f{}: {e:#}", script_next);
                            }
                        }
                        script_next += 1;
                    }
                    if let Some(e) = gpu_error.lock().unwrap().as_ref() {
                        anyhow::bail!("gpu 执行线程: {e}");
                    }
                    let px =
                        rx_px.recv().map_err(|_| anyhow!("gpu 执行线程提前收场（无更多像素）"))?;
                    Ok(Some(px))
                },
            );
            // 收场：script 已推进完（或中途错），tx_frame 随作用域 drop 让
            // gpu 线程 recv None 自然退出。
            drop(tx_frame);
            let _ = gpu_handle.join();
            let gpu_err = gpu_error.lock().unwrap().take();
            // 根因优先：script/gpu 的错误盖过编码错误（后者多半是前者引起）。
            if let Some(e) = script_error {
                return Err(e);
            }
            if let Some(e) = gpu_err {
                return Err(anyhow!("gpu 执行线程: {e}"));
            }
            encode_result?;
        }
    }

    Ok(())
}

/// Render a single frame from a JSX source via EnginePipeline, returning raw RGBA bytes.
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
    use crate::script::V8JsContext;
    use opencat_core::pipeline::Pipeline;
    use opencat_core::script::js_context::JsContext;

    let cache_base =
        dirs::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let cache_dir = cache_base.join(".opencat").join("assets");

    let loader = crate::resource::loader::EngineLoader::new(
        base_dir.unwrap_or(&cache_base).to_path_buf(),
        cache_dir,
    )?;
    let ctx = V8JsContext::new()?;
    crate::script::bridge::install_glsl_reflector(&ctx);
    crate::script::bridge::install_asset_bridge(&ctx, loader.base_dir());
    let mut pipeline = crate::pipeline::open(input, loader, ctx)?;
    let info = pipeline.info().clone();
    let frame_count = duration_secs_to_frames(info.duration, info.fps);

    if frame_index >= frame_count {
        anyhow::bail!(
            "frame_index {} out of range (composition has {} frames)",
            frame_index,
            frame_count
        );
    }

    let mut media_ctx = MediaContext::new();
    media_ctx.set_composition_fps(info.fps);
    let mut executor = crate::render::executor::EngineDrawExecutor::new();
    let mut generated_cache = generated_image_cache_from_env();
    let mut renderer = FrameRenderer::new(
        &mut pipeline,
        &mut media_ctx,
        &mut executor,
        &mut generated_cache,
        frame_count,
        info.width,
        info.height,
    )?;
    let rgba = match renderer.render_frame_blocking(frame_index) {
        Ok(rgba) => rgba,
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


