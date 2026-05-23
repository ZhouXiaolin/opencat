//! opencat-engine — 桌面渲染引擎。
//! 承载 IO / ffmpeg / quickjs / skia / 系统字体 / RenderSession。

pub mod codec;
pub mod executor;
pub mod fonts;
pub mod inspect;
pub mod js_context;
pub mod jsonl_io;
pub mod media;
pub mod platform;
pub mod render;
pub mod resource;
pub mod runtime;
pub mod script;

// 转发 core 中位于 runtime/ 的纯算法模块
pub use opencat_core::parse::preflight as preflight_collect;
pub use opencat_core::analyze::annotation;
pub use opencat_core::analyze::fingerprint;
pub use opencat_core::analyze::invalidation;

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
// RenderSession is now a type alias for the core generic session monomorphised with EnginePlatform.
pub use crate::render::RenderSession;
pub use crate::resource::loader::{EngineAssetHandle, EngineLoader};
