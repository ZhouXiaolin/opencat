//! Linux 呈现器：EGL（X11）+ Skia GL 后端。
//!
//! 窗口的 EGL 上下文由 [`opencat_gpu_native::egl::EglContext`] 建并持有；
//! 本模块只做 Skia 那一侧。wgpu 以外部上下文模式加入同一 context（见
//! [`Self::build_scene3_renderer`]），3D 输出纹理对本节点的 Skia 可直接
//! 采样——零拷贝。

use std::ffi::c_void;

use anyhow::{Result, anyhow};
use opencat_gpu_native::TextureRegistry;
use opencat_gpu_native::egl::EglContext;
use opencat_three::Renderer3d;
use skia_safe::{
    ColorType,
    gpu::{self, SurfaceOrigin, backend_render_targets, gl},
};

use crate::consumer::GpuImageResolver;
use crate::gpu_compositor::CompositorResolver;

/// 宿主窗口句柄。两者都来自 `raw-window-handle` 的 Xlib 句柄。
pub struct WindowHandle {
    pub x11_display: *mut c_void,
    pub x11_window: u64,
}

pub struct Presenter {
    ctx: EglContext,
    skia: gpu::DirectContext,
    /// 当前帧的 surface。窗口是双缓冲的默认帧缓冲，所以每帧重建（尺寸变时）。
    current_surface: Option<skia_safe::Surface>,
    /// `current_surface` 包裹的帧缓冲尺寸；窗口不可缩放，所以通常只建一次。
    surface_size: (i32, i32),
}

impl Presenter {
    pub fn new(window: WindowHandle, _width: i32, _height: i32) -> Result<Self> {
        let ctx = EglContext::from_window(window.x11_display, window.x11_window)?;
        let interface = gl::Interface::new_load_with(opencat_gpu_native::egl::load_proc)
            .ok_or_else(|| anyhow!("failed to create Skia OpenGL interface (EGL)"))?;
        let skia = gpu::direct_contexts::make_gl(interface, None)
            .ok_or_else(|| anyhow!("failed to create Skia OpenGL direct context (EGL)"))?;

        Ok(Self {
            ctx,
            skia,
            current_surface: None,
            surface_size: (0, 0),
        })
    }

    fn make_current(&self) -> Result<()> {
        self.ctx.make_current()
    }

    pub fn begin_frame(&mut self, width: i32, height: i32) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Err(anyhow!("invalid render target size {width}x{height}"));
        }

        self.make_current()?;

        if self.current_surface.is_none() || self.surface_size != (width, height) {
            // fboid 0 = 窗口的默认帧缓冲。
            let backend_render_target = backend_render_targets::make_gl(
                (width, height),
                0,
                8,
                gl::FramebufferInfo {
                    fboid: 0,
                    format: gl::Format::RGBA8.into(),
                    ..Default::default()
                },
            );

            let surface = gpu::surfaces::wrap_backend_render_target(
                &mut self.skia,
                &backend_render_target,
                SurfaceOrigin::BottomLeft,
                ColorType::RGBA8888,
                None,
                None,
            )
            .ok_or_else(|| anyhow!("failed to wrap Linux OpenGL framebuffer for Skia"))?;

            self.current_surface = Some(surface);
            self.surface_size = (width, height);
        }
        Ok(())
    }

    pub fn end_frame(&mut self) -> Result<()> {
        let surface = self
            .current_surface
            .as_mut()
            .ok_or_else(|| anyhow!("end_frame called before begin_frame"))?;
        self.skia.flush_and_submit_surface(surface, None);
        Ok(())
    }

    pub fn present_frame(&mut self) -> Result<()> {
        self.make_current()?;
        self.ctx.swap_buffers()
    }

    /// 丢弃 Skia 缓存的 GL 状态。
    ///
    /// wgpu 的 3D pass 会改动 GL 状态（绑定、viewport、program……），而 Skia
    /// 的 `DirectContext` 缓存了它上次留下的状态。二者共享同一 EGL context，
    /// 所以每帧合成前必须让 Skia 失效缓存，否则它会按过期状态发包。
    pub fn reset_backend_state(&mut self) {
        self.skia.reset(None);
    }

    /// 建一个与本窗口 EGL 上下文**共享**的 wgpu 渲染器（3D 零拷贝）。
    ///
    /// `make_current` 回调是必需的——wgpu-hal 的 GL 实例初始化会在结尾
    /// unmake，把宿主的上下文解绑。
    pub fn build_scene3_renderer(&mut self) -> Result<Option<Renderer3d>> {
        Ok(Some(Renderer3d::new_external(
            |name| opencat_gpu_native::egl::load_proc(name),
            || self.ctx.make_current(),
        )?))
    }

    /// 在同一帧内借出绘制 canvas 与 GPU 图像解析器。
    ///
    /// 二者必须同时可用（合成时要导入 3D 纹理），所以用一个回调而非两个
    /// 返回值——否则 `&mut self` 会被借两次。
    pub fn with_canvas_and_resolver(
        &mut self,
        textures: &TextureRegistry,
        f: &mut dyn FnMut(
            &mut skia_safe::Canvas,
            Option<&mut dyn GpuImageResolver>,
        ) -> Result<()>,
    ) -> Result<()> {
        // canvas 与 skia 是不同字段，可并存借用（同 `GpuCompositor::split`）。
        let surface = self.current_surface.as_mut().ok_or_else(|| {
            anyhow!("skia canvas requested before Linux framebuffer surface was ready")
        })?;
        #[allow(invalid_reference_casting)]
        let canvas: &mut skia_safe::Canvas = unsafe {
            &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
        };
        let mut resolver = CompositorResolver {
            skia: &mut self.skia,
            textures,
        };
        f(canvas, Some(&mut resolver))
    }
}

impl Drop for Presenter {
    fn drop(&mut self) {
        self.current_surface.take();
        self.skia.release_resources_and_abandon();
        // `self.ctx` 的 Drop 负责解绑、删上下文/表面、terminate display。
    }
}
