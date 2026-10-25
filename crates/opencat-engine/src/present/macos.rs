//! macOS 呈现器：CAMetalLayer + Skia Metal 后端。

use std::ffi::c_void;

use anyhow::{Result, anyhow};
use cocoa::appkit::NSView;
use cocoa::base::{YES, id};
use core_graphics_types::geometry::CGSize;
// `ForeignType::as_ptr` / `as_ref` —— `Device::as_ptr`、`layer.as_ref` 要用。
use foreign_types::{ForeignType, ForeignTypeRef};
use metal::{CommandQueue, Device, MTLPixelFormat, MetalDrawable, MetalLayer};
use opencat_three::Renderer3d;
use skia_safe::{
    ColorType,
    gpu::{self, SurfaceOrigin, backend_render_targets, mtl},
};

/// 宿主窗口句柄。`ns_view` 来自 `raw-window-handle` 的 `AppKitWindowHandle`。
pub struct WindowHandle {
    pub ns_view: *mut c_void,
    /// 窗口的 DPI 缩放（`winit::Window::scale_factor`）。
    pub scale_factor: f64,
}

pub struct Presenter {
    layer: MetalLayer,
    command_queue: CommandQueue,
    skia: gpu::DirectContext,
    current_drawable: Option<MetalDrawable>,
    current_surface: Option<skia_safe::Surface>,
}

impl Presenter {
    pub fn new(window: WindowHandle, width: i32, height: i32) -> Result<Self> {
        let device = Device::system_default().ok_or_else(|| anyhow!("no Metal device found"))?;
        let command_queue = device.new_command_queue();

        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_presents_with_transaction(false);
        layer.set_display_sync_enabled(true);
        layer.set_framebuffer_only(false);
        layer.set_contents_scale(window.scale_factor);
        layer.set_drawable_size(CGSize::new(width as f64, height as f64));

        unsafe {
            let view: id = window.ns_view as id;
            view.setWantsLayer(YES);
            view.setLayer(std::mem::transmute(layer.as_ref()));
        }

        let backend = unsafe {
            mtl::BackendContext::new(
                device.as_ptr() as mtl::Handle,
                command_queue.as_ptr() as mtl::Handle,
            )
        };
        let skia = gpu::direct_contexts::make_metal(&backend, None)
            .ok_or_else(|| anyhow!("failed to create Skia Metal direct context"))?;

        Ok(Self {
            layer,
            command_queue,
            skia,
            current_drawable: None,
            current_surface: None,
        })
    }

    pub fn begin_frame(&mut self, width: i32, height: i32) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Err(anyhow!("invalid render target size {width}x{height}"));
        }

        // drawable 尺寸按合成分辨率的物理像素走；`contents_scale` 已在构造时
        // 按窗口 DPI 设过。
        self.layer
            .set_drawable_size(CGSize::new(width as f64, height as f64));

        let drawable_ref = self
            .layer
            .next_drawable()
            .ok_or_else(|| anyhow!("CAMetalLayer failed to produce drawable"))?;
        let drawable = drawable_ref.to_owned();

        let drawable_size = self.layer.drawable_size();
        let (drawable_width, drawable_height) =
            (drawable_size.width as i32, drawable_size.height as i32);

        if drawable_width <= 0 || drawable_height <= 0 {
            return Err(anyhow!(
                "invalid drawable size {}x{}",
                drawable_width,
                drawable_height
            ));
        }

        let texture_info =
            unsafe { mtl::TextureInfo::new(drawable.texture().as_ptr() as mtl::Handle) };

        let backend_render_target =
            backend_render_targets::make_mtl((drawable_width, drawable_height), &texture_info);

        let surface = gpu::surfaces::wrap_backend_render_target(
            &mut self.skia,
            &backend_render_target,
            SurfaceOrigin::TopLeft,
            ColorType::BGRA8888,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("failed to wrap metal backend render target"))?;

        self.current_drawable = Some(drawable);
        self.current_surface = Some(surface);
        Ok(())
    }

    pub fn end_frame(&mut self) -> Result<()> {
        let surface = self
            .current_surface
            .take()
            .ok_or_else(|| anyhow!("end_frame called before begin_frame"))?;
        self.skia.flush_and_submit();
        drop(surface);
        Ok(())
    }

    pub fn present_frame(&mut self) -> Result<()> {
        let drawable = self
            .current_drawable
            .take()
            .ok_or_else(|| anyhow!("present_frame called before end_frame"))?;
        let command_buffer = self.command_queue.new_command_buffer();
        command_buffer.present_drawable(&drawable);
        command_buffer.commit();
        Ok(())
    }

    /// Metal 没有 GL 式的共享 context，Skia 与本节点之间不需要重同步状态
    /// （跨队列顺序由 wgpu 出口的 `poll(Wait)` 与 `metal::Target` 的
    /// `wait_inflight` 保证），所以为空操作。
    pub fn reset_backend_state(&mut self) {}

    /// 建 wgpu 的 Metal 渲染器，与本呈现器共享同一块 `MTLDevice`。
    ///
    /// Metal 不像 GL 需要「加入宿主的 context」：系统默认设备是进程级单例，
    /// wgpu 自建的渲染器与这里 Skia 的 `DirectContext` 落在同一块设备上，
    /// 纹理凭裸指针即可跨队列共享，因此 3D 输出同样走零拷贝。
    pub fn build_scene3_renderer(&mut self) -> Result<Option<Renderer3d>> {
        Ok(Some(Renderer3d::new_blocking()?))
    }

    pub fn with_canvas_and_resolver(
        &mut self,
        textures: &opencat_gpu_native::TextureRegistry,
        f: &mut dyn FnMut(
            &mut skia_safe::Canvas,
            Option<&mut dyn crate::consumer::GpuImageResolver>,
        ) -> Result<()>,
    ) -> Result<()> {
        let surface = self
            .current_surface
            .as_mut()
            .ok_or_else(|| anyhow!("skia canvas requested before drawable surface was ready"))?;
        // SAFETY: skia_safe::Canvas has interior mutability; surface owns the canvas.
        #[allow(invalid_reference_casting)]
        let canvas: &mut skia_safe::Canvas = unsafe {
            &mut *(surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
        };
        let mut resolver = crate::gpu_compositor::CompositorResolver {
            skia: &mut self.skia,
            textures,
        };
        f(canvas, Some(&mut resolver))
    }
}
