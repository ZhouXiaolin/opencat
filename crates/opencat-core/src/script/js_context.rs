//! 端侧 JS 运行环境的最小抽象。
//!
//! engine 用 rquickjs 实现；web wasm32 将用 js_sys + wasm-bindgen 实现。
//! core 的脚本调度流程（reset / set ctx / call run / call flush / snapshot）
//! 只通过本 trait 操作 JS 环境，不依赖任何具体 JS 引擎类型。

use crate::script::recorder::MutationStore;

pub trait JsContext: Sized {
    /// 构造一个全新的 JS 运行环境实例。
    fn new() -> anyhow::Result<Self>;

    /// 执行一段 JS 代码。
    fn eval(&self, code: &str) -> anyhow::Result<()>;

    /// 设置 `globalThis.ctx[name] = v`。
    fn set_ctx_field_i64(&self, name: &str, v: i64) -> anyhow::Result<()>;
    fn set_ctx_field_str(&self, name: &str, v: &str) -> anyhow::Result<()>;

    /// 调用一个无参、无返回值的全局函数 `globalThis[name]()`。
    fn call_global_fn(&self, name: &str) -> anyhow::Result<()>;

    /// 注册所有 native binding（`__record_* / __canvas_* / __animate_* / __text_*`）。
    fn install_all_bindings(&self) -> anyhow::Result<()>;

    /// 借出内部 MutationStore 让 core 流程读写。
    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R;
}
