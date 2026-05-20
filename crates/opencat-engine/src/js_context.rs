//! engine 端 rquickjs 后端：`JsContext` 的具体实现。
//!
//! 本文件只关心"如何在 rquickjs 环境里实现 [`opencat_core::script::js_context::JsContext`] 的
//! 几个原语"——它不认识任何具体 binding 名字，也不展开 `for_each_binding!`。
//! 所有 binding 的派发集中在 core 的 [`opencat_core::script::dispatch::dispatch_binding`]，
//! 通过 [`RqJsContext::install_dispatcher`] 桥接到 JS 端的 `__opencatCallNative`。

use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{
    Array, Context, Ctx, Error as JsError, Exception, FromJs, Function, IntoJs, Object, Persistent,
    Runtime, Type, Value,
};
use serde_json::Value as JsonValue;

use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::MutationStore;

// ── Error mapping ────────────────────────────────────────────────────

pub(crate) fn map_js_result<T>(
    result: Result<T, JsError>,
    ctx: &Ctx<'_>,
    error_context: &str,
) -> anyhow::Result<T> {
    match result {
        Ok(value) => Ok(value),
        Err(JsError::Exception) => {
            let caught = ctx.catch();
            if let Ok(exception) = Exception::from_js(ctx, caught.clone()) {
                let message = exception
                    .message()
                    .unwrap_or_else(|| "uncaught JavaScript exception".to_string());
                let stack = exception.stack().unwrap_or_default();
                if stack.is_empty() {
                    anyhow::bail!("{error_context}: {message}");
                }
                anyhow::bail!("{error_context}: {message}\n{stack}");
            }
            anyhow::bail!("{error_context}: uncaught JavaScript exception");
        }
        Err(err) => Err(err.into()),
    }
}

// ── rquickjs::Value <-> serde_json::Value walker ─────────────────────

fn rq_to_json(v: &Value<'_>) -> anyhow::Result<JsonValue> {
    match v.type_of() {
        Type::Uninitialized | Type::Undefined | Type::Null => Ok(JsonValue::Null),
        Type::Bool => Ok(JsonValue::Bool(v.as_bool().unwrap())),
        Type::Int => Ok(JsonValue::Number(v.as_int().unwrap().into())),
        Type::Float => {
            let f = v.as_float().unwrap();
            serde_json::Number::from_f64(f)
                .map(JsonValue::Number)
                .ok_or_else(|| anyhow!("non-finite number: {f}"))
        }
        Type::String => {
            let s = v.as_string().unwrap().to_string()?;
            Ok(JsonValue::String(s))
        }
        Type::Array => {
            let arr = v.as_array().unwrap();
            let mut out = Vec::with_capacity(arr.len());
            for item in arr.iter::<Value>() {
                out.push(rq_to_json(&item?)?);
            }
            Ok(JsonValue::Array(out))
        }
        Type::Object => {
            let obj = v.as_object().unwrap();
            let mut map = serde_json::Map::new();
            for kv in obj.props::<String, Value>() {
                let (k, val) = kv?;
                map.insert(k, rq_to_json(&val)?);
            }
            Ok(JsonValue::Object(map))
        }
        other => Err(anyhow!("unsupported js value type: {}", other.as_str())),
    }
}

fn json_to_rq<'js>(ctx: &Ctx<'js>, v: JsonValue) -> anyhow::Result<Value<'js>> {
    match v {
        JsonValue::Null => Ok(Value::new_null(ctx.clone())),
        JsonValue::Bool(b) => Ok(b.into_js(ctx)?),
        JsonValue::Number(n) => {
            let f = n
                .as_f64()
                .ok_or_else(|| anyhow!("non-finite number in return value"))?;
            Ok(Value::new_number(ctx.clone(), f))
        }
        JsonValue::String(s) => Ok(s.into_js(ctx)?),
        JsonValue::Array(arr) => {
            let out = Array::new(ctx.clone())?;
            for (i, item) in arr.into_iter().enumerate() {
                out.set(i, json_to_rq(ctx, item)?)?;
            }
            Ok(out.into_value())
        }
        JsonValue::Object(map) => {
            let out = Object::new(ctx.clone())?;
            for (k, val) in map.into_iter() {
                out.set(k, json_to_rq(ctx, val)?)?;
            }
            Ok(out.into_value())
        }
    }
}

