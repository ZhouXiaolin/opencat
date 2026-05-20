//! 浏览器原生 JS 引擎后端：`JsContext` 的具体实现。


use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value as JsonValue;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};

use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::MutationStore;

pub struct WebJsContext {
    store: Rc<RefCell<MutationStore>>,
    _dispatcher_handle: RefCell<Option<Closure<dyn FnMut(JsValue, JsValue) -> JsValue>>>,
}

impl JsContext for WebJsContext {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            store: Rc::new(RefCell::new(MutationStore::default())),
            _dispatcher_handle: RefCell::new(None),
        })
    }

    fn eval(&self, code: &str) -> anyhow::Result<()> {
        js_sys::eval(code)
            .map_err(|e| anyhow::anyhow!("script eval: {}", js_err_to_string(&e)))?;
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
        let store = self.store.clone();
        let closure = Closure::wrap(Box::new(
            move |name_js: JsValue, args_js: JsValue| -> JsValue {
                let name = name_js.as_string().unwrap_or_default();
                let args = js_array_to_json(&args_js).unwrap_or_default();
                let mut guard = store.borrow_mut();
                match dispatcher(&mut guard, &name, &args) {
                    Ok(v) => json_to_js(&v).unwrap_or(JsValue::NULL),
                    Err(e) => {
                        web_sys::console::error_1(&JsValue::from_str(&e.to_string()));
                        JsValue::NULL
                    }
                }
            },
        )
            as Box<dyn FnMut(JsValue, JsValue) -> JsValue>);

        let global = js_sys::global();
        js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatCallNative"),
            closure.as_ref().unchecked_ref(),
        )
        .map_err(|e| anyhow::anyhow!("install __opencatCallNative: {}", js_err_to_string(&e)))?;

        *self._dispatcher_handle.borrow_mut() = Some(closure);
        Ok(())
    }

    fn rebind_dispatcher(&self) -> anyhow::Result<()> {
        let handle = self._dispatcher_handle.borrow();
        if let Some(ref closure) = *handle {
            let global = js_sys::global();
            js_sys::Reflect::set(
                &global,
                &JsValue::from_str("__opencatCallNative"),
                closure.as_ref().unchecked_ref(),
            )
            .map_err(|e| anyhow::anyhow!("rebind __opencatCallNative: {}", js_err_to_string(&e)))?;
        }
        Ok(())
    }

    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
        let mut guard = self.store.borrow_mut();
        f(&mut guard)
    }
}

// -- helpers --

fn js_err_to_string(v: &JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{:?}", v))
}

fn json_to_js(v: &JsonValue) -> anyhow::Result<JsValue> {
    let s = serde_json::to_string(v)?;
    js_sys::JSON::parse(&s)
        .map_err(|e| anyhow::anyhow!("json_to_js: {}", js_err_to_string(&e)))
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
