//! 浏览器原生 JS 引擎后端：`JsContext` 的具体实现。
//!
//! Isolation note (issue #20): core owns one [`opencat_core::script::ScriptRealm`]
//! per pipeline and never rebinds dispatchers across runners. The web host holds
//! a single active pipeline (`WebRenderer`); replacing a design constructs a new
//! `WebJsContext`. Correctness does not depend on rebinding a shared
//! `globalThis` between drivers — drivers share one realm via per-driver globals.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value as JsonValue;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::MutationStore;

pub struct WebJsContext {
    store: Rc<RefCell<MutationStore>>,
    _dispatcher_handle: RefCell<Option<Closure<dyn FnMut(JsValue, JsValue) -> JsValue>>>,
    _natives_handles: RefCell<Vec<Closure<dyn FnMut(JsValue) -> JsValue>>>,
}

impl JsContext for WebJsContext {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            store: Rc::new(RefCell::new(MutationStore::default())),
            _dispatcher_handle: RefCell::new(None),
            _natives_handles: RefCell::new(Vec::new()),
        })
    }

    fn eval(&self, code: &str) -> anyhow::Result<()> {
        js_sys::eval(code).map_err(|e| anyhow::anyhow!("script eval: {}", js_err_to_string(&e)))?;
        Ok(())
    }

    fn set_ctx_field(&self, name: &str, v: JsonValue) -> anyhow::Result<()> {
        let global = js_sys::global();
        let ctx = js_sys::Reflect::get(&global, &JsValue::from_str("ctx"))
            .map_err(|e| anyhow::anyhow!("get globalThis.ctx: {}", js_err_to_string(&e)))?;
        let js_val = json_to_js(&v)?;
        js_sys::Reflect::set(&ctx, &JsValue::from_str(name), &js_val)
            .map_err(|e| anyhow::anyhow!("set ctx.{}: {}", name, js_err_to_string(&e)))?;
        Ok(())
    }

    fn call_global_fn(&self, name: &str) -> anyhow::Result<()> {
        let global = js_sys::global();
        let f = js_sys::Reflect::get(&global, &JsValue::from_str(name))
            .map_err(|e| anyhow::anyhow!("get global.{}: {}", name, js_err_to_string(&e)))?;
        let func = f
            .dyn_ref::<js_sys::Function>()
            .ok_or_else(|| anyhow::anyhow!("global.{} is not a function", name))?;
        func.call0(&JsValue::UNDEFINED)
            .map_err(|e| anyhow::anyhow!("call {}: {}", name, js_err_to_string(&e)))?;
        Ok(())
    }

    fn install_dispatcher<F>(&self, dispatcher: F) -> anyhow::Result<()>
    where
        F: Fn(&mut MutationStore, &str, &[JsonValue]) -> anyhow::Result<JsonValue> + 'static,
    {
        let put_store = self.store.clone();
        let closure = Closure::wrap(
            Box::new(move |name_js: JsValue, args_js: JsValue| -> JsValue {
                let name = name_js.as_string().unwrap_or_default();
                let args = js_array_to_json(&args_js).unwrap_or_default();
                let mut guard = put_store.borrow_mut();
                match dispatcher(&mut guard, &name, &args) {
                    Ok(v) => json_to_js(&v).unwrap_or(JsValue::NULL),
                    Err(e) => {
                        web_sys::console::error_1(&JsValue::from_str(&e.to_string()));
                        JsValue::NULL
                    }
                }
            }) as Box<dyn FnMut(JsValue, JsValue) -> JsValue>,
        );

        let global = js_sys::global();
        js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatCallNative"),
            closure.as_ref().unchecked_ref(),
        )
        .map_err(|e| anyhow::anyhow!("install __opencatCallNative: {}", js_err_to_string(&e)))?;

        *self._dispatcher_handle.borrow_mut() = Some(closure);

        // Binary fast paths: typed arrays bypass the JSON dispatcher (the
        // engine registers the same two globals in
        // `crates/opencat-engine/src/js_context.rs`).
        let mut natives = self._natives_handles.borrow_mut();

        let put_store = self.store.clone();
        let put = Closure::wrap(Box::new(move |args: JsValue| -> JsValue {
            put_image_data(&put_store, &args);
            JsValue::UNDEFINED
        }) as Box<dyn FnMut(JsValue) -> JsValue>);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatPutImageData"),
            put.as_ref().unchecked_ref(),
        );
        natives.push(put);

        let read = Closure::wrap(Box::new(|args: JsValue| -> JsValue {
            surface_read_into(&args);
            JsValue::UNDEFINED
        }) as Box<dyn FnMut(JsValue) -> JsValue>);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatSurfaceReadInto"),
            read.as_ref().unchecked_ref(),
        );
        natives.push(read);

        Ok(())
    }

    fn rebind_dispatcher(&self) -> anyhow::Result<()> {
        // No-op: ScriptRealm installs one dispatcher for the lifetime of this
        // context. Multi-driver scheduling no longer relies on rebinding a
        // shared process global between runners (issue #20).
        Ok(())
    }

    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
        let mut guard = self.store.borrow_mut();
        f(&mut guard)
    }
}

