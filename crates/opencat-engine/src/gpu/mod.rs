//! Engine 的 GPU 全家：原生 FFI 落点、离屏会话、合成器、命令流执行器。
//!
//! Skia 与 wgpu 是统一抽象，任何模块都可以直接用。原生 API 不是——
//! WGL/GL、Metal、EGL 只许出现在本模块的 [`gl`] / [`win32`] / [`egl`] /
//! [`metal`] 里，其余代码只见不透明句柄。
//!
//! 原生侧管三件事：
//!
//! - **上下文**（[`win32::WglContext`] / [`egl::EglContext`] / macOS 的
//!   [`metal::DeviceContext`]）：宿主从自己的窗口建出来，交给 wgpu 加入。
//!   两端共享同一个 context，才谈得上纹理零拷贝。Metal 没有 GL 式
//!   context——一块系统 MTLDevice 天然共享，[`metal::DeviceContext`] 只补
//!   一条读回自用队列。离线渲染没有窗口，GL 上下文由 wgpu 自建，不经过
//!   这里——但 GL 入口解析（[`egl::load_proc`]）两侧共用。
//! - **纹理登记**（[`TextureRegistry`]）：生产方交出「原生句柄 + 保活
//!   凭据」，消费方凭 [`TextureId`] 取用、归还。谁保活、谁归还在这一处定。
//! - **渲染目标**（[`gl::Target`] / [`metal::Target`]）：离屏目标的原生
//!   纹理、FBO/Metal staging 与双缓冲读回。两者接口同形，调用方经
//!   [`Target`] 别名按平台取得正确的那一个。
//!
//! 原生子模块只依赖 `std` 与平台 FFI——它们认识原生 API，不认识 wgpu，
//! 也不认识 Skia。
//!
//! 能力探测一律走返回值：入口缺失时 [`gl::create_framebuffer`] 与
//! [`gl::ReadbackPixels::new`] 返回 `None`，调用方据此回退。

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod gl;
#[cfg(target_os = "macos")]
pub mod metal;
#[cfg(target_os = "linux")]
pub mod egl;
// Vulkan 会话：ANGLE 共享 VkDevice（零拷贝）或独立 ICD 上的 Skia Vulkan
// backend。三栈统一（Skia/ANGLE/wgpu 全 Vulkan）的底座。
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod vk;
mod registry;
// ANGLE 后端：动态加载（OC_ANGLE_DIR），Vulkan 后端，三平台一致
// （官方 ANGLE Linux 构建，OC_ANGLE_DIR）——两端共用同一个 ANGLE 实现，
// WebGL 语义不随平台漂移。
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod angle;
#[cfg(target_os = "windows")]
pub mod win32;

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod session;
pub mod compositor;
pub mod stream_executor;
/// WebGPU 命令流执行器（`wgpu_opcode` → wgpu crate 一对一映射）。
pub mod wgpu_executor;

pub use registry::{TextureId, TextureRegistry};

// skia-bindings 的 Linux 构建把 `GrGLMakeEGLInterface.o` 带进了 rlib；它一旦
// 被拉入链接就需要 libGL/libEGL 的符号。最终二进制统一在这里声明链接。
#[cfg(target_os = "linux")]
#[link(name = "GL")]
#[link(name = "EGL")]
unsafe extern "C" {}

/// 离屏渲染目标：GL 上是 FBO + PBO 双缓冲，Metal 上是 staging buffer 双缓冲。
///
/// 两个平台的 `Target` 接口同形（`new` / `width` / `height` / `has_readback`
/// / `readback`，Metal 多一个帧头必须调的 `wait_inflight`），调用方只见本
/// 别名。
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub use backend::Target;

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub use gl as backend;
#[cfg(target_os = "macos")]
pub use metal as backend;

/// 后端原生纹理句柄。
///
/// Windows 上是 `GL_TEXTURE_2D` 名，Linux 上同是 GL 纹理名，macOS 上是
/// `MTLTexture` 指针。本类型只传不解释——解释它的代码在 [`gl`] 与未来的
/// `metal` 里。
///
/// 数值 0 表示「没有原生纹理」：非 GL 后端，或纹理尚未就绪。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeTexture(u64);

impl NativeTexture {
    /// 没有原生纹理。
    pub const NONE: Self = Self(0);

    /// 从裸值重建。生产方交出句柄时用。
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// 取裸值。句柄进 `GpuImageHandle` 时用。
    pub fn raw(self) -> u64 {
        self.0
    }

    /// 是否为空句柄。
    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

/// GPU 管线是否启用（用户开关）。
///
/// 默认开启。只有显式设 `OC_GPU_PIPELINE=0` / `false` / `off` 才走 CPU——
/// `1` / `true`（旧写法）与任何其他值都保持开启。非 GL/Metal 平台即使开着
/// 也走 CPU——否则 3D 会产出 GPU 句柄，而 CPU 合成器无法消费。
pub fn requested() -> bool {
    !matches!(
        std::env::var("OC_GPU_PIPELINE").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}