/// JS-side arg → `serde_json::Value`，让 `Function::new` 闭包以 `JsonArg` 收参。
struct JsonArg(JsonValue);

impl<'js> FromJs<'js> for JsonArg {
    fn from_js(_ctx: &Ctx<'js>, value: Value<'js>) -> Result<Self, JsError> {
        rq_to_json(&value)
            .map(JsonArg)
            .map_err(|e| JsError::new_from_js_message("script", "arg", &e.to_string()))
    }
}

/// `serde_json::Value` → JS-side return，让 dispatcher 闭包返回与 rquickjs 衔接。
struct JsonReturn(JsonValue);

impl<'js> IntoJs<'js> for JsonReturn {
    fn into_js(self, ctx: &Ctx<'js>) -> Result<Value<'js>, JsError> {
        json_to_rq(ctx, self.0)
            .map_err(|e| JsError::new_from_js_message("script", "ret", &e.to_string()))
    }
}

// ── RqJsContext ──────────────────────────────────────────────────────

pub struct RqJsContext {
    context: Context,
    store: Arc<Mutex<MutationStore>>,
    ctx_obj: Persistent<Object<'static>>,
    _runtime: Runtime,
}

impl JsContext for RqJsContext {
    fn new() -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store = Arc::new(Mutex::new(MutationStore::default()));

        let ctx_obj = context.with(|ctx| -> anyhow::Result<_> {
            let obj = Object::new(ctx.clone())?;
            ctx.globals().set("ctx", obj.clone())?;
            Ok(Persistent::save(&ctx, obj))
        })?;

        Ok(Self {
            context,
            store,
            ctx_obj,
            _runtime: runtime,
        })
    }

    fn eval(&self, code: &str) -> anyhow::Result<()> {
        self.context
            .with(|ctx| map_js_result(ctx.eval::<(), _>(code), &ctx, "script eval"))
    }

    fn set_ctx_field(&self, name: &str, v: JsonValue) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let obj = self.ctx_obj.clone().restore(&ctx)?;
            let rq = json_to_rq(&ctx, v)?;
            obj.set(name, rq)?;
            Ok(())
        })
    }

    fn call_global_fn(&self, name: &str) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let f: Function = ctx.globals().get(name)?;
            map_js_result(f.call::<(), ()>(()), &ctx, name)
        })
    }

    fn install_dispatcher<F>(&self, dispatcher: F) -> anyhow::Result<()>
    where
        F: Fn(&mut MutationStore, &str, &[JsonValue]) -> anyhow::Result<JsonValue> + 'static,
    {
        let store = self.store.clone();
        self.context.with(|ctx| -> anyhow::Result<()> {
            let f = Function::new(
                ctx.clone(),
                move |name: String, args_arg: JsonArg| -> Result<JsonReturn, JsError> {
                    let args: Vec<JsonValue> = match args_arg.0 {
                        JsonValue::Array(arr) => arr,
                        other => vec![other],
                    };
                    let mut guard = store.lock().map_err(|e| {
                        JsError::new_from_js_message("script", "lock", &e.to_string())
                    })?;
                    dispatcher(&mut guard, &name, &args).map(JsonReturn).map_err(|e| {
                        JsError::new_from_js_message("script", "script", &e.to_string())
                    })
                },
            )?;
            ctx.globals().set("__opencatCallNative", f)?;
            Ok(())
        })
    }

    fn rebind_dispatcher(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
        let mut guard = self.store.lock().expect("script store lock poisoned");
        f(&mut guard)
    }
}
