//! engine 端 V8 后端：`JsContext` 的具体实现。
//!
//! 本文件只关心"如何在 V8 环境里实现 [`opencat_core::script::js_context::JsContext`]
//! 的几个原语"——它不认识任何具体 binding 名字，也不展开 `for_each_binding!`。
//! 所有 binding 的派发集中在 core 的
//! [`opencat_core::script::dispatch::dispatch_binding`]，通过
//! [`V8JsContext::install_dispatcher`] 桥接到 JS 端的 `__opencatCallNative`。
//!
//! ## 与 rquickjs 后端的差异
//!
//! - **隔离单元仍是「一个 pipeline 一个 isolate」。** V8 的 `OwnedIsolate`
//!   构造时自动 enter、drop 时断言自己是 `Isolate::GetCurrent()` 后 exit，
//!   即要求严格按创建逆序销毁。多 pipeline 共存时这个顺序不受控，所以本后端
//!   把 isolate 转成 `SharedIsolate`：访问经 `v8::Locker` 串行化，最后一个
//!   守卫消失时自行 dispose，realm 生命周期与创建顺序无关。
//! - **JSON 编组走 V8 原生 `JSON.stringify` / `JSON.parse`**，不再手写
//!   Value 遍历。原生实现快得多，且语义一致（`undefined` → `null`）。
//! - **原生函数是零捕获的 `fn` 项**（V8 回调要求 `Copy + Sized`），per-realm
//!   状态经 isolate data slot 取回。

use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use anyhow::anyhow;
use serde_json::Value as JsonValue;
use v8::{
    Context, Function, Global, Local, Object, PinScope, Script, SharedIsolate,
    String as V8String, Value,
};

use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::MutationStore;

// ── V8 platform lifecycle ────────────────────────────────────────────

/// Initialize the process-wide V8 platform exactly once. `V8::initialize` is
/// global state, so every isolate in the process shares it; it is never
/// disposed because realms come and go for the whole process lifetime.
fn ensure_v8_initialized() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

// ── Error mapping ────────────────────────────────────────────────────

/// Extract `message\nstack` from a live `TryCatch` and prefix `error_context`.
///
/// A macro rather than a function: `TryCatch`'s `GetIsolate` bound is
/// `pub(crate)` in the `v8` crate, so the type cannot be named in a signature
/// here. Expanding at each call site keeps the borrow of the surrounding scope
/// intact.
macro_rules! caught {
    ($tc:expr, $context:expr) => {{
        let tc = $tc;
        let exception = tc
            .exception()
            .map(|e| e.to_rust_string_lossy(tc))
            .unwrap_or_else(|| "uncaught JavaScript exception".to_string());
        let stack = tc
            .stack_trace()
            .map(|s| s.to_rust_string_lossy(tc))
            .unwrap_or_default();
        if stack.is_empty() {
            format!("{}: {}", $context, exception)
        } else {
            format!("{}: {}", $context, stack)
        }
    }};
}

/// Throw a JS exception carrying `message`. V8 requires the caller to return
/// immediately after scheduling an exception.
fn throw(scope: &mut PinScope<'_, '_>, message: &str) {
    if let Some(s) = V8String::new(scope, message) {
        scope.throw_exception(s.into());
    }
}

// ── JSON marshalling ─────────────────────────────────────────────────

/// `serde_json::Value` → JS value, via the engine's native JSON parser.
fn json_to_v8<'s>(scope: &mut PinScope<'s, '_>, v: &JsonValue) -> anyhow::Result<Local<'s, Value>> {
    let text = serde_json::to_string(v)?;
    let s = V8String::new(scope, &text).ok_or_else(|| anyhow!("v8: string allocation failed"))?;
    v8::json::parse(scope, s).ok_or_else(|| anyhow!("v8: JSON.parse failed for `{text}`"))
}

/// JS value → `serde_json::Value`. `undefined` (and any value JSON cannot
/// represent, e.g. a function) maps to `null`, matching the rquickjs backend's
/// walker for the argument shapes scripts actually pass.
fn v8_to_json(scope: &mut PinScope<'_, '_>, v: Local<Value>) -> anyhow::Result<JsonValue> {
    if v.is_undefined() {
        return Ok(JsonValue::Null);
    }
    let Some(s) = v8::json::stringify(scope, v) else {
        return Ok(JsonValue::Null);
    };
    let text = s.to_rust_string_lossy(scope);
    serde_json::from_str(&text).map_err(|e| anyhow!("v8: JSON round-trip failed: {e}"))
}

