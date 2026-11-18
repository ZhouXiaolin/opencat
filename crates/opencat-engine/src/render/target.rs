//! RenderTarget / StreamRunner —— 执行端骨架（M4-⑦ A′，ADR 0001：只住 engine）。
//!
//! 红线（评审拍板）：
//! - [`RenderTarget::execute`] 吃一帧纯数据 [`FrameJob`]，**不收帧号**——
//!   帧序由调用方（script 侧 frame=f(t)）与 [`StreamRunner`] 的通道 FIFO
//!   承载；重叠流水线（render(N+1) 与 execute(N) 分线程重叠）的命脉就是
//!   执行端对「这是第几帧」零依赖。`FrameJob::label` 只是日志/dump 标签。
//! - 后端实现不碰 [`OutputFormat`](super::OutputFormat)/flume——运输带
//!   细节全部住 [`StreamRunner`]。
//! - 全部 OC_* 开关语义不变（后端选择仍由 `OC_GPU_PIPELINE`/
//!   `OC_SKIA_BACKEND` 在编排层判别）。
//!
//! 收编次序按评审：VK 先迁（本批，生产路）→ CPU → GL。

use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};

use opencat_core::ir::RenderFrame;

use super::executor::EngineDrawExecutor;
use super::{GeneratedImageCache, VkRenderSession, generated_image_cache_from_env};
use crate::gpu::TextureRegistry;
use crate::media::MediaContext;
use crate::resource::loader::EngineLoader;

/// 会话级只读共享束：open 后不再写的宿主状态，Arc 共享给并行渲染线程。
#[derive(Clone)]
pub struct SessionDeps {
    pub loader: Arc<EngineLoader>,
    pub textures: Arc<TextureRegistry>,
}

impl SessionDeps {
    pub fn from_pipeline(pipeline: &crate::pipeline::EnginePipelineHost) -> Self {
        Self {
            loader: pipeline.loader_arc().clone(),
            textures: pipeline.textures.clone(),
        }
    }
}

/// 一帧执行的纯数据输入。`render` 按值接管（`draw` 在重放中被排空）；
/// `deps` 是会话级只读共享；`label` 仅用于日志/dump 文件名（通常 = 帧号，
/// 但执行语义绝不读它）。
pub struct FrameJob<'a> {
    pub render: RenderFrame,
    pub deps: &'a SessionDeps,
    pub label: u32,
}

