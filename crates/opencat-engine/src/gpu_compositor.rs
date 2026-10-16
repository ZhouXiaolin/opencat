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
//! - Skia / 裸 GL 操作在 [`opencat_three::LazyRenderer3d::with_context`]
//!   的锁内完成；
//! - 二者**绝不嵌套**。离线渲染逐帧串行（3D → 合成），天然满足。
//!
//! 本模块的所有 `&mut self` 方法都假定调用方已持 GL 锁。

use anyhow::{Result, anyhow};
use opencat_gpu_native::gl;
use opencat_gpu_native::{NativeTexture, TextureId, TextureRegistry};
use skia_safe::gpu::SurfaceOrigin;
use skia_safe::{AlphaType, ColorType, ImageInfo, Surface, gpu};

/// 合成器持有的 GPU 资源。
///
/// 构造与销毁涉及裸 GL 调用（FBO），必须在 GL 锁内进行；其余方法同。
/// 合成目标纹理的所有权在 [`opencat_gpu_native::TextureRegistry`] 里，
/// 所以本模块不依赖 wgpu 类型。
///
/// 字段顺序即析构顺序：Skia 先放弃资源，再解 surface，最后 `target` 删
/// FBO 与 PBO。
pub struct GpuCompositor {
    /// Skia 的 GL direct context（建在 wgpu 的 HGLRC 上）。
    skia: gpu::DirectContext,
    /// 包裹目标纹理的 Skia surface（每帧复用）。
    surface: Surface,
    /// 合成目标：原生纹理 + FBO + 双缓冲读回。
    target: gl::Target,
    /// 合成目标在共享纹理表里的凭据（归还凭据，见 `GpuImageHandle::token`）。
    target_token: TextureId,
    width: u32,
    height: u32,
}

impl GpuCompositor {
    /// 建一个 `width × height` 的合成器。
    ///
    /// `target_token` 与 `native` 由调用方在**锁外**经
    /// [`opencat_three::LazyRenderer3d::make_composite_target`] 取得——
    /// 本函数只做锁内的 Skia / 裸 GL 工作。
    pub fn new(
        target_token: TextureId,
        native: NativeTexture,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let interface =
            gpu::interfaces::make_win().ok_or_else(|| anyhow!("Skia make_win interface 失败"))?;
        let mut skia = gpu::direct_contexts::make_gl(interface, None)
            .ok_or_else(|| anyhow!("Skia make_gl 失败"))?;

        let target = gl::Target::new(native, width, height)?;

        let brt = gpu::backend_render_targets::make_gl(
            (width as i32, height as i32),
            0usize,
            8,
            gpu::gl::FramebufferInfo {
                fboid: target.fbo(),
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
            surface,
            target,
            target_token,
            width,
            height,
        })
    }

    /// 合成目标纹理的凭据（调用方据此在帧末释放）。
    pub fn target_token(&self) -> TextureId {
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

    /// 提交 GPU，并取回**上一帧**的像素。
    ///
    /// 本帧的像素只是**发起**读回到 PBO 就返回（约 0.1ms），GPU→CPU 的传输
    /// 交给 DMA 引擎，与下一帧的渲染重叠。所以这里交出的是上一帧的画面——
    /// 调用方必须容忍一帧延迟（末帧由 `encode_rgba_frames` 多渲染一帧排空）。
    ///
    /// 首帧没有在途读回可取，返回 `None`。
    pub fn finish_frame(&mut self) -> Result<Option<Vec<u8>>> {
        self.skia
            .flush_and_submit_surface(&mut self.surface, Some(gpu::SyncCpu::Yes));
        if self.target.has_readback() {
            // 取回上一帧 + 发起本帧。PBO 路径只把传输移出关键路径，帧序不变。
            let out = self.target.readback();
            // 裸 GL 读回解绑了 FBO，Skia 的状态缓存却仍以为它绑着——下一帧
            // 会把 DrawOp 发到空目标上（实测表现为 Skia 读回全零、画面冻结
            // 在旧帧）。读回后必须让 Skia 丢弃缓存，重新查询绑定。
            self.skia.reset(None);
            return Ok(out);
        }
        // 无 PBO 入口：退回同步读回，帧延迟为 0（首帧即返回本帧）。
        let mut out = vec![0u8; self.width as usize * self.height as usize * 4];
        let info = ImageInfo::new(
            (self.width as i32, self.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        if !self
            .surface
            .read_pixels(&info, &mut out, self.width as usize * 4, (0, 0))
        {
            return Err(anyhow!("GPU surface read_pixels 失败"));
        }
        Ok(Some(out))
    }

    /// 提交 GPU 并**同步**读回本帧像素（无延迟）。
    ///
    /// 单帧 / PNG 序列这类一次一帧的调用方用它——PBO 的一帧延迟会打乱它们
    /// 的输出。逐帧循环请改用 [`Self::finish_frame`]。
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
            return Err(anyhow!("GPU surface read_pixels 失败"));
        }
        Ok(out)
    }
}

impl Drop for GpuCompositor {
    fn drop(&mut self) {
        // Skia 先放弃资源，再解 surface、删 FBO——顺序不能反（surface 引用 FBO）。
        // 字段的 drop 顺序由声明顺序决定，所以这里只做 Skia 那一步。
        self.skia.release_resources_and_abandon();
    }
}

/// [`crate::consumer::GpuImageResolver`] 实现：凭共享纹理表的凭据取原生句柄，
/// 导入为 Skia image。
///
/// 只借用 Skia 的 `DirectContext` 与共享登记处——本类型不认识 opencat-three。
/// 纹理的存活由登记处保证：本帧合成完成后宿主才 `release` 凭据。
pub struct CompositorResolver<'a> {
    pub skia: &'a mut gpu::DirectContext,
    pub textures: &'a TextureRegistry,
}

impl crate::consumer::GpuImageResolver for CompositorResolver<'_> {
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
}

/// 把一个原生纹理句柄导入为 Skia image（零拷贝）。
///
/// 纹理必须在 image 的整个采样期内存活——调用方保证
/// （凭据在本帧合成完成后再 `release`）。
pub fn import_texture(
    skia: &mut gpu::DirectContext,
    native: NativeTexture,
    width: u32,
    height: u32,
) -> Result<skia_safe::Image> {
    let backend_tex = unsafe {
        let info = gpu::gl::TextureInfo {
            target: gl::TEXTURE_2D,
            id: native.raw() as u32,
            format: gl::RGBA8,
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