fn set_global<'s>(
    scope: &mut PinScope<'s, '_>,
    name: &str,
    value: Local<'s, Value>,
) -> anyhow::Result<()> {
    let key = V8String::new(scope, name).ok_or_else(|| anyhow!("v8: string allocation failed"))?;
    scope
        .get_current_context()
        .global(scope)
        .set(scope, key.into(), value)
        .ok_or_else(|| anyhow!("v8: failed to set global `{name}`"))?;
    Ok(())
}

// ── Per-realm native state ───────────────────────────────────────────

/// Binding dispatcher shape required by [`JsContext::install_dispatcher`].
type Dispatcher = Box<dyn Fn(&mut MutationStore, &str, &[JsonValue]) -> anyhow::Result<JsonValue>>;

/// Realm state reachable from a native callback. V8 callbacks must be plain
/// `fn` items (zero-sized), so this is stored in an isolate data slot and
/// recovered through [`PinScope::get_slot`].
///
/// `Rc`, not `Arc`: the render pipeline is single-threaded (the encoder runs
/// on its own thread and receives RGBA bytes over a channel), and the isolate
/// only ever hands this state to callbacks on the thread holding its lock.
struct NativeState {
    store: Rc<Mutex<MutationStore>>,
    dispatcher: Mutex<Option<Dispatcher>>,
}

fn native_state(scope: &PinScope<'_, '_>) -> Option<Rc<NativeState>> {
    scope.get_slot::<Rc<NativeState>>().cloned()
}

// ── Native entry points ──────────────────────────────────────────────

/// `__opencatCallNative(name, argsArray)` — the single generic binding entry.
fn call_native(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "script: realm native state missing");
        return;
    };
    let name = args.get(0).to_rust_string_lossy(scope);
    let args_json = match v8_to_json(scope, args.get(1)) {
        Ok(JsonValue::Array(arr)) => arr,
        Ok(other) => vec![other],
        Err(e) => {
            throw(scope, &format!("script: {e}"));
            return;
        }
    };
    let dispatcher = match state.dispatcher.lock() {
        Ok(guard) => guard,
        Err(_) => {
            throw(scope, "script: dispatcher lock poisoned");
            return;
        }
    };
    let Some(dispatcher) = dispatcher.as_ref() else {
        throw(scope, "script: dispatcher not installed");
        return;
    };
    let mut store = match state.store.lock() {
        Ok(guard) => guard,
        Err(_) => {
            throw(scope, "script: store lock poisoned");
            return;
        }
    };
    match dispatcher(&mut store, &name, &args_json) {
        Ok(value) => match json_to_v8(scope, &value) {
            Ok(v) => rv.set(v),
            Err(e) => throw(scope, &format!("script: {e}")),
        },
        Err(e) => throw(scope, &format!("script: {e}")),
    }
}

/// `__opencatPutImageData(nodeId, key, pixels, x, y, w, h)` — binary fast path.
///
/// Pixel payloads are pixel-sized (>1 MB per full frame) and would be
/// unworkable through the JSON dispatcher, so the `Uint8Array` is read
/// directly.
fn put_image_data(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    _rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "putImageData: realm native state missing");
        return;
    };
    let node_id = args.get(0).to_rust_string_lossy(scope);
    let key = args.get(1).to_rust_string_lossy(scope);
    let pixels: Local<v8::Uint8Array> = match args.get(2).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "putImageData: `pixels` must be a Uint8Array");
            return;
        }
    };
    let mut bytes = vec![0u8; pixels.byte_length()];
    let copied = v8::ArrayBufferView::copy_contents(&pixels, &mut bytes);
    bytes.truncate(copied);
    let number = |index: i32| {
        args.get(index)
            .to_number(scope)
            .map(|n| n.value())
            .unwrap_or(0.0)
    };
    let (x, y, w, h) = (number(3), number(4), number(5), number(6));

    // Deterministic id: same key ⇒ same id on fresh and reused pipelines
    // (GeneratedImageTable idempotency contract).
    let id = opencat_core::ir::GeneratedImageId::from_key(&key);
    let Ok(mut store) = state.store.lock() else {
        throw(scope, "putImageData: store lock poisoned");
        return;
    };
    store.record_frame_generated_image(
        &node_id,
        id,
        x as f32,
        y as f32,
        w as u32,
        h as u32,
        bytes.into(),
    );
}