/// 渲染目标：执行端统一签名（评审 B 的定型依据）。
///
/// 实现者自带会话级可变协作者（draw executor / generated cache /
/// media ctx）——它们只属于执行线程，不跨线程；本 trait 只约定
/// 「一帧纯数据进、一帧 premul RGBA 出」。
pub trait RenderTarget {
    fn execute(&mut self, job: FrameJob<'_>) -> Result<Vec<u8>>;
}

/// 工厂产物（`Box<dyn RenderTarget>`）直接当 target 用：StreamRunner 的
/// `make_target` 闭包走 [`make_render_target`] 时，执行线程拿到的就是
/// boxed trait 对象——委托 impl 让它与具名 target 同型，backend 判别
/// 语义（OC_SKIA_BACKEND / gpu::requested）在 Png、MP4、预览三路一致。
impl RenderTarget for Box<dyn RenderTarget> {
    fn execute(&mut self, job: FrameJob<'_>) -> Result<Vec<u8>> {
        (**self).execute(job)
    }
}

/// VK 生产路的 [`RenderTarget`] 适配：会话 + 执行线程专属协作者
/// 全部在此自建（GL/Skia 上下文非 Send，产物不跨线程）。
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub struct VkRenderTarget {
    session: VkRenderSession,
    executor: EngineDrawExecutor,
    generated_cache: GeneratedImageCache,
    media_ctx: MediaContext,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl VkRenderTarget {
    /// 建会话与线程专属协作者。`width`/`height` 用编排层算好的对齐尺寸
    /// （x264 偶数要求，收编前 = `aligned_info`）；`fps` 进 `MediaContext`
    /// （实时解码窗口用）。
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Self> {
        let mut media_ctx = MediaContext::new();
        media_ctx.set_composition_fps(fps);
        Ok(Self {
            session: VkRenderSession::new(width, height)?,
            executor: EngineDrawExecutor::new(),
            generated_cache: generated_image_cache_from_env(),
            media_ctx,
        })
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl RenderTarget for VkRenderTarget {
    fn execute(&mut self, job: FrameJob<'_>) -> Result<Vec<u8>> {
        let SessionDeps { loader, textures } = job.deps;
        self.session.execute_render(
            job.render,
            &mut self.executor,
            &mut self.generated_cache,
            &mut self.media_ctx,
            textures,
            loader,
            job.label,
        )
    }
}

/// CPU 软光栅目标（三平台通用）：Skia raster surface 直接画，无 GPU 流
/// 执行器（GPU 命令流走 CPU 回读路径 `cpu_execute_gpu_streams`）。
pub struct CpuRenderTarget {
    executor: EngineDrawExecutor,
    generated_cache: GeneratedImageCache,
    media_ctx: MediaContext,
    surface_w: u32,
    surface_h: u32,
}

impl CpuRenderTarget {
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Self> {
        let mut media_ctx = MediaContext::new();
        media_ctx.set_composition_fps(fps);
        Ok(Self {
            executor: EngineDrawExecutor::new(),
            generated_cache: generated_image_cache_from_env(),
            media_ctx,
            surface_w: width,
            surface_h: height,
        })
    }
}

impl RenderTarget for CpuRenderTarget {
    fn execute(&mut self, job: FrameJob<'_>) -> Result<Vec<u8>> {
        let SessionDeps { loader, textures } = job.deps;
        let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
        let t0 = timing.then(std::time::Instant::now);
        let mut frame = job.render.draw;
        let mut media_plan = job.render.media;
        if let Some(gpu) = &job.render.gpu {
            if let Some(dir) = std::env::var_os("OC_DUMP_GPU_STREAM") {
                super::dump_gpu_streams(std::path::Path::new(&dir), gpu, job.label);
            }
            super::cpu_execute_gpu_streams(textures, &mut media_plan, gpu)?;
        }
        let t_gpu = t0.map(|t| t.elapsed());

        let mut surface = skia_safe::surfaces::raster_n32_premul((
            self.surface_w as i32,
            self.surface_h as i32,
        ))
        .ok_or_else(|| anyhow!("failed to create skia raster surface"))?;

        // SAFETY: skia_safe::Canvas wraps a C++ ref-counted object with interior mutability.
        // All draw methods take &self at the Rust level while mutating internal C++ state.
        // The surface owns the canvas and no other references exist at this point.
        #[allow(invalid_reference_casting)]
        let canvas: &mut skia_safe::Canvas =
            unsafe { &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas) };

        super::frame::execute_render_frame(
            &mut frame,
            &media_plan,
            loader,
            super::frame::FrameExec {
                executor: &mut self.executor,
                media_ctx: &mut self.media_ctx,
                generated_cache: &mut self.generated_cache,
            },
            canvas,
        )?;
        let t_draw = t0.map(|t| t.elapsed());

        let image = surface.image_snapshot();
        let image_info = skia_safe::ImageInfo::new(
            (self.surface_w as i32, self.surface_h as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        let mut rgba = vec![0u8; (self.surface_w as usize) * (self.surface_h as usize) * 4];
        let read_ok = image.read_pixels(
            &image_info,
            rgba.as_mut_slice(),
            self.surface_w as usize * 4,
            (0, 0),
            skia_safe::image::CachingHint::Allow,
        );
        if !read_ok {
            return Err(anyhow!("failed to read pixels from skia surface"));
        }
        if let Some(t0) = t0 {
            let t_end = t0.elapsed();
            // script 分段由编排层打（[timing-script]）；本行只覆盖执行段。
            eprintln!(
                "[timing-cpu] f{} gpu-stream={:.1}ms draw={:.1}ms readback={:.1}ms total={:.1}ms",
                job.label,
                (t_gpu.unwrap_or_default()).as_secs_f64() * 1e3,
                (t_draw.unwrap_or_default() - t_gpu.unwrap_or_default()).as_secs_f64() * 1e3,
                (t_end - t_draw.unwrap_or_default()).as_secs_f64() * 1e3,
                t_end.as_secs_f64() * 1e3,
            );
        }
        Ok(rgba)
    }
}

/// GL 合成目标（Linux）：EGL 离屏会话 + Skia GPU 合成器（原
/// `GpuRenderSession` 的同步读回路径，PNG/单帧这类一次一帧的调用方）。
#[cfg(target_os = "linux")]
pub struct GlRenderTarget {
    session: super::GpuRenderSession,
    executor: EngineDrawExecutor,
    generated_cache: GeneratedImageCache,
    media_ctx: MediaContext,
}

#[cfg(target_os = "linux")]
impl GlRenderTarget {
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Self> {
        let mut media_ctx = MediaContext::new();
        media_ctx.set_composition_fps(fps);
        Ok(Self {
            session: super::GpuRenderSession::new(width, height)?,
            executor: EngineDrawExecutor::new(),
            generated_cache: generated_image_cache_from_env(),
            media_ctx,
        })
    }
}

#[cfg(target_os = "linux")]
impl RenderTarget for GlRenderTarget {
    fn execute(&mut self, job: FrameJob<'_>) -> Result<Vec<u8>> {
        let SessionDeps { loader, .. } = job.deps;
        // 同步读回本帧（deferred=false）；GL 会话自带纹理登记处（与 Vk
        // 共享 pipeline.textures 的拓扑不同，逐位保持）。script 分段由
        // 编排层打，execute 段从本调用起计。
        let timing = std::env::var_os("OC_FRAME_TIMING").is_some();
        let t0 = timing.then(std::time::Instant::now);
        let px = self.session.execute_frame(
            job.render,
            loader,
            &mut self.media_ctx,
            &mut self.executor,
            &mut self.generated_cache,
            job.label,
            false,
            t0,
            None,
        )?;
        Ok(px.expect("同步读回必定产出本帧"))
    }
}

/// 按环境判别建执行端（原 `FrameRenderer::new` 的三态判别收编；
/// `OC_SKIA_BACKEND`/`OC_GPU_PIPELINE` 判别点原样）。
pub fn make_render_target(width: u32, height: u32, fps: u32) -> Result<Box<dyn RenderTarget>> {
    // `OC_SKIA_BACKEND=vulkan` 切到 Vulkan 路径（Skia 的光栅走
    // Vulkan backend）。**必须先于 GL 分支判别**：后者会建自己的 ANGLE
    // 会话，若随后被 Vk 分支遮蔽丢弃，其 Drop 调 eglTerminate 终止共享
    // display——零拷贝路径的 make_current 从此拿 EGL_NOT_INITIALIZED，
    // ANGLE 的 VkDevice 连带销毁，Skia 的 VMA 析构撞死 device（lvp 实测
    // 双症状同源）。
    #[cfg(target_os = "linux")]
    let target: Box<dyn RenderTarget> =
        if std::env::var("OC_SKIA_BACKEND").as_deref() == Ok("vulkan")
            && crate::gpu::requested()
        {
            Box::new(VkRenderTarget::new(width, height, fps)?)
        } else if crate::gpu::requested() {
            Box::new(GlRenderTarget::new(width, height, fps)?)
        } else {
            Box::new(CpuRenderTarget::new(width, height, fps)?)
        };
    #[cfg(target_os = "windows")]
    let target: Box<dyn RenderTarget> = if crate::gpu::requested() {
        Box::new(VkRenderTarget::new(width, height, fps)?)
    } else {
        Box::new(CpuRenderTarget::new(width, height, fps)?)
    };
    #[cfg(target_os = "macos")]
    let target: Box<dyn RenderTarget> = Box::new(CpuRenderTarget::new(width, height, fps)?);
    Ok(target)
}

/// 三级重叠流水线的执行端运输带（原 GPU 分支手写编排的收编）：
///
/// `script 线程产 RenderFrame → (flume, 背压 2) → 执行线程
/// execute(FrameJob) → (flume, 背压 2) → 像素`。
///
/// 帧序 = 双通道 FIFO，各阶段内部严格串行——frame=f(t) 确定性不变。
/// 会话在线程闭包内经 `make_target` 自建（GL/Skia 上下文非 Send）；
/// 错误经槽位传递，`recv_px`/`finish` 处显式浮出。
pub struct StreamRunner {
    tx_frame: Option<flume::Sender<(u32, RenderFrame, bool)>>,
    rx_px: flume::Receiver<Vec<u8>>,
    error: Arc<Mutex<Option<String>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl StreamRunner {
    /// 启动执行线程。`make_target` 在线程内调用（产物留在该线程直到
    /// `finish`）；背压与通道拓扑固定——编码回调每推进一帧 script、
    /// 取一帧像素的节奏由编排层保持（submit 先于 recv_px）。
    pub fn spawn<T, F>(make_target: F, deps: SessionDeps) -> Result<Self>
    where
        T: RenderTarget + 'static,
        F: FnOnce(&SessionDeps) -> Result<T> + Send + 'static,
    {
        let (tx_frame, rx_frame) = flume::bounded::<(u32, RenderFrame, bool)>(2);
        let (tx_px, rx_px) = flume::bounded::<Vec<u8>>(2);
        let error: Arc<Mutex<Option<String>>> = Arc::default();
        let err_slot = error.clone();
        let thread_deps = deps.clone();
        let handle = std::thread::Builder::new()
            .name("oc-gpu-exec".into())
            .spawn(move || {
                let mut target = match make_target(&thread_deps) {
                    Ok(t) => t,
                    Err(e) => {
                        *err_slot.lock().unwrap() = Some(format!("会话创建: {e:#}"));
                        return;
                    }
                };
                while let Ok((label, rf, discard)) = rx_frame.recv() {
                    let job = FrameJob {
                        render: rf,
                        deps: &thread_deps,
                        label,
                    };
                    match target.execute(job) {
                        Ok(px) => {
                            if !discard && tx_px.send(px).is_err() {
                                break; // 编码端提前收场
                            }
                        }
                        Err(e) => {
                            *err_slot.lock().unwrap() = Some(format!("f{label}: {e:#}"));
                            break;
                        }
                    }
                }
                // tx_px 随闭包结束 drop：script 端 send 失败、编码端
                // recv 失败，都能感知收场。
            })?;
        Ok(Self {
            tx_frame: Some(tx_frame),
            rx_px,
            error,
            handle: Some(handle),
        })
    }

    /// 提交一帧（`discard` = 预热帧，执行但不产像素）。通道满则阻塞——
    /// 背压 2 帧即 script 最多超前执行 2 帧。执行线程已死时静默返回
    /// （错误在槽位，`recv_px`/`finish` 浮出——与收编前语义一致）。
    pub fn submit(&self, label: u32, render: RenderFrame, discard: bool) {
        if let Some(tx) = &self.tx_frame {
            let _ = tx.send((label, render, discard));
        }
    }

    /// 取下一帧像素（执行线程死了则报槽位错误）。
    pub fn recv_px(&self) -> Result<Vec<u8>> {
        if let Some(e) = self.error.lock().unwrap().as_ref() {
            anyhow::bail!("gpu 执行线程: {e}");
        }
        self.rx_px
            .recv()
            .map_err(|_| anyhow!("gpu 执行线程提前收场（无更多像素）"))
    }

    /// 显式检查执行端错误（编码回调里每帧先查再取，与收编前一致）。
    pub fn check_error(&self) -> Result<()> {
        match self.error.lock().unwrap().as_ref() {
            Some(e) => Err(anyhow!("gpu 执行线程: {e}")),
            None => Ok(()),
        }
    }

    /// 收场：drop 帧通道让执行线程 recv 失败自然退出，join 并浮出错误。
    pub fn finish(mut self) -> Result<()> {
        self.tx_frame = None; // script 已推进完（或中途错），执行线程自然退出
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        match self.error.lock().unwrap().take() {
            Some(e) => Err(anyhow!("gpu 执行线程: {e}")),
            None => Ok(()),
        }
    }
}
