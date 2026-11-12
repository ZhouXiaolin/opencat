//! Skia GPU 合成器（Windows=WGL / Linux=EGL 走 GL；macOS 走 Metal）。
//!
//! 全片 GPU 化时，Skia 不再光栅到 CPU surface，而是直接合成到一块自建纹理
//! 上；GPU 命令流执行器的输出以原生纹理句柄零拷贝导入（经共享纹理表
//! [`crate::gpu::TextureRegistry`]），省掉每帧 GPU → CPU 的整帧
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
use crate::gpu::{NativeTexture, TextureId, TextureRegistry};
use skia_safe::gpu::SurfaceOrigin;
use skia_safe::{AlphaType, ColorType, ImageInfo, Surface, gpu};

/// 合成器持有的 GPU 资源。
///
/// 构造与销毁涉及原生调用（FBO / staging buffer），必须在 GL 上下文
/// current 的线程上进行；其余方法同。
/// 合成目标纹理由调用方建（[`crate::gpu::gl::create_texture_rgba8`]）
/// 并自管生命周期；FBO 归 [`crate::gpu::Target`]。
pub struct GpuCompositor {
    /// Skia 的 direct context（建在 current 的 GL 上下文上）。
    skia: gpu::DirectContext,
    /// 包裹目标纹理的 Skia surface（每帧复用）。
    surface: Surface,
    /// 合成目标：原生纹理 + FBO + 双缓冲读回。Vulkan 变体为 None
    /// （Skia 自建渲染目标，无外部目标可持）。
    target: Option<crate::gpu::Target>,
    width: u32,
    height: u32,
}

