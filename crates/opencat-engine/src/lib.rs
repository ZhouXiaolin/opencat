//! opencat-engine — 桌面渲染引擎。
//! 承载 IO / ffmpeg / v8 / skia / 系统字体。
//!
//! Public surface is host-owned: open/render/inspect/execute against an
//! explicit lifecycle pipeline. Core internals are not re-exported for tests.

pub mod fonts;
// GPU 全家（原生 FFI 落点 / 离屏 GL 会话 / Skia 合成器 / WebGL 命令流执行器 /
// 用户开关）。macOS 的离线渲染走 CPU 路径，预览的 Metal 合成在 present 平台
// 模块里自建上下文，不经 gpu::session。
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
