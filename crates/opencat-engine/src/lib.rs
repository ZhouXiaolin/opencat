//! opencat-engine — 桌面渲染引擎。
//! 承载 IO / ffmpeg / quickjs / skia / 系统字体。
//!
//! Public surface is host-owned: open/render/inspect/execute against an
//! explicit lifecycle pipeline. Core internals are not re-exported for tests.

pub mod consumer;
pub mod executor;
pub mod fonts;
pub mod inspect;
pub mod js_context;
pub mod media;
pub mod pipeline;
pub mod probe;
pub mod render;
pub mod resource;
pub mod runtime;
pub mod script;
pub mod source_io;

pub use crate::consumer::execute_render_frame;
pub use crate::resource::loader::{EngineAssetHandle, EngineLoader};

/// Engine host that owns a lifecycle-opened core pipeline plus resource loader.
/// Render/audio code reads cached bytes off `host.loader`, never through the
/// core pipeline.
pub type EnginePipeline = crate::pipeline::EnginePipelineHost;

// Test-only paths so engine integration tests can use `crate::div` / etc.
// Not part of the production public API (issue #24).
#[cfg(test)]
pub(crate) use opencat_core::frame_ctx::FrameCtx;
#[cfg(test)]
pub(crate) use opencat_core::parse::composition::Composition;
#[cfg(test)]
pub(crate) use opencat_core::parse::easing::Easing;
#[cfg(test)]
pub(crate) use opencat_core::parse::node::Node;
#[cfg(test)]
pub(crate) use opencat_core::parse::primitives::{SrtEntry, canvas, caption, div, image, text};
#[cfg(test)]
pub(crate) use opencat_core::parse::transition::{fade, timeline};
#[cfg(test)]
pub(crate) use opencat_core::style::ColorToken;
