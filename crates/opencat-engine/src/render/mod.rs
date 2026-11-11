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
    let render = pipeline.render_frame(frame_index)?;
    let mut frame = render.draw;
    let mut media_plan = render.media;
    if let Some(gpu) = &render.gpu {
        if let Some(dir) = std::env::var_os("OC_DUMP_GPU_STREAM") {
            dump_gpu_streams(std::path::Path::new(&dir), gpu, frame_index);
        }
        cpu_execute_gpu_streams(&pipeline.textures, &mut media_plan, gpu)?;
    }

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
    Ok(rgba)
}

/// GPU 合成路径的会话状态（Windows=WGL / Linux=EGL 的桌面 GL）。
///
/// 持有离屏 GL 会话与 Skia GPU 合成器。**线程纪律**：GL 上下文非线程安全——
/// 会话在线程 A 建立就在线程 A 使用（离线渲染的建与用同线程），每帧前
/// `make_current` 一次；无锁。
#[cfg(any(target_os = "windows", target_os = "linux"))]
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
#[cfg(any(target_os = "windows", target_os = "linux"))]
struct CompositeTexture {
    gl: std::sync::Arc<glow::Context>,
    tex: crate::gpu::NativeTexture,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Drop for CompositeTexture {
    fn drop(&mut self) {
        crate::gpu::gl::delete_texture(&self.gl, self.tex);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl GpuRenderSession {
    /// 建一个 `width × height` 的 GPU 合成会话。
    pub fn new(width: u32, height: u32) -> Result<Self> {
        // 离屏 GL 会话：构造即 make current。
        let gl_session = crate::gpu::session::GlSession::offscreen()?;
        let gl = gl_session.gl();
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
        self.gl_session.make_current()?;
        let render = pipeline.render_frame(frame_index)?;
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
        let mut stream_outputs: Vec<(crate::gpu::NativeTexture, u32)> = Vec::new();
        // 流执行过 = GL 上下文被改过（哪怕占位不可见、输出被丢弃——three
        // 这类持久 renderer 的不可见帧照常执行命令），Skia 合成前就必须
        // reset。判据不是 gpu_handles：输出被丢弃的帧 handle 为空。
        let mut gpu_stream_ran = false;
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
                pipeline.loader(),
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
            compositor.finish_frame_blocking().map(Some)?
        };

        // 合成完成：删 GL 资源（像素已被 Skia 采样进合成帧），归还凭据。
        if std::env::var_os("OC_KEEP_STREAM_TEX").is_none() {
            for (tex, fbo) in stream_outputs {
                crate::gpu::gl::delete_framebuffer(&self.gl_session.gl(), fbo);
                crate::gpu::gl::delete_texture(&self.gl_session.gl(), tex);
            }
        }
        for id in gpu_handles {
            textures.release(id);
        }
        Ok(rgba)
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
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Gpu(Box<GpuRenderSession>),
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
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        let backend = if crate::gpu::requested() {
            FrameBackend::Gpu(Box::new(GpuRenderSession::new(width, height)?))
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
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            FrameBackend::Gpu(session) => session.render_frame(
                self.pipeline,
                self.media_ctx,
                self.executor,
                self.generated_cache,
                frame_index,
            ),
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
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            FrameBackend::Gpu(session) => session.render_frame_blocking(
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
    let rgba = renderer.render_frame_blocking(frame_index)?;

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


