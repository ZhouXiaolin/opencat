//! Engine 的 JS 域：V8/QuickJS 后端与 HTML/JS dist 文档宿主。
//!
//! 调度与隔离单位是 `opencat_core::script::ScriptRealm`（每 pipeline 一个
//! realm）；[`V8JsContext`] 实现 core 的 `JsContext` trait，提供 V8 后端
//! 原语。作者侧 JSX/Remotion 源码先编译成 HTML/JS dist；这里只保留按
//! HTML 文档导入的单一入口。

#[cfg(feature = "js-v8")]
mod v8;

#[cfg(feature = "js-quickjs")]
mod quickjs;

pub mod bridge;
pub mod html;
mod js_context;
pub mod offscreen;
pub mod omp;

pub use js_context::EngineJsContext;

#[cfg(feature = "js-v8")]
pub use v8::V8JsContext;
