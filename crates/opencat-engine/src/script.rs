//! engine 端脚本运行器别名。
//!
//! 调度与隔离单位是 `opencat_core::script::ScriptRealm`（每 pipeline 一个 realm）。
//! 本 crate 通过 `RqJsContext`（实现 `JsContext`）提供 rquickjs 后端原语。
//!
//! 历史名 `ScriptRunner` / `ScriptRuntimeCache` 仍可用，但都收敛到同一 realm 模型。

pub use crate::js_context::RqJsContext;

pub type ScriptRealm = opencat_core::script::ScriptRealm<RqJsContext>;
pub type ScriptRunner = opencat_core::script::ScriptRunner<RqJsContext>;
pub type ScriptRuntimeCache = opencat_core::script::ScriptRuntimeCache<RqJsContext>;