// -- binary natives (arguments arrive as one JS array) --

fn arg(args: &JsValue, i: u32) -> JsValue {
    js_sys::Reflect::get_u32(args.unchecked_ref(), i).unwrap_or(JsValue::UNDEFINED)
}

fn num_arg(args: &JsValue, i: u32) -> f64 {
    arg(args, i).as_f64().unwrap_or(0.0)
}

fn str_arg(args: &JsValue, i: u32) -> String {
    arg(args, i).as_string().unwrap_or_default()
}

fn bytes_arg(args: &JsValue, i: u32) -> Option<js_sys::Uint8Array> {
    arg(args, i).dyn_into::<js_sys::Uint8Array>().ok()
}

/// `__opencatPutImageData(nodeId, key, pixels, x, y, w, h)`.
///
/// Registers the buffer as a frame-scoped generated image; the id is derived
/// from `key` so a reused pipeline sees the same id for the same key.
fn put_image_data(store: &Rc<RefCell<MutationStore>>, args: &JsValue) {
    let node_id = str_arg(args, 0);
    let key = str_arg(args, 1);
    let Some(pixels) = bytes_arg(args, 2) else {
        web_sys::console::error_1(&JsValue::from_str("putImageData: `pixels` must be a Uint8Array"));
        return;
    };
    let (x, y, w, h) = (
        num_arg(args, 3),
        num_arg(args, 4),
        num_arg(args, 5),
        num_arg(args, 6),
    );
    let id = opencat_core::ir::GeneratedImageId::from_key(&key);
    store.borrow_mut().record_frame_generated_image(
        &node_id,
        id,
        x as f32,
        y as f32,
        w as u32,
        h as u32,
        pixels.to_vec().into(),
    );
}

/// `__opencatSurfaceReadInto(id, x, y, w, h, out)` — fill `out` in place.
fn surface_read_into(args: &JsValue) {
    let id = str_arg(args, 0);
    let (x, y, w, h) = (
        num_arg(args, 1),
        num_arg(args, 2),
        num_arg(args, 3),
        num_arg(args, 4),
    );
    let Some(out) = bytes_arg(args, 5) else {
        web_sys::console::error_1(&JsValue::from_str("surface.read: `out` must be a Uint8Array"));
        return;
    };
    match opencat_core::text::surface::surface_get_rgba(&id, x, y, w, h) {
        Ok(bytes) => {
            let len = bytes.len().min(out.length() as usize);
            out.set(&js_sys::Uint8Array::from(&bytes[..len]), 0);
        }
        Err(e) => web_sys::console::error_1(&JsValue::from_str(&format!("surface.read: {e}"))),
    }
}

// -- helpers --

fn js_err_to_string(v: &JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{:?}", v))
}

fn json_to_js(v: &JsonValue) -> anyhow::Result<JsValue> {
    let s = serde_json::to_string(v)?;
    js_sys::JSON::parse(&s).map_err(|e| anyhow::anyhow!("json_to_js: {}", js_err_to_string(&e)))
}

fn js_array_to_json(v: &JsValue) -> anyhow::Result<Vec<JsonValue>> {
    if v.is_undefined() || v.is_null() {
        return Ok(Vec::new());
    }
    let s: String = js_sys::JSON::stringify(v)
        .map(|js_string| js_string.as_string().unwrap_or_else(|| "[]".to_string()))
        .map_err(|e| anyhow::anyhow!("stringify args: {}", js_err_to_string(&e)))?;
    let parsed: JsonValue = serde_json::from_str(&s)?;
    match parsed {
        JsonValue::Array(arr) => Ok(arr),
        other => Ok(vec![other]),
    }
}
