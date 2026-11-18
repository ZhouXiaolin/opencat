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
