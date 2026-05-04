//! opencat-engine — 默认 host features 全开的渲染引擎。
//! 承载 IO / ffmpeg / quickjs / skia / 系统字体 / RenderSession。

#![cfg(feature = "host-default")]

pub mod fonts;
pub mod inspect;
pub mod jsonl_io;
pub mod resource;
pub mod runtime;
pub mod script;

// 转发 core 中位于 runtime/ 的纯算法模块
pub use opencat_core::runtime::analysis;
pub use opencat_core::runtime::annotation;
pub use opencat_core::runtime::fingerprint;
pub use opencat_core::runtime::invalidation;
pub use opencat_core::runtime::preflight_collect;
