//! Skia GPU 合成器。Linux/Windows 离线 Engine 路径使用 Skia Vulkan。
//!
//! 全片 GPU 化时，Skia 不再光栅到 CPU surface，而是直接合成到一块自建纹理
//! 上；GPU 命令流执行器的输出以原生纹理句柄零拷贝导入（经共享纹理表
//! [`crate::gpu::TextureRegistry`]），省掉每帧 GPU → CPU 的整帧
//! 往返。每帧末尾只读回一次交给编码器。
//!
//! WebGL2 命令由 ANGLE 执行，WebGPU 命令由 wgpu 执行。Skia Vulkan 在可用时
//! 共享 ANGLE 导出的 Vulkan device，并导入其输出 image。CPU raster 是显式
//! fallback；macOS Vulkan 离线合成仍未接入。

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::gpu::{NativeTexture, TextureId, TextureRegistry};
use anyhow::{Result, anyhow};
use skia_safe::gpu::SurfaceOrigin;
use skia_safe::{AlphaType, ColorType, gpu};
#[cfg(any(target_os = "windows", target_os = "linux"))]
use skia_safe::{ImageInfo, Surface};

/// 合成器持有的 GPU 资源。
///
/// 构造与销毁涉及 GPU 原生调用，须遵循所属设备的线程与生命周期规则。
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) struct GpuCompositor {
    /// Skia 的 direct context（离线 GPU 路径为 Vulkan）。
    skia: gpu::DirectContext,
    /// Vulkan 合成 surface（每帧复用）。
    surface: Surface,
    width: u32,
    height: u32,
    // 字段按声明顺序析构：先释放 Skia context 和 surface，再释放 Vulkan 会话。
    _session: crate::gpu::vk::VkSession,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl GpuCompositor {
    /// 建一个 `width × height` 的合成器。
    ///
    /// Skia 使用 `session` 的 Vulkan device，自建合成 surface。
    pub(crate) fn new(session: crate::gpu::vk::VkSession, width: u32, height: u32) -> Result<Self> {
        let mut skia = gpu::direct_contexts::make_vulkan(session.backend(), None)
            .ok_or_else(|| anyhow!("Skia make_vulkan 失败"))?;
        let surface = Self::create_surface(&mut skia, width, height)?;

        Ok(Self {
            skia,
            surface,
            width,
            height,
            _session: session,
        })
    }

    fn create_surface(skia: &mut gpu::DirectContext, width: u32, height: u32) -> Result<Surface> {
        let info = ImageInfo::new(
            (width as i32, height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        gpu::surfaces::render_target(
            skia,
            gpu::Budgeted::Yes,
            &info,
            None,
            SurfaceOrigin::TopLeft,
            None,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("Skia Vulkan render_target 失败"))
    }

    /// 同时借出合成 canvas 与 Skia context：前者重放 DrawOp，后者导入 3D
    /// 纹理。二者是**不同字段**，所以可以并存（合成器整体的可变借用则不行）。
    ///
    /// `Canvas` 的可变借用是 Skia 内部可变性的产物。
    #[allow(invalid_reference_casting)]
    pub(crate) fn split(&mut self) -> (&mut skia_safe::Canvas, &mut gpu::DirectContext) {
        let canvas = unsafe {
            &mut *(self.surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
        };
        (canvas, &mut self.skia)
    }

    /// 提交 GPU 并**同步**读回本帧像素（无延迟）。
    pub(crate) fn finish_frame(&mut self) -> Result<Vec<u8>> {
        self.skia
            .flush_and_submit_surface(&mut self.surface, Some(gpu::SyncCpu::Yes));
        self.read_pixels()
    }

    fn read_pixels(&mut self) -> Result<Vec<u8>> {
        let info = ImageInfo::new(
            (self.width as i32, self.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let mut out = vec![0u8; self.width as usize * self.height as usize * 4];
        if !self
            .surface
            .read_pixels(&info, &mut out, self.width as usize * 4, (0, 0))
        {
            return Err(anyhow!("GPU surface read_pixels 失败"));
        }
        if std::env::var_os("OC_TRACE_COMPOSITOR").is_some() {
            let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1i32, -1i32);
            for y in 0..self.height as i32 {
                for x in 0..self.width as i32 {
                    let a = out[((y as usize * self.width as usize + x as usize) * 4) + 3];
                    if a != 0 {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x);
                        y1 = y1.max(y);
                    }
                }
            }
            eprintln!(
                "[compositor] surface={}x{} bbox={:?}",
                self.surface.width(),
                self.surface.height(),
                (x0, y0, x1, y1)
            );
        }
        Ok(out)
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Drop for GpuCompositor {
    fn drop(&mut self) {
        // Skia 先放弃资源，再释放它拥有的 surface。
        self.skia.release_resources_and_abandon();
    }
}

/// [`crate::render::frame::GpuImageResolver`] 实现：凭共享纹理表的凭据取原生句柄，
/// 导入为 Skia image。
///
/// 只借用 Skia 的 `DirectContext` 与共享登记处。
/// 纹理的存活由登记处保证：本帧合成完成后宿主才 `release` 凭据。
/// 供 Linux/macOS 桌面预览使用；Windows 离线路径用 VkResolver。
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub struct CompositorResolver<'a> {
    pub skia: &'a mut gpu::DirectContext,
    pub textures: &'a TextureRegistry,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl crate::render::frame::GpuImageResolver for CompositorResolver<'_> {
    fn import(
        &mut self,
        id: TextureId,
        width: u32,
        height: u32,
    ) -> anyhow::Result<skia_safe::Image> {
        let native = self
            .textures
            .resolve(id)
            .ok_or_else(|| anyhow!("纹理凭据 {id:?} 不在共享表里（已被归还？）"))?;
        import_texture(self.skia, native, width, height)
    }

    fn debug_sample(&mut self, image: &skia_safe::Image) -> Option<Vec<u8>> {
        let mut px = vec![0u8; 2 * 2 * 4];
        let ok = image.read_pixels_with_context(
            Some(&mut *self.skia),
            &skia_safe::ImageInfo::new(
                (2, 2),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Premul,
                None,
            ),
            &mut px,
            8,
            (960, 540),
            skia_safe::image::CachingHint::Allow,
        );
        eprintln!("[import] read_pixels ok={ok}");
        Some(px)
    }
}

/// 把一个原生纹理句柄导入为 Skia image（零拷贝）。
///
/// 纹理必须在 image 的整个采样期内存活——调用方保证
/// （凭据在本帧合成完成后再 `release`）。
/// 供 Linux/macOS 桌面预览使用；Windows Vulkan 路径走
/// `vk::import_stream_texture`。
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn import_texture(
    skia: &mut gpu::DirectContext,
    native: NativeTexture,
    width: u32,
    height: u32,
) -> Result<skia_safe::Image> {
    #[cfg(target_os = "linux")]
    let backend_tex = unsafe {
        let info = gpu::gl::TextureInfo {
            target: crate::gpu::gl::TEXTURE_2D,
            id: native.raw() as u32,
            format: crate::gpu::gl::RGBA8,
            ..Default::default()
        };
        gpu::backend_textures::make_gl(
            (width as i32, height as i32),
            gpu::Mipmapped::No,
            info,
            "gpu-stream-output",
        )
    };
    #[cfg(target_os = "macos")]
    let backend_tex = unsafe {
        let info = gpu::mtl::TextureInfo::new(native.raw() as gpu::mtl::Handle);
        gpu::backend_textures::make_mtl(
            (width as i32, height as i32),
            gpu::Mipmapped::No,
            &info,
            "gpu-stream-output",
        )
    };

    // GL 纹理是 bottom-left 原点（FBO 渲染行序自底向上）；Metal 是 top-left。
    // wgpu 时代写死 TopLeft 是对的（wgpu NDC 即 top-left），GL 执行器接手后
    // 必须按后端区分，否则合成帧竖直翻转（实测 tracer 全帧上下颠倒）。
    #[cfg(target_os = "linux")]
    let origin = SurfaceOrigin::BottomLeft;
    #[cfg(not(target_os = "linux"))]
    let origin = SurfaceOrigin::TopLeft;
    let img = gpu::images::borrow_texture_from(
        skia,
        &backend_tex,
        origin,
        ColorType::RGBA8888,
        // GL 执行器输出的物理字节 = framebuffer 原样：透明清屏 + src-alpha
        // 混合天然产生预乘内容（rgb=a·color），必须按 Premul 解读。曾按
        // Unpremul 直读（flex-threejs α=1 场景两语义等价，当时误判），对
        // 半透明内容（G2 柱 α≈0.95）会让 Skia 再预乘一次、白底合成后整体
        // 偏暗一档（实测 G2 蓝柱 35,137,255 → 34,131,243，逐位吻合双重
        // 预乘模型），改回 Premul。
        AlphaType::Premul,
        None,
    )
    .ok_or_else(|| anyhow!("borrow_texture_from 失败"))?;

    // ANGLE 下 Skia 对借入的 GL 纹理**画不出来**：`read_pixels` 能读回正确
    // 像素、同一位置的 `draw_rect` 正常落帧，只有 `draw_image` 静默无输出
    // （纹理内容、尺寸、alpha、裁剪、坐标均已排除；ANGLE 之前走 WGL 时
    // 无此问题）。绕法：借入后立刻 `TextureFromImage`——GPU 内一次拷贝把
    // 内容转成 Skia **自有**纹理，绘制不再经过 external 代理。曾在此处
    // 回读 8MB 再 `raster_from_data`（绘制时又上传回 GPU）：同样的字节、
    // 同样的 origin 处理，但一次 GPU→CPU→GPU 双往返；全 GPU 化后这里
    // 必须保持零 CPU。
    gpu::images::texture_from_image(skia, &img, gpu::Mipmapped::No, gpu::Budgeted::Yes)
        .ok_or_else(|| anyhow!("texture_from_image 失败（{width}x{height}）"))
}
