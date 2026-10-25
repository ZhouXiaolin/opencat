//! Skia GPU 合成器（Windows=WGL / Linux=EGL 走 GL；macOS 走 Metal）。
//!
//! 全片 GPU 化时，Skia 不再光栅到 CPU surface，而是直接合成到一块 3D 渲染
//! 器产出的纹理上；`ctx.three` 的输出以原生纹理句柄零拷贝导入（见
//! [`opencat_three::LazyRenderer3d::render_to_gpu`]），省掉每帧
//! `wgpu → CPU → Skia` 的整帧往返。每帧末尾只读回一次交给编码器。
//!
//! ## 两个平台只差三处
//!
//! 本模块只有三个平台分支，都用 `cfg` 收在函数体内，调用方（离线渲染、
//! 预览）完全无感知：
//!
//! | 环节 | GL | Metal |
//! |---|---|---|
//! | 建 `DirectContext` | 用 wgpu 的 `HGLRC` 建（共享资源空间） | 用同一 `MTLDevice` 建 |
//! | 包渲染目标 | GL FBO（`gl::Target`） | MTLTexture（`metal::Target`） |
//! | 导入 3D 纹理 | `make_gl` | `make_mtl` |
//!
//! ## 锁纪律（GL 平台的硬约束）
//!
//! wgpu 的 GL 后端用一把互斥锁串行化 context 访问，**1 秒超时后 panic**。
//! 因此：
//!
//! - 所有 wgpu 调用（建纹理、渲染）必须在**锁外**完成；
//! - Skia / 裸 GL 操作在 `LazyRenderer3d::run_with_gpu` 的闭包内完成；
//! - 二者**绝不嵌套**。离线渲染逐帧串行（3D → 合成），天然满足。
//!
//! Metal 没有 context 概念：wgpu 与 Skia 各持一条 MTLCommandQueue，共享同一
//! MTLDevice，`run_with_gpu` 直接调用闭包。跨队列顺序由两处保证：wgpu 在 3D
//! 出口等待 GPU 完成（见 `Renderer3d::render_to_texture`），Skia 读回前由
//! [`opencat_gpu_native::Target::wait_inflight`] 等上一帧 blit。

use anyhow::{Result, anyhow};
use opencat_gpu_native::{NativeTexture, TextureId, TextureRegistry};
use skia_safe::gpu::SurfaceOrigin;
use skia_safe::{AlphaType, ColorType, ImageInfo, Surface, gpu};

/// 合成器持有的 GPU 资源。
///
/// 构造与销毁涉及原生调用（FBO / staging buffer），必须在
/// `LazyRenderer3d::run_with_gpu` 的闭包内进行；其余方法同。
/// 合成目标纹理的所有权在 [`opencat_gpu_native::TextureRegistry`] 里，
/// 所以本模块不依赖 wgpu 类型。
pub struct GpuCompositor {
    /// Skia 的 direct context（建在 wgpu 的设备/上下文上）。
    skia: gpu::DirectContext,
    /// 包裹目标纹理的 Skia surface（每帧复用）。
    surface: Surface,
    /// 合成目标：原生纹理 + 双缓冲读回。两个平台同形。
    target: opencat_gpu_native::Target,
    width: u32,
    height: u32,
}