/// `__opencatSurfaceReadInto(id, x, y, w, h, out)` — fill `out` in place.
///
/// Fills the caller's preallocated `Uint8Array` instead of returning a buffer:
/// the view is rooted on the call stack for the duration of the copy, so the
/// backing store cannot move.
fn surface_read_into(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let id = args.get(0).to_rust_string_lossy(scope);
    let number = |index: i32| {
        args.get(index)
            .to_number(scope)
            .map(|n| n.value())
            .unwrap_or(0.0)
    };
    let (x, y, w, h) = (number(1), number(2), number(3), number(4));
    let out: Local<v8::Uint8Array> = match args.get(5).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "surface.read: `out` must be a Uint8Array");
            return;
        }
    };
    let bytes = match opencat_core::text::surface::surface_get_rgba(&id, x, y, w, h) {
        Ok(bytes) => bytes,
        Err(e) => {
            throw(scope, &format!("surface.read: {e}"));
            return;
        }
    };
    if out.byte_length() < bytes.len() {
        throw(scope, "surface.read: destination Uint8Array smaller than region");
        return;
    }
    let destination = out.data() as *mut u8;
    // SAFETY: `out` is rooted by this call's arguments, so its backing store
    // stays alive (and cannot be detached) across the copy.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), destination, bytes.len());
    }
    rv.set_int32(bytes.len() as i32);
}

// ── V8JsContext ──────────────────────────────────────────────────────

/// V8-backed [`JsContext`]: one isolate + one context per pipeline.
///
/// The isolate is held as a [`SharedIsolate`] rather than an `OwnedIsolate`.
/// `OwnedIsolate` must be dropped in reverse order of creation — it asserts it
/// is `Isolate::GetCurrent()` before exiting — which a process running several
/// pipelines (each with its own realm) cannot guarantee. `SharedIsolate`
/// serializes access through a `v8::Locker` and disposes itself when the last
/// guard is gone, so realm lifetime is independent of creation order.
pub struct V8JsContext {
    isolate: SharedIsolate,
    context: Global<Context>,
    ctx_obj: Global<Object>,
    natives: Rc<NativeState>,
}

impl V8JsContext {
    /// Evaluate `code` and marshal the result back as JSON.
    ///
    /// Used by the JSX frontend host pass (transform + tree extraction), where
    /// the realm is throwaway and the only output is a value.
    pub fn eval_json(&self, code: &str) -> anyhow::Result<JsonValue> {
        self.with_scope(|scope| -> anyhow::Result<JsonValue> {
            let src =
                V8String::new(scope, code).ok_or_else(|| anyhow!("v8: string allocation failed"))?;
            v8::tc_scope!(let tc, scope);
            let Some(script) = Script::compile(tc, src, None) else {
                return Err(anyhow!(caught!(tc, "script eval")));
            };
            let Some(value) = script.run(tc) else {
                return Err(anyhow!(caught!(tc, "script eval")));
            };
            v8_to_json(tc, value)
        })
    }

    /// Lock this realm's isolate, run `f` inside its context, then release.
    ///
    /// The `v8::Locker` guard is dropped at the end of the call, so the isolate
    /// is only ever entered for the duration of one primitive.
    fn with_scope<R>(&self, f: impl for<'s, 'i> FnOnce(&mut PinScope<'s, 'i>) -> R) -> R {
        let mut locker = self.isolate.lock();
        v8::scope!(let handle_scope, &mut *locker);
        let context = Local::new(handle_scope, &self.context);
        let scope = &mut v8::ContextScope::new(handle_scope, context);
        f(scope)
    }
}

