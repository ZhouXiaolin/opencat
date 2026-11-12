//! Windows 呈现器：Skia Vulkan 离屏合成 + CPU blit 到窗口 DC。
//!
//! 三栈统一 Vulkan 后 WGL/GL 呈现退役（Windows 的 Skia 预编译包无 GL
//! 符号）。预览不是性能关键路径：Vulkan 离屏 surface 渲染、`read_pixels`
//! 读回 BGRA、`StretchDIBits` 拉到窗口——~10ms/帧 足够 20-30fps 预览。
//! 交换链直写是后续优化项，不阻塞。

use std::ffi::c_void;

use anyhow::{Result, anyhow};
use skia_safe::{
    AlphaType, ColorType, ImageInfo,
    gpu::{self, Budgeted, SurfaceOrigin},
};

use crate::gpu::TextureRegistry;
use crate::gpu::vk::VkSession;

/// 宿主窗口句柄（`raw-window-handle` 的 `Win32WindowHandle::hwnd`）。
pub struct WindowHandle {
    pub hwnd: *mut c_void,
}

pub struct Presenter {
    hwnd: *mut c_void,
    /// 独立 Vulkan 会话（预览无 ANGLE 流纹理要共享，不走 from_angle）。
    _session: std::sync::Arc<VkSession>,
    skia: gpu::DirectContext,
    /// 离屏合成 surface。尺寸 = 预览画布尺寸（合成单位），呈现时按窗口
    /// 拉伸。
    current_surface: Option<skia_safe::Surface>,
    /// `current_surface` 包裹的尺寸；窗口不可缩放，所以通常只建一次。
    surface_size: (i32, i32),
}

impl Presenter {
    /// # Safety
    /// `window.hwnd` 必须是宿主线程上有效的顶层窗口句柄。
    pub unsafe fn new(window: WindowHandle, _width: i32, _height: i32) -> Result<Self> {
        let session = std::sync::Arc::new(
            VkSession::new().map_err(|e| anyhow!("预览 Vulkan 会话创建失败: {e}"))?,
        );
        let skia = gpu::direct_contexts::make_vulkan(session.backend(), None)
            .ok_or_else(|| anyhow!("failed to create Skia Vulkan direct context"))?;

        Ok(Self {
            hwnd: window.hwnd,
            _session: session,
            skia,
            current_surface: None,
            surface_size: (0, 0),
        })
    }

    pub fn begin_frame(&mut self, width: i32, height: i32) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Err(anyhow!("invalid render target size {width}x{height}"));
        }

        if self.current_surface.is_none() || self.surface_size != (width, height) {
            let info = ImageInfo::new(
                (width, height),
                ColorType::RGBA8888,
                AlphaType::Premul,
                None,
            );
            let surface = gpu::surfaces::render_target(
                &mut self.skia,
                Budgeted::Yes,
                &info,
                None,
                SurfaceOrigin::TopLeft,
                None,
                None,
                None,
            )
            .ok_or_else(|| anyhow!("failed to create Skia Vulkan render target"))?;

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
        use windows_sys::Win32::Graphics::Gdi::{
            GetDC, ReleaseDC, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
            SRCCOPY,
        };

        let (w, h) = self.surface_size;
        let Some(surface) = self.current_surface.as_mut() else {
            return Err(anyhow!("present_frame called before begin_frame"));
        };
        // 直接读 BGRA：GDI 的 32bpp 是 BGRA 字节序，免逐字节 swizzle。
        let info = ImageInfo::new((w, h), ColorType::BGRA8888, AlphaType::Premul, None);
        let mut px = vec![0u8; w as usize * h as usize * 4];
        if !surface.read_pixels(&info, &mut px, w as usize * 4, (0, 0)) {
            return Err(anyhow!("preview read_pixels 失败"));
        }

        // top-down DIB（biHeight 负值）免行序翻转；全窗口拉伸。
        let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: px.len() as u32,
            ..unsafe { std::mem::zeroed() }
        };
        unsafe {
            let hdc = GetDC(self.hwnd);
            if hdc.is_null() {
                return Err(anyhow!("GetDC 失败"));
            }
            StretchDIBits(
                hdc,
                0,
                0,
                w,
                h,
                0,
                0,
                w,
                h,
                px.as_ptr().cast(),
                &bmi,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            ReleaseDC(self.hwnd, hdc);
        }
        Ok(())
    }

    /// 丢弃 Skia 缓存的后端状态（接口与 GL 时代一致；Vulkan 无共享状态
    /// 被踩的问题，保留以统一调用点）。
    pub fn reset_backend_state(&mut self) {
        self.skia.reset(None);
    }

    /// 在同一帧内借出绘制 canvas 与 GPU 图像解析器。
    ///
    /// 预览链路不执行 GPU 流（3D 预览不经此路），resolver 恒 `None`。
    pub fn with_canvas_and_resolver(
        &mut self,
        _textures: &TextureRegistry,
        f: &mut dyn FnMut(
            &mut skia_safe::Canvas,
            Option<&mut dyn crate::render::frame::GpuImageResolver>,
        ) -> Result<()>,
    ) -> Result<()> {
        let surface = self.current_surface.as_mut().ok_or_else(|| {
            anyhow!("skia canvas requested before Vulkan surface was ready")
        })?;
        #[allow(invalid_reference_casting)]
        let canvas: &mut skia_safe::Canvas =
            unsafe { &mut *(surface.canvas() as *const _ as *mut skia_safe::Canvas) };
        f(canvas, None)
    }
}

impl Drop for Presenter {
    fn drop(&mut self) {
        self.current_surface.take();
        self.skia.release_resources_and_abandon();
    }
}
