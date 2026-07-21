//! 端侧 JS 运行环境的最小抽象。
//!
//! engine 用 rquickjs 实现；web wasm32 将用 js_sys + wasm-bindgen 实现。
//! core 的脚本调度（frame context / install / run / flush / mutation snapshot）
//! 集中在 [`crate::script::ScriptRealm`]，只通过本 trait 操作 JS 环境。
//!
//! **Isolation unit is one pipeline = one realm.** Each `JsContext::new()` must
//! yield a private environment. Correctness must not depend on rebinding a
//! process-wide `globalThis` when multiple pipelines coexist (issue #20).
//!
//! binding 的派发逻辑集中在 [`crate::script::dispatch::dispatch_binding`]；端侧
//! 只需通过 [`JsContext::install_dispatcher`] 把它桥接到约定的 native 入口
//! `__opencatCallNative(name, ...args)`。

use crate::script::recorder::MutationStore;

pub trait JsContext: Sized {
    /// 构造一个全新的、与其他 pipeline 隔离的 JS 运行环境实例。
    fn new() -> anyhow::Result<Self>;

    /// 执行一段 JS 代码。
    fn eval(&self, code: &str) -> anyhow::Result<()>;

    /// 设置 `globalThis.ctx[name] = v`。值通过 `serde_json::Value` 传递，端侧负责
    /// 翻译到具体 JS 引擎类型。
    fn set_ctx_field(&self, name: &str, v: serde_json::Value) -> anyhow::Result<()>;

    /// 调用一个无参、无返回值的全局函数 `globalThis[name]()`。
    fn call_global_fn(&self, name: &str) -> anyhow::Result<()>;

    /// 注册唯一的 native 入口（约定名 `__opencatCallNative`）。
    ///
    /// JS 端调用形如 `__opencatCallNative('record_opacity', id, v)` 时，端侧实现
    /// 应该：
    /// 1. 把 JS args 解码成 `Vec<serde_json::Value>`；
    /// 2. 取出 `&mut MutationStore`；
    /// 3. 调用 `dispatcher(store, name, &args)`；
    /// 4. 把返回值编码回端侧 JS Value。
    ///
    /// Dispatcher must stay bound to **this** context's store for the lifetime
    /// of the realm; multi-pipeline correctness must not require rebinding a
    /// shared process global.
    fn install_dispatcher<F>(&self, dispatcher: F) -> anyhow::Result<()>
    where
        F: Fn(&mut MutationStore, &str, &[serde_json::Value]) -> anyhow::Result<serde_json::Value>
            + 'static;

    /// Optional no-op for backends that already keep dispatcher private to the
    /// realm. Kept for expand-phase compatibility; new code should not rely on
    /// this for multi-pipeline isolation.
    fn rebind_dispatcher(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// 借出内部 MutationStore 让 core 流程读写。
    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R;
}
