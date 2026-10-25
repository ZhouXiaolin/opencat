//! Skia GPU 合成器（Windows=WGL / Linux=EGL 走 GL；macOS 走 Metal）。
//!
//! 全片 GPU 化时，Skia 不再光栅到 CPU surface，而是直接合成到一块自建纹理
//! 上；GPU 命令流执行器的输出以原生纹理句柄零拷贝导入（经共享纹理表
//! [`opencat_gpu_native::TextureRegistry`]），省掉每帧 GPU → CPU 的整帧
//! 往返。每帧末尾只读回一次交给编码器。
//!
//! ## 两个平台只差三处
//!
//! 本模块只有三个平台分支，都用 `cfg` 收在函数体内，调用方（离线渲染、
//! 预览）完全无感知：
//!
//! | 环节 | GL | Metal |
//! |---|---|---|
//! | 建 `DirectContext` | 用 current 的 `HGLRC` 建（共享资源空间） | 用同一 `MTLDevice` 建 |
//! | 包渲染目标 | GL FBO（`gl::Target`） | MTLTexture（`metal::Target`） |
//! | 导入 3D 纹理 | `make_gl` | `make_mtl` |
//!
//! ## GL 上下文纪律
//!
//! GL 上下文非线程安全。离线渲染的合成器、命令流执行器与裸 GL 操作都在
//! 拥有上下文的同一线程上串行执行（`GlSession` 建与用同线程），每帧前
//! `make_current` 一次即可；无锁。

use anyhow::{Result, anyhow};
use opencat_gpu_native::{NativeTexture, TextureId, TextureRegistry};
use skia_safe::gpu::SurfaceOrigin;
use skia_safe::{AlphaType, ColorType, ImageInfo, Surface, gpu};

/// 合成器持有的 GPU 资源。
///
/// 构造与销毁涉及原生调用（FBO / staging buffer），必须在 GL 上下文
/// current 的线程上进行；其余方法同。
/// 合成目标纹理由调用方建（[`opencat_gpu_native::gl::create_texture_rgba8`]）
/// 并自管生命周期；FBO 归 [`opencat_gpu_native::Target`]。
pub struct GpuCompositor {
    /// Skia 的 direct context（建在 current 的 GL 上下文上）。
    skia: gpu::DirectContext,
    /// 包裹目标纹理的 Skia surface（每帧复用）。
    surface: Surface,
    /// 合成目标：原生纹理 + FBO + 双缓冲读回。
    target: opencat_gpu_native::Target,
    width: u32,
    height: u32,
}

impl GpuCompositor {
    /// 建一个 `width × height` 的合成器。
    ///
    /// `gl` 是共享的 glow 函数面（GL 平台）；调用线程必须已把上下文
    /// make current。`native` 由调用方在 current 上下文里建
    /// （[`opencat_gpu_native::gl::create_texture_rgba8`]）。`backend` 是
    /// 平台桥：GL 为空标记，Metal 交出 `MTLDevice` / `MTLCommandQueue`。
    pub fn new(
        backend: CompositorBackend,
        gl: Option<std::sync::Arc<glow::Context>>,
        native: NativeTexture,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let mut skia = backend.make_direct_context()?;
        let target = {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                opencat_gpu_native::Target::new(
                    gl.ok_or_else(|| anyhow!("GL 平台必须携带 glow 上下文"))?,
                    native,
                    width,
                    height,
                )?
            }
            #[cfg(target_os = "macos")]
            {
                let _ = gl;
                opencat_gpu_native::Target::new(native, width, height)?
            }
        };
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
/// 只借用 Skia 的 `DirectContext` 与共享登记处。
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
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let origin = SurfaceOrigin::BottomLeft;
    #[cfg(target_os = "macos")]
    let origin = SurfaceOrigin::TopLeft;
    gpu::images::borrow_texture_from(
        skia,
        &backend_tex,
        origin,
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
