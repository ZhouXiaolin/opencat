//! Web 端脚本 realm 别名。
//!
//! 调度与隔离单位是 `opencat_core::script::ScriptRealm`（每 pipeline 一个 realm）。
//! `WebJsContext` 只实现 runtime 原语；正确性不依赖共享 `globalThis` 重绑。

pub use crate::js_context::WebJsContext;

pub type ScriptRealm = opencat_core::script::ScriptRealm<WebJsContext>;
