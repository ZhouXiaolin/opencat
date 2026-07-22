//! engine 端脚本 realm 别名。
//!
//! 调度与隔离单位是 `opencat_core::script::ScriptRealm`（每 pipeline 一个 realm）。
//! 本 crate 通过 `RqJsContext`（实现 `JsContext`）提供 rquickjs 后端原语。

pub use crate::js_context::RqJsContext;

pub type ScriptRealm = opencat_core::script::ScriptRealm<RqJsContext>;
