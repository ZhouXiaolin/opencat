//! Web 端脚本运行器别名。


pub use crate::js_context::WebJsContext;

pub type ScriptRuntimeCache = opencat_core::scene::script::ScriptRuntimeCache<WebJsContext>;