impl GpuCompositor {
    /// 建一个 `width × height` 的合成器。
    ///
    /// `gl` 是共享的 glow 函数面（GL 平台）；调用线程必须已把上下文
    /// make current。`native` 由调用方在 current 上下文里建
    /// （[`crate::gpu::gl::create_texture_rgba8`]）。`backend` 是
    /// 平台桥：GL 为空标记，Metal 交出 `MTLDevice` / `MTLCommandQueue`。
    pub fn new(
        backend: CompositorBackend,
        gl: Option<std::sync::Arc<glow::Context>>,
        native: NativeTexture,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let mut skia = backend.make_direct_context()?;
        // Vulkan 变体：Skia 自建渲染目标（Vulkan 路径上不与宿主共享 image
        // 内存，无外部纹理可包装）。其余变体包装宿主的目标纹理。
        #[cfg(target_os = "linux")]
        let (target, surface) = match &backend {
            CompositorBackend::Vulkan { .. } => {
                let surface = backend.wrap_vulkan_target(&mut skia, width, height)?;
                (None, surface)
            }
            _ => {
                let target = crate::gpu::Target::new(
                    gl.ok_or_else(|| anyhow!("GL 平台必须携带 glow 上下文"))?,
                    native,
                    width,
                    height,
                )?;
                let surface = backend.wrap_target(&mut skia, &target, width, height)?;
                (Some(target), surface)
            }
        };
        #[cfg(not(target_os = "linux"))]
        let (target, surface) = {
            let target = {
                #[cfg(target_os = "macos")]
                {
                    let _ = gl;
                    crate::gpu::Target::new(native, width, height)?
                }
                #[cfg(target_os = "windows")]
                {
                    crate::gpu::Target::new(
                        gl.ok_or_else(|| anyhow!("GL 平台必须携带 glow 上下文"))?,
                        native,
                        width,
                        height,
                    )?
                }
            };
            let surface = backend.wrap_target(&mut skia, &target, width, height)?;
            (Some(target), surface)
        };

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

    /// 合成目标 FBO id（调试用）。
    pub fn fbo(&self) -> u32 {
        self.target.as_ref().map(|t| t.fbo()).unwrap_or(0)
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
        // 不能 SyncCpu::Yes：那会 glFinish 等本帧全部 GPU 工作排空，PBO 双
        // 缓冲的重叠就被打穿（实测 ANGLE/D3D11 每帧多 ~200ms 停顿）。这里只需
        // **提交**——GL context FIFO 保证 PBO 读回排在 Skia 绘制之后；上一帧
        // 的传输由 map(MAP_READ) 的完成语义兜底。
        self.skia
            .flush_and_submit_surface(&mut self.surface, Some(gpu::SyncCpu::No));
        // 帧头等上一帧 blit：Metal 跨队列无隐式顺序，不等就会与 Skia 本帧
        // 对同一纹理的重绘并发（GL 靠 context FIFO 天然免掉这一步）。
        // Vulkan 变体无外部目标（Skia 自建渲染目标），走同步读回。
        let Some(target) = self.target.as_mut() else {
            return Ok(Some(self.read_pixels()?));
        };
        target.wait_inflight();
        if target.has_readback() {
            // 取回上一帧 + 发起本帧。读回路径只把传输移出关键路径，帧序不变。
            let out = target.readback();
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
    /// GL：Skia 与命令流执行器共享同一上下文——GL 入口解析跟会话同源
    /// （ANGLE 用 eglGetProcAddress，WGL 用 wglGetProcAddress；调用方保证
    /// context 已 current）。
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    Gl {
        loader: std::sync::Arc<dyn Fn(&str) -> *const std::ffi::c_void + Send + Sync>,
    },
    /// Metal：一块系统 MTLDevice，wgpu 与 Skia 各自的队列共享它。
    #[cfg(target_os = "macos")]
    Metal { device: u64, queue: u64 },
    /// Vulkan（Linux）：Skia 的 Vulkan backend，设备来自 SwiftShader ICD。
    ///
    /// 与 GL 变体的关键差别：Skia 自建渲染目标（`render_target`），
    /// 不包装外部纹理——Vulkan 路径上 Skia 与宿主不共享同一份 image
    /// 内存，跨 API 零拷贝做不到（这正是统一到 Vulkan 后 ANGLE 的 GL
    /// 输出纹理进不来 Skia 的原因）。
    #[cfg(target_os = "linux")]
    Vulkan { session: std::sync::Arc<crate::gpu::vk::VkSession> },
}

impl CompositorBackend {
    fn make_direct_context(&self) -> Result<gpu::DirectContext> {
        match self {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            Self::Gl { loader } => {
                // 入口与会话同源（Windows：ANGLE→eglGetProcAddress / WGL→
                // wglGetProcAddress；Linux：EGL/libGL）。
                let interface = gpu::gl::Interface::new_load_with(|name| (loader)(name))
                    .ok_or_else(|| anyhow!("Skia GL 接口构建失败（会话入口缺失？）"))?;
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
            #[cfg(target_os = "linux")]
            Self::Vulkan { session } => gpu::direct_contexts::make_vulkan(session.backend(), None)
                .ok_or_else(|| anyhow!("Skia make_vulkan 失败")),
        }
    }

    fn wrap_target(
        &self,
        skia: &mut gpu::DirectContext,
        target: &crate::gpu::Target,
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

    /// Vulkan 变体：让 Skia 自建渲染目标。
    ///
    /// Vulkan 路径上没有「宿主纹理 + Skia 共享」这回事（跨 API 不能零拷贝
    /// 共享 image 内存），所以由 Skia 自己分配渲染目标，读回走
    /// `read_pixels`。
    #[cfg(target_os = "linux")]
    fn wrap_vulkan_target(
        &self,
        skia: &mut gpu::DirectContext,
        width: u32,
        height: u32,
    ) -> Result<Surface> {
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
}

/// [`crate::render::frame::GpuImageResolver`] 实现：凭共享纹理表的凭据取原生句柄，
/// 导入为 Skia image。
///
/// 只借用 Skia 的 `DirectContext` 与共享登记处。
/// 纹理的存活由登记处保证：本帧合成完成后宿主才 `release` 凭据。
pub struct CompositorResolver<'a> {
    pub skia: &'a mut gpu::DirectContext,
    pub textures: &'a TextureRegistry,
}

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
pub fn import_texture(
    skia: &mut gpu::DirectContext,
    native: NativeTexture,
    width: u32,
    height: u32,
) -> Result<skia_safe::Image> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
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
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let origin = SurfaceOrigin::BottomLeft;
    #[cfg(target_os = "macos")]
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
    // 无此问题）。零拷贝导入在这里不可用，改为一次性回读把纹理包成 raster
    // image——语义不变（同样的 Premul 字节、同样的 origin 处理），代价是
    // 每帧每 canvas 一次 8MB 回读。
    let mut px = vec![0u8; (width as usize) * (height as usize) * 4];
    let info = ImageInfo::new(
        (width as i32, height as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    if !img.read_pixels_with_context(
        Some(&mut *skia),
        &info,
        &mut px,
        (width as usize) * 4,
        (0, 0),
        skia_safe::image::CachingHint::Allow,
    ) {
        return Err(anyhow!("GPU 纹理回读失败（{width}x{height}）"));
    }
    let data = skia_safe::Data::new_copy(&px);
    skia_safe::images::raster_from_data(&info, data, (width as usize) * 4)
        .ok_or_else(|| anyhow!("raster_from_data 失败（{width}x{height}）"))
}
