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
pub use opencat_core::runtime::analysis;
pub use opencat_core::runtime::annotation;
pub use opencat_core::runtime::fingerprint;
pub use opencat_core::runtime::invalidation;
pub use opencat_core::runtime::preflight_collect;

// Re-export core types used in engine tests (via `crate::` path)
pub use opencat_core::frame_ctx::FrameCtx;
pub use opencat_core::scene::composition::Composition;
pub use opencat_core::scene::easing::Easing;
pub use opencat_core::scene::node::Node;
pub use opencat_core::scene::primitives::{SrtEntry, canvas, caption, div, image, text};
pub use opencat_core::scene::script::ScriptDriver;
pub use opencat_core::scene::transition::{fade, timeline};
pub use opencat_core::style::ColorToken;

// Re-export engine types used in tests via `crate::` path
// RenderSession is now a type alias for the core generic session monomorphised with EnginePlatform.
pub use crate::render::RenderSession;
