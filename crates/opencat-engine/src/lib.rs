//! opencat-engine — 桌面渲染引擎。
//! 承载 IO / ffmpeg / quickjs / skia / 系统字体 / RenderSession。

pub mod codec;
pub mod consumer;
pub mod executor;
pub mod fonts;
pub mod inspect;
pub mod js_context;
pub mod media;
pub mod pipeline;
pub mod platform;
pub mod render;
pub mod resource;
pub mod runtime;
pub mod script;
pub mod source_io;

// 转发 core 中位于 runtime/ 的纯算法模块
pub use opencat_core::analyze::annotation;
pub use opencat_core::analyze::fingerprint;
pub use opencat_core::analyze::invalidation;
pub use opencat_core::parse::preflight as preflight_collect;

// Re-export core types used in engine tests (via `crate::` path)
pub use opencat_core::frame_ctx::FrameCtx;
pub use opencat_core::parse::composition::Composition;
pub use opencat_core::parse::easing::Easing;
pub use opencat_core::parse::node::Node;
pub use opencat_core::parse::primitives::{SrtEntry, canvas, caption, div, image, text};
pub use opencat_core::parse::transition::{fade, timeline};
pub use opencat_core::script::ScriptDriver;
pub use opencat_core::style::ColorToken;

// Re-export engine types used in tests via `crate::` path
pub use crate::consumer::execute_render_frame;
pub use crate::render::RenderSession;
pub use crate::resource::loader::{EngineAssetHandle, EngineLoader};

// Pipeline integration: the engine host that owns the loader-free core
// pipeline (opened via `open_with_prepared_catalog`) plus the engine resource
// owner. Render/audio code reads the cached bytes off `host.loader`, never
// through the core pipeline. See `pipeline::EnginePipelineHost` (#7 / #11).
pub type EnginePipeline = crate::pipeline::EnginePipelineHost;
