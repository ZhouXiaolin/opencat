//! opencat-engine — 桌面渲染引擎。
//! 承载 IO / ffmpeg / quickjs / skia / 系统字体 / RenderSession。

pub mod backend;
pub mod codec;
pub mod fonts;
pub mod inspect;
pub mod jsonl_io;
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
pub use opencat_core::scene::primitives::{SrtEntry, caption, div, text};
pub use opencat_core::scene::script::ScriptDriver;
pub use opencat_core::scene::transition::{fade, timeline};

// Re-export engine types used in tests via `crate::` path
pub use crate::runtime::session::RenderSession;