impl JsContext for V8JsContext {
    fn new() -> anyhow::Result<Self> {
        ensure_v8_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        let natives = Rc::new(NativeState {
            store: Rc::new(Mutex::new(MutationStore::default())),
            dispatcher: Mutex::new(None),
        });
        isolate.set_slot(natives.clone());

        // SAFETY: the isolate is entered by its own construction and this call
        // is the only one on the current thread, so nothing else is on top of
        // it. `try_into_shared` consumes it; the returned `SharedIsolate`
        // serializes every later access through its `Locker`.
        let isolate = unsafe { isolate.try_into_shared() }
            .map_err(|e| anyhow!("v8: isolate cannot be shared: {e}"))?;

        let (context, ctx_obj) = {
            let mut locker = isolate.lock();
            v8::scope!(let handle_scope, &mut *locker);
            let context = Context::new(handle_scope, Default::default());
            let scope = &mut v8::ContextScope::new(handle_scope, context);
            let obj = Object::new(scope);
            set_global(scope, "ctx", obj.into())?;
            (Global::new(scope, context), Global::new(scope, obj))
        };

        Ok(Self {
            isolate,
            context,
            ctx_obj,
            natives,
        })
    }

    fn eval(&self, code: &str) -> anyhow::Result<()> {
        self.with_scope(|scope| -> anyhow::Result<()> {
            let src =
                V8String::new(scope, code).ok_or_else(|| anyhow!("v8: string allocation failed"))?;
            v8::tc_scope!(let tc, scope);
            let Some(script) = Script::compile(tc, src, None) else {
                return Err(anyhow!(caught!(tc, "script eval")));
            };
            match script.run(tc) {
                Some(_) => Ok(()),
                None => Err(anyhow!(caught!(tc, "script eval"))),
            }
        })
    }

    fn set_ctx_field(&self, name: &str, v: JsonValue) -> anyhow::Result<()> {
        self.with_scope(|scope| -> anyhow::Result<()> {
            let obj = Local::new(scope, &self.ctx_obj);
            let key = V8String::new(scope, name)
                .ok_or_else(|| anyhow!("v8: string allocation failed"))?;
            let value = json_to_v8(scope, &v)?;
            obj.set(scope, key.into(), value)
                .ok_or_else(|| anyhow!("ctx.{name}: set failed"))?;
            Ok(())
        })
    }

    fn call_global_fn(&self, name: &str) -> anyhow::Result<()> {
        self.with_scope(|scope| -> anyhow::Result<()> {
            let global = scope.get_current_context().global(scope);
            let key = V8String::new(scope, name)
                .ok_or_else(|| anyhow!("v8: string allocation failed"))?;
            let value = global
                .get(scope, key.into())
                .ok_or_else(|| anyhow!("{name}: global lookup failed"))?;
            let function: Local<Function> = value
                .try_into()
                .map_err(|_| anyhow!("{name}: global is not a function"))?;
            v8::tc_scope!(let tc, scope);
            let receiver = v8::undefined(tc);
            function
                .call(tc, receiver.into(), &[])
                .map(|_| ())
                .ok_or_else(|| anyhow!(caught!(tc, name)))
        })
    }

    fn install_dispatcher<F>(&self, dispatcher: F) -> anyhow::Result<()>
    where
        F: Fn(&mut MutationStore, &str, &[JsonValue]) -> anyhow::Result<JsonValue> + 'static,
    {
        *self
            .natives
            .dispatcher
            .lock()
            .map_err(|_| anyhow!("script: dispatcher lock poisoned"))? = Some(Box::new(dispatcher));

        self.with_scope(|scope| -> anyhow::Result<()> {
            let call = Function::new(scope, call_native)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatCallNative"))?;
            set_global(scope, "__opencatCallNative", call.into())?;

            // Binary fast paths: typed arrays bypass the JSON dispatcher.
            let put = Function::new(scope, put_image_data)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatPutImageData"))?;
            set_global(scope, "__opencatPutImageData", put.into())?;

            let read = Function::new(scope, surface_read_into)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatSurfaceReadInto"))?;
            set_global(scope, "__opencatSurfaceReadInto", read.into())?;
            Ok(())
        })
    }

    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
        let mut guard = self.natives.store.lock().expect("script store lock poisoned");
        f(&mut guard)
    }
}
