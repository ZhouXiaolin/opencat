//! Skia GPU 合成器（Windows / GL 后端）。
//!
//! 全片 GPU 化时，Skia 不再光栅到 CPU surface，而是在一块自建 GL FBO 上
//! 合成；`ctx.three` 的 3D 输出以 GL texture id 零拷贝导入（见
//! [`opencat_three::LazyRenderer3d::render_to_gpu`]），省掉每帧
//! `wgpu → CPU → Skia` 的整帧往返。每帧末尾只 `read_pixels` 一次交给编码器。
//!
//! ## 与 wgpu 的上下文共享
//!
//! Skia 用 wgpu 的 `HGLRC` 建 `DirectContext`（同一 GL context），因此两者
//! 共享同一资源空间——wgpu 建的纹理 Skia 可直接采样。
//!
//! ## 锁纪律（硬约束）
//!
//! wgpu 的 GL 后端用一把互斥锁串行化 context 访问，**1 秒超时后 panic**。
//! 因此：
//!
//! - 所有 wgpu 调用（建纹理、渲染）必须在**锁外**完成；
//! - Skia / 裸 GL 操作在 [`opencat_three::LazyRenderer3d::with_gl_context`]
//!   的锁内完成；
//! - 二者**绝不嵌套**。离线渲染逐帧串行（3D → 合成），天然满足。
//!
//! 本模块的所有 `&mut self` 方法都假定调用方已持 GL 锁。

use anyhow::{Result, anyhow};
use skia_safe::gpu::SurfaceOrigin;
use skia_safe::{AlphaType, ColorType, ImageInfo, Surface, gpu};

/// 合成器持有的 GPU 资源。
///
/// 构造与销毁涉及裸 GL 调用（FBO），必须在 GL 锁内进行；其余方法同。
/// 合成目标纹理的所有权留在 [`opencat_three::LazyRenderer3d`] 的纹理表里
/// （句柄 [`GpuImageHandle`]），所以本模块不依赖 wgpu 类型。
///
/// [`GpuImageHandle`]: opencat_core::ir::generated_image::GpuImageHandle
pub struct GpuCompositor {
    /// Skia 的 GL direct context（建在 wgpu 的 HGLRC 上）。
    skia: gpu::DirectContext,
    /// 合成目标在纹理表里的 token（归还凭据，见 `GpuImageHandle::token`）。
    target_token: u64,
    /// 绑到目标纹理的 FBO id（裸 GL 建，drop 时删）。
    fbo: u32,
    /// 包裹目标纹理的 Skia surface（每帧复用）。
    surface: Surface,
    width: u32,
    height: u32,
}

impl GpuCompositor {
    /// 建一个 `width × height` 的合成器。
    ///
    /// `target_token` 与 `tex_id` 由调用方在**锁外**经
    /// [`opencat_three::LazyRenderer3d::make_composite_target`] 取得——
    /// 本函数只做锁内的 Skia / 裸 GL 工作。
    pub fn new(target_token: u64, tex_id: u32, width: u32, height: u32) -> Result<Self> {
        let interface =
            gpu::interfaces::make_win().ok_or_else(|| anyhow!("Skia make_win interface 失败"))?;
        let mut skia = gpu::direct_contexts::make_gl(interface, None)
            .ok_or_else(|| anyhow!("Skia make_gl 失败"))?;

        let fbo = opencat_three::gl_share::create_framebuffer(tex_id)
            .ok_or_else(|| anyhow!("创建 FBO 失败（GL 入口缺失或 FBO 不完整）"))?;

        let brt = gpu::backend_render_targets::make_gl(
            (width as i32, height as i32),
            0usize,
            8,
            gpu::gl::FramebufferInfo {
                fboid: fbo,
                format: gpu::gl::Format::RGBA8.into(),
                ..Default::default()
            },
        );
        let surface = gpu::surfaces::wrap_backend_render_target(
            &mut skia,
            &brt,
            SurfaceOrigin::TopLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("wrap_backend_render_target 失败"))?;

        Ok(Self {
            skia,
            target_token,
            fbo,
            surface,
            width,
            height,
        })
    }