impl GpuCompositor {
    /// 建一个 `width × height` 的合成器。
    ///
    /// `native` 由调用方在**锁外**经
    /// [`opencat_three::LazyRenderer3d::make_composite_target`] 取得——本函数
    /// 只做需要原生上下文的 Skia 工作，必须在
    /// `opencat_three::LazyRenderer3d::run_with_gpu` 的闭包内调用。
    ///
    /// `backend` 是平台桥：GL 为空标记（用当前线程的 context），Metal 交出
    /// `MTLDevice` / `MTLCommandQueue`。
    pub fn new(
        backend: CompositorBackend,
        native: NativeTexture,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let mut skia = backend.make_direct_context()?;
        let target = opencat_gpu_native::Target::new(native, width, height)?;
        let surface = backend.wrap_target(&mut skia, &target, width, height)?;

        Ok(Self {
            skia,
            surface,
            target,
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

    /// 让 Skia 丢弃缓存的后端状态，下次绘制时重新查询。
    ///
    /// wgpu 的 3D pass 会改动 GL 状态（绑定、viewport、program……），而 Skia
    /// 的 `DirectContext` 缓存了它上次留下的状态。二者共享同一个 `HGLRC`，
    /// 所以每帧合成前必须让 Skia 失效缓存，否则它会按过期状态发包——在
    /// NVIDIA 驱动上表现为 `nvg lv64.dll` 里的访问违例。
    ///
    /// Metal 上没有共享的 GL 状态，调用无副作用（仍保留以统一调用点）。
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
    /// 本帧的像素只是**发起**读回就返回，GPU→CPU 的传输交给 DMA 引擎，与
    /// 下一帧的渲染重叠。所以这里交出的是上一帧的画面——调用方必须容忍一帧
    /// 延迟（末帧由 `encode_rgba_frames` 多渲染一帧排空）。首帧返回 `None`。
    pub fn finish_frame(&mut self) -> Result<Option<Vec<u8>>> {
        self.skia
            .flush_and_submit_surface(&mut self.surface, Some(gpu::SyncCpu::Yes));
        // 帧头等上一帧 blit：Metal 跨队列无隐式顺序，不等就会与 Skia 本帧
        // 对同一纹理的重绘并发（GL 靠 context FIFO 天然免掉这一步）。
        self.target.wait_inflight();
        if self.target.has_readback() {
            // 取回上一帧 + 发起本帧。读回路径只把传输移出关键路径，帧序不变。
            let out = self.target.readback();
            // GL 上裸读回解绑了 FBO，Skia 的状态缓存却仍以为它绑着——下一帧
            // 会把 DrawOp 发到空目标上（实测表现为 Skia 读回全零、画面冻结
            // 在旧帧）。读回后必须让 Skia 丢弃缓存，重新查询绑定。
            self.skia.reset(None);
            return Ok(out);
        }
        // 无读回入口：退回同步读回，帧延迟为 0。
        Ok(Some(self.read_pixels()?))
    }

    /// 提交 GPU 并**同步**读回本帧像素（无延迟）。
    ///
    /// 单帧 / PNG 序列这类一次一帧的调用方用它——双缓冲的一帧延迟会打乱
    /// 它们的输出。逐帧循环请改用 [`Self::finish_frame`]。
    pub fn finish_frame_blocking(&mut self) -> Result<Vec<u8>> {
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

/// 平台桥：把「建 DirectContext / 包渲染目标 / 导入纹理」三件事分派到 GL
/// 或 Metal。
///
/// 这是本模块唯一的 `cfg` 集中处。两个变体的构造函数在调用方的 `cfg`
/// 分支里创建（GL 需要 `&Renderer3d` 上锁，Metal 只需要两个裸句柄）。
pub enum CompositorBackend {
    /// GL：`HGLRC` 由 wgpu 持有，Skia 共享同一上下文——入口从当前线程解析
    /// （调用方保证 context 已 current）。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Gl,
    /// Metal：一块系统 MTLDevice，wgpu 与 Skia 各自的队列共享它。
    #[cfg(target_os = "macos")]
    Metal { device: u64, queue: u64 },
}

impl CompositorBackend {
    fn make_direct_context(&self) -> Result<gpu::DirectContext> {
        match self {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            Self::Gl => {
                let interface = {
                    #[cfg(target_os = "windows")]
                    {
                        gpu::interfaces::make_win()
                            .ok_or_else(|| anyhow!("Skia make_win interface 失败"))?
                    }
                    #[cfg(target_os = "linux")]
                    {
                        // 与 wgpu 的 EGL context 同一驱动栈；核心入口经 libGL
                        // dlsym，扩展经 eglGetProcAddress
                        // （见 opencat_gpu_native::egl::load_proc）。
                        gpu::gl::Interface::new_load_with(opencat_gpu_native::egl::load_proc)
                            .ok_or_else(|| {
                                anyhow!("Skia GL 接口构建失败（EGL/libGL 入口缺失？）")
                            })?
                    }
                };
                gpu::direct_contexts::make_gl(interface, None)
                    .ok_or_else(|| anyhow!("Skia make_gl 失败"))
            }
            #[cfg(target_os = "macos")]
            Self::Metal { device, queue } => {
                let backend = unsafe {
                    skia_safe::gpu::mtl::BackendContext::new(
                        *device as skia_safe::gpu::mtl::Handle,
                        *queue as skia_safe::gpu::mtl::Handle,
                    )
                };
                gpu::direct_contexts::make_metal(&backend, None)
                    .ok_or_else(|| anyhow!("Skia make_metal 失败"))
            }
        }
    }

    fn wrap_target(
        &self,
        skia: &mut gpu::DirectContext,
        target: &opencat_gpu_native::Target,
        width: u32,
        height: u32,
    ) -> Result<Surface> {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
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
        #[cfg(target_os = "macos")]
        let brt = unsafe {
            let info = gpu::mtl::TextureInfo::new(target.texture().raw() as gpu::mtl::Handle);
            gpu::backend_render_targets::make_mtl((width as i32, height as i32), &info)
        };
        #[cfg(not(any(
            target_os = "windows",
            target_os = "linux",
            target_os = "macos"
        )))]
        let brt = unreachable!();

        gpu::surfaces::wrap_backend_render_target(
            skia,
            &brt,
            SurfaceOrigin::TopLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or_else(|| anyhow!("wrap_backend_render_target 失败"))
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
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let backend_tex = unsafe {
        let info = gpu::gl::TextureInfo {
            target: opencat_gpu_native::gl::TEXTURE_2D,
            id: native.raw() as u32,
            format: opencat_gpu_native::gl::RGBA8,
            ..Default::default()
        };
        gpu::backend_textures::make_gl(
            (width as i32, height as i32),
            gpu::Mipmapped::No,
            info,
            "opencat-three-output",
        )
    };
    #[cfg(target_os = "macos")]
    let backend_tex = unsafe {
        let info = gpu::mtl::TextureInfo::new(native.raw() as gpu::mtl::Handle);
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
        // 实测（flex-threejs 帧 672）：Unpremul 时内圈 opacity 0.85 叶片
        // 对参考偏差 ≤2/255，Premul 反而整体 +27——内容含 rgb=a·color 的
        // 预乘痕迹（透明清屏 + src-alpha 混合），但 Skia 端按 Unpremul
        // 直读直绘恰好落在参考值上；语义待与 read_back CPU 路径统一后
        // 再定，先保持现状。
        AlphaType::Unpremul,
        None,
    )
    .ok_or_else(|| anyhow!("borrow_texture_from 失败"))
}
