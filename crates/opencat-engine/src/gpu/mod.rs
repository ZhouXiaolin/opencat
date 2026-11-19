//! Engine 的 GPU 全家：Vulkan 会话、合成器、命令流执行器。
//!
//! Linux/Windows 离线 Engine 的 backend 固定为 Vulkan。ANGLE/MozANGLE 服务 WebGL2 GL
//! 调用面，并固定落到 Vulkan device；wgpu 只服务 WebGPU，也固定落到
//! Vulkan。硬件 GPU 与 SwiftShader 是 device 选择。
//!
//! 原生侧管三件事：
//!
//! - **GL 调用面**：[`angle`] 创建 WebGL2 上下文；`glow` 从 current ANGLE
//!   上下文加载函数入口。它不是独立渲染 backend。
//! - **纹理登记**（[`TextureRegistry`]）：生产方交出「原生句柄 + 保活
//!   凭据」，消费方凭 [`TextureId`] 取用、归还。谁保活、谁归还在这一处定。
//! - **Vulkan 会话**（[`vk`]）：供 Skia 创建离屏合成目标。CPU raster 是显式
//!   fallback。macOS 离线 GPU 路径留待后续 Metal 平台批次。

#[cfg(target_os = "linux")]
pub mod egl;
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod gl;
// 当前 Linux/Windows 离线渲染的 Skia Vulkan 会话。
mod registry;
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod vk;
// WebGL2 调用面：动态加载 ANGLE（OC_ANGLE_DIR），ANGLE 自身选择 Vulkan
// device。当前仅 Linux/Windows 离线路径接入。
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod angle;
#[cfg(target_os = "windows")]
pub mod win32;

pub mod compositor;
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod session;
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

/// 平台 GPU 纹理句柄。
///
/// 该不透明值仅用于当前内部交接。Vulkan 离线路径通过 Vulkan image 句柄
/// 交接；预览 presenter 仍可能使用平台原生纹理。0 表示没有句柄。
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
/// `1` / `true`（旧写法）与任何其他值都保持开启。平台是否支持 GPU target
/// 由渲染目标工厂决定。
pub fn requested() -> bool {
    !matches!(
        std::env::var("OC_GPU_PIPELINE").as_deref(),
        Ok("0") | Ok("false") | Ok("off")
    )
}