    /// 合成目标纹理的 token（调用方据此在帧末释放）。
    pub fn target_token(&self) -> u64 {
        self.target_token
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// 让 Skia 丢弃缓存的 GL 状态，下次绘制时重新查询。
    ///
    /// wgpu 的 3D pass 会改动 GL 状态（绑定、viewport、program……），而 Skia
    /// 的 `DirectContext` 缓存了它上次留下的状态。二者共享同一个 `HGLRC`，
    /// 所以每帧合成前必须让 Skia 失效缓存，否则它会按过期状态发包——在
    /// NVIDIA 驱动上表现为 `nvoglv64.dll` 里的访问违例。
    pub fn reset_backend_state(&mut self) {
        self.skia.reset(None);
    }

    /// 同时借出合成 canvas 与 Skia context：前者重放 DrawOp，后者导入 3D
    /// 纹理。二者是**不同字段**，所以可以并存（合成器整体的可变借用则不行）。
    ///
    /// `Canvas` 的可变借用是 Skia 内部可变性的产物（同 CPU 路径的
    /// `render_pipeline_frame_to_rgba`）。
    #[allow(invalid_reference_casting)]
    pub fn split(&mut self) -> (&mut skia_safe::Canvas, &mut gpu::DirectContext) {
        let canvas = unsafe {
            &mut *(self.surface.canvas() as *const skia_safe::Canvas as *mut skia_safe::Canvas)
        };
        (canvas, &mut self.skia)
    }

    /// 提交并同步 GPU，随后读回 CPU（每帧一次，交给编码器）。
    pub fn finish_frame(&mut self) -> Result<Vec<u8>> {
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
            return Err(anyhow!("GPU surface read_pixels 失败"));
        }
        Ok(out)
    }
}

impl Drop for GpuCompositor {
    fn drop(&mut self) {
        // Skia 先放弃资源，再删 FBO——顺序不能反（surface 仍引用 FBO）。
        self.skia.release_resources_and_abandon();
        if self.fbo != 0 {
            opencat_three::gl_share::delete_framebuffer(self.fbo);
        }
    }
}

/// [`crate::consumer::GpuImageResolver`] 实现：把 GPU 句柄里的裸 GL 名导入为
/// Skia image。
///
/// 只借用 Skia 的 `DirectContext`——句柄自带 GL 名，所以本类型不认识
/// opencat-three（也不查它的纹理表）。纹理的存活由生产方保证：本帧合成
/// 完成后宿主才调 `release_gpu_texture`。
pub struct CompositorResolver<'a> {
    pub skia: &'a mut gpu::DirectContext,
}

impl crate::consumer::GpuImageResolver for CompositorResolver<'_> {
    fn import(
        &mut self,
        texture: u32,
        width: u32,
        height: u32,
    ) -> anyhow::Result<skia_safe::Image> {
        import_texture_id(self.skia, texture, width, height)
    }
}

/// 把一个 GL texture id 导入为 Skia image（零拷贝）。
///
/// 纹理必须在 image 的整个采样期内存活——调用方保证
/// （`renderer.release_gpu_texture` 在本帧合成完成后再调）。
pub fn import_texture_id(
    skia: &mut gpu::DirectContext,
    tex_id: u32,
    width: u32,
    height: u32,
) -> Result<skia_safe::Image> {
    let backend_tex = unsafe {
        let info = gpu::gl::TextureInfo {
            target: 0x0DE1, // GL_TEXTURE_2D
            id: tex_id,
            format: 0x8058, // GL_RGBA8
            ..Default::default()
        };
        gpu::backend_textures::make_gl(
            (width as i32, height as i32),
            gpu::Mipmapped::No,
            info,
            "opencat-three-output",
        )
    };
    gpu::images::borrow_texture_from(
        skia,
        &backend_tex,
        SurfaceOrigin::TopLeft,
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    )
    .ok_or_else(|| anyhow!("borrow_texture_from 失败"))
}
