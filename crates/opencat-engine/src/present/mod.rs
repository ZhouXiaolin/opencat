//! 预览呈现器：把合成帧画到宿主窗口并呈现。
//!
//! 宿主（`opencat-see`）只管窗口与帧循环——它不认识 Skia，也不认识原生
//! 图形 API。窗口的图形上下文、Skia 后端、每帧的 surface 都归本模块。
//!
//! 三个平台各一个实现，对宿主是同一个 [`PreviewPresenter`]：
//!
//! | 平台 | 上下文 | Skia 后端 |
//! |---|---|---|
//! | Windows | WGL（[`opencat_gpu_native::win32::WglContext`]） | GL |
//! | macOS | CAMetalLayer | Metal |
//! | Linux | EGL（X11） | GL |
//!
//! 窗口句柄由宿主从 `raw-window-handle` 解出后交给 [`WindowHandle`]——
//! 那是宿主唯一需要认识平台的地方。

use anyhow::Result;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod win32;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use win32 as platform;

pub use platform::WindowHandle;

use crate::EnginePipeline;
use crate::executor::EngineDrawExecutor;
use crate::media::MediaContext;
use crate::render::GeneratedImageCache;
use opencat_gpu_native::TextureId;

/// 一帧的渲染输入。宿主提供**数据**，不提供任何图形对象。
pub struct PreviewFrame<'a> {
    pub pipeline: &'a mut EnginePipeline,
    pub media_ctx: &'a mut MediaContext,
    pub executor: &'a mut EngineDrawExecutor,
    pub generated_cache: &'a mut GeneratedImageCache,
    /// 合成单位下的画布尺寸。
    pub width: i32,
    pub height: i32,
    pub frame_index: u32,
    /// 画布缩放；1.0 表示按合成原分辨率预览。
    pub render_scale: f32,
}

/// 宿主窗口上的呈现器。
pub struct PreviewPresenter {
    inner: platform::Presenter,
}

impl PreviewPresenter {
    /// 用宿主的窗口建呈现器，并把 wgpu 加入窗口的图形上下文。
    ///
    /// 加入成功后，3D 输出纹理对本节点的 Skia 可直接采样（零拷贝）；后端
    /// 不支持时保持懒初始化，3D 走 CPU 读回。必须在首帧渲染前调用
    /// （`preset` 只设一次）。
    pub fn for_window(
        window: WindowHandle,
        pipeline: &EnginePipeline,
        width: i32,
        height: i32,
    ) -> Result<Self> {
        let mut inner = platform::Presenter::new(window, width, height)?;
        if let Some(renderer) = inner.build_scene3_renderer()? {
            pipeline.scene3_renderer.preset(renderer)?;
        }
        Ok(Self { inner })
    }

    /// 渲染一帧并呈现到窗口。
    pub fn render_frame(&mut self, frame: PreviewFrame<'_>) -> Result<()> {
        let PreviewFrame {
            pipeline,
            media_ctx,
            executor,
            generated_cache,
            width,
            height,
            frame_index,
            render_scale,
        } = frame;

        self.inner.begin_frame(width, height)?;

        let render = pipeline.render_frame(frame_index)?;
        let mut draw = render.draw;
        let media_plan = render.media;

        // 本帧引用的 GPU 纹理凭据：合成后归还。
        let gpu_tokens: Vec<TextureId> = media_plan
            .generated_images
            .iter()
            .filter_map(|g| g.gpu.map(|h| TextureId::from_raw(h.token)))
            .collect();

        // 有 3D 时 wgpu 刚跑过图形 pass，Skia 缓存的状态已失效。
        if !gpu_tokens.is_empty() {
            self.inner.reset_backend_state();
        }

        let loader = pipeline.loader();
        let textures = &pipeline.textures;
        let result = self
            .inner
            .with_canvas_and_resolver(textures, &mut |canvas, resolver| {
                // 画布坐标系仍是合成单位（DrawOp 的坐标不变），只把设备矩阵
                // 缩放。用 `reset_matrix` 而不是 `save`/`scale`：DrawOp 里的
                // `RestoreToCount` 带绝对计数，多一层 save 会被它弹掉。
                canvas.reset_matrix();
                if render_scale != 1.0 {
                    canvas.scale((render_scale, render_scale));
                }
                crate::consumer::execute_render_frame_with_gpu(
                    &mut draw,
                    &media_plan,
                    executor,
                    loader,
                    media_ctx,
                    generated_cache,
                    resolver,
                    canvas,
                )
                .map_err(|e| anyhow::anyhow!("{e}"))
            });
        for id in gpu_tokens {
            textures.release(id);
        }
        result?;

        self.inner.end_frame()?;
        self.inner.present_frame()?;
        Ok(())
    }
}
