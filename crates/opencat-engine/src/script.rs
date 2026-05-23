//! engine 端脚本运行器别名。
//!
//! 真正的调度流程在 `opencat_core::script::script_runner::ScriptRunner<C>`；
//! 本 crate 通过 `RqJsContext`（实现 `JsContext`）提供 rquickjs 后端。

pub use crate::js_context::RqJsContext;

pub type ScriptRunner = opencat_core::script::ScriptRunner<RqJsContext>;
pub type ScriptRuntimeCache = opencat_core::script::ScriptRuntimeCache<RqJsContext>;
