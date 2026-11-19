//! opencat-engine — 桌面渲染引擎。
//! 承载 IO / ffmpeg / v8 / skia / 系统字体。
//!
//! Public surface is host-owned: open/render/inspect/execute against an
//! explicit lifecycle pipeline. Core internals are not re-exported for tests.

pub mod fonts;
// GPU 执行与呈现模块。Linux/Windows 离线 GPU 渲染固定使用 Vulkan：WebGL2
// 经 ANGLE，WebGPU 经 wgpu。macOS 离线 GPU 渲染留待后续 Metal 平台批次；桌面
// 预览仍由平台 presenter 使用原生图形 API。
pub mod gpu;
pub mod media;
pub mod pipeline;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub mod present;
pub mod probe;
pub mod render;
pub mod resource;
pub mod script;

pub use crate::render::frame::{ConsumerError, MediaError, execute_render_frame};
pub use crate::resource::loader::{EngineAssetHandle, EngineLoader};

/// Engine host that owns a lifecycle-opened core pipeline plus resource loader.
/// Render/audio code reads cached bytes off `host.loader`, never through the
/// core pipeline.
pub type EnginePipeline = crate::pipeline::EnginePipelineHost;
