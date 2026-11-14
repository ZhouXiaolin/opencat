//! Engine 的 JS 域：V8 后端与 JSX 文本前端。
//!
//! 调度与隔离单位是 `opencat_core::script::ScriptRealm`（每 pipeline 一个
//! realm）；[`V8JsContext`] 实现 core 的 `JsContext` trait，提供 V8 后端
//! 原语。JSX 是唯一的文本前端，解析（extract）也在 V8 realm 里跑。

#[cfg(feature = "js-v8")]
mod v8;

#[cfg(feature = "js-quickjs")]
mod quickjs;

pub mod bridge;
mod js_context;
pub mod html;
pub mod jsx;
pub mod offscreen;
pub mod omp;

pub use js_context::EngineJsContext;

#[cfg(feature = "js-v8")]
pub use v8::V8JsContext;
