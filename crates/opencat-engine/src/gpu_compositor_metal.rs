//! Skia GPU 合成器（macOS = Metal 后端）。
//!
//! 与 [`crate::gpu_compositor`]（Windows=WGL / Linux=EGL）同构：Skia 不
//! 光栅到 CPU surface，而是直接合成到 wgpu 建的 Metal 纹理上；`ctx.three`
//! 的 3D 输出以 MTLTexture 零拷贝导入，省掉每帧整帧往返。每帧末尾只
//! `read_pixels` 一次交给编码器。
//!
//! ## 与 GL 版的差异
//!
//! Metal 没有"current context"概念：命令队列由 Skia 自己提交，锁纪律
//! 自然消失。GL 版靠 `with_context` 串行化的部分在这里不需要。
//!
//! 纹理所有权仍在 [`opencat_gpu_native::TextureRegistry`]：wgpu 纹理活着，
//! MTLTexture 就活着；Skia 的 `mtl::TextureInfo` 自己 retain 一份。

use anyhow::{Result, anyhow};
use opencat_gpu_native::NativeTexture;
use skia_safe::gpu::{self, SurfaceOrigin, mtl};
use skia_safe::{AlphaType, ColorType, ImageInfo, Surface};

/// Metal 后端的 GPU 合成器。
///
/// 字段顺序即析构顺序：Skia 先放弃资源，再解 surface。
pub struct MetalCompositor {
    skia: gpu::DirectContext,
    surface: Surface,
    width: u32,
    height: u32,
}

impl MetalCompositor {
    /// 建一个 `width × height` 的 Metal 合成器。
    ///
    /// `mtl_device` / `mtl_queue` 是 MTLDevice / MTLCommandQueue 的裸指针值
    /// （由 opencat-three 从 wgpu 的 hal 句柄取出）；`native` 是合成目标
    /// 纹理的 MTLTexture 裸指针值。
    ///
    /// 目标纹理的存活由共享登记处保证（调用方在帧末才 release 凭据），
    /// 所以这里不另持所有权——`mtl::TextureInfo` 会自己 retain。
    pub fn new(
        mtl_device: u64,
        mtl_queue: u64,
        native: NativeTexture,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        if mtl_device == 0 || mtl_queue == 0 {
            anyhow::bail!("Metal 设备/队列句柄为空");
        }
        if native.is_none() {
            anyhow::bail!("合成目标没有 MTLTexture 句柄");
        }

        let backend = unsafe {
            mtl::BackendContext::new(mtl_device as mtl::Handle, mtl_queue as mtl::Handle)
        };
        let mut skia = gpu::direct_contexts::make_metal(&backend, None)
            .ok_or_else(|| anyhow!("Skia make_metal 失败"))?;

        // SAFETY: `native` 是本帧合成目标的 MTLTexture，存活期由登记处
        // 覆盖；`TextureInfo::new` 自己 retain，取出后即可独立持有。
        let info = unsafe { mtl::TextureInfo::new(native.raw() as mtl::Handle) };
        let brt = gpu::backend_render_targets::make_mtl((width as i32, height as i32), &info);
        let surface = gpu::surfaces::wrap_backend_render_target(
            &mut skia,
            &brt,
            SurfaceOrigin::TopLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("wrap_backend_render_target 失败（Metal）"))?;

        Ok(Self {
            skia,
            surface,
            width,
            height,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// 让 Skia 丢弃缓存的 GPU 状态。
    ///
    /// wgpu 的 3D pass 与 Skia 共享同一批资源；Metal 没有全局状态可失效，
    /// 但 Skia 内部仍缓存资源绑定，每帧合成前重置更稳妥（与 GL 版对齐）。
    pub fn reset_backend_state(&mut self) {
        self.skia.reset(None);
    }

    /// 同时借出合成 canvas 与 Skia context（二者是不同字段）。
    #[allow(invalid_reference_casting)]
    pub fn split(&mut self) -> (&mut skia_safe::Canvas, &mut gpu::DirectContext) {
        let canvas = unsafe {
            &mut *(self.surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
        };
        (canvas, &mut self.skia)
    }

    /// 提交 GPU 并同步读回本帧像素。
    ///
    /// Metal 后端不走 PBO 双缓冲（Skia 自己管理 command buffer），所以这里
    /// 直接返回本帧，无延迟。
    pub fn finish_frame(&mut self) -> Result<Option<Vec<u8>>> {
        self.finish_frame_blocking().map(Some)
    }

    /// 提交 GPU 并同步读回本帧像素。
    pub fn finish_frame_blocking(&mut self) -> Result<Vec<u8>> {
        self.skia
            .flush_and_submit_surface(&mut self.surface, Some(gpu::SyncCpu::Yes));
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
            return Err(anyhow!("Metal surface read_pixels 失败"));
        }
        Ok(out)
    }
}

impl Drop for MetalCompositor {
    fn drop(&mut self) {
        // Skia 先放弃资源，再解 surface（surface 引用它的 GPU 资源）。
        self.skia.release_resources_and_abandon();
    }
}

/// 把一个 MTLTexture 裸句柄导入为 Skia image（零拷贝）。
///
/// 纹理必须在 image 的整个采样期内存活——调用方保证（凭据在本帧合成完成
/// 后再 `release`）。
pub fn import_texture_metal(
    skia: &mut gpu::DirectContext,
    native: NativeTexture,
    width: u32,
    height: u32,
) -> Result<skia_safe::Image> {
    // SAFETY: `native` 是存活中的 MTLTexture；`TextureInfo::new` 自己 retain。
    let info = unsafe { mtl::TextureInfo::new(native.raw() as mtl::Handle) };
    let backend_tex = unsafe {
        gpu::backend_textures::make_mtl(
            (width as i32, height as i32),
            gpu::Mipmapped::No,
            &info,
            "opencat-three-output",
        )
    };
    gpu::images::borrow_texture_from(
        skia,
        &backend_tex,
        SurfaceOrigin::TopLeft,
        ColorType::RGBA8888,
        // 与 GL 版同口径（见 `gpu_compositor::import_texture` 的注释）：
        // 内容含预乘痕迹，但 Skia 端按 Unpremul 直读直绘才落在参考值上。
        AlphaType::Unpremul,
        None,
    )
    .ok_or_else(|| anyhow!("borrow_texture_from 失败（Metal）"))
}

/// [`crate::consumer::GpuImageResolver`] 实现（Metal 后端）：凭共享纹理表的
/// 凭据取 MTLTexture 裸句柄，导入为 Skia image。
///
/// 与 GL 版的 [`crate::gpu_compositor::CompositorResolver`] 同构，只换导入方式。
pub struct CompositorResolver<'a> {
    pub skia: &'a mut gpu::DirectContext,
    pub textures: &'a opencat_gpu_native::TextureRegistry,
}

impl crate::consumer::GpuImageResolver for CompositorResolver<'_> {
    fn import(
        &mut self,
        id: opencat_gpu_native::TextureId,
        width: u32,
        height: u32,
    ) -> anyhow::Result<skia_safe::Image> {
        let native = self
            .textures
            .resolve(id)
            .ok_or_else(|| anyhow!("纹理凭据 {id:?} 不在共享表里（已被归还？）"))?;
        import_texture_metal(self.skia, native, width, height)
    }
}
