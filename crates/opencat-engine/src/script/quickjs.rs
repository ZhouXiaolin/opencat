//! rquickjs backend for [`opencat_core::script::js_context::JsContext`].

use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use rquickjs::{
    Array, Context, Ctx, Error as JsError, Exception, Function, IntoJs, Object, Persistent,
    Runtime, Type, Value, function::Rest,
};
use serde_json::Value as JsonValue;

use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::MutationStore;

type Store = Arc<Mutex<MutationStore>>;

fn map_js_result<T>(result: Result<T, JsError>, ctx: &Ctx<'_>, label: &str) -> anyhow::Result<T> {
    result.map_err(|err| anyhow!("{label}: {err}"))
}

pub(crate) fn exception(ctx: &Ctx<'_>, label: &str) -> anyhow::Error {
    let caught = ctx.catch();
    if let Ok(object) = Object::from_value(caught.clone())
        && let Some(exception) = Exception::from_object(object)
    {
        let message = exception.message().unwrap_or_else(|| "uncaught exception".into());
        let stack = exception.stack().unwrap_or_default();
        return anyhow!("{label}: {message}\n{stack}");
    }
    anyhow!("{label}: uncaught JavaScript exception")
}

fn json_number(value: f64) -> JsonValue {
    if !value.is_finite() {
        return JsonValue::Null;
    }
    if value.fract() == 0.0 && value.abs() < 1e21 {
        if value >= 0.0 && value < u64::MAX as f64 {
            return JsonValue::from(value as u64);
        }
        if value < 0.0 && value >= i64::MIN as f64 {
            return JsonValue::from(value as i64);
        }
    }
    serde_json::Number::from_f64(value).map_or(JsonValue::Null, JsonValue::Number)
}

fn to_json(value: &Value<'_>) -> anyhow::Result<JsonValue> {
    match value.type_of() {
        Type::Uninitialized | Type::Undefined | Type::Null => Ok(JsonValue::Null),
        Type::Bool => Ok(JsonValue::Bool(value.as_bool().unwrap_or(false))),
        Type::Int => Ok(JsonValue::from(value.as_int().unwrap_or_default())),
        Type::Float => Ok(json_number(value.as_float().unwrap_or(f64::NAN))),
        Type::String => Ok(JsonValue::String(value.as_string().unwrap().to_string()?)),
        Type::Array => {
            let array = value.as_array().unwrap();
            let mut out = Vec::with_capacity(array.len());
            for item in array.iter::<Value>() {
                out.push(to_json(&item?)?);
            }
            Ok(JsonValue::Array(out))
        }
        Type::Object => {
            let object = value.as_object().unwrap();
            let mut out = serde_json::Map::new();
            for prop in object.props::<String, Value>() {
                let (key, item) = prop?;
                out.insert(key, to_json(&item)?);
            }
            Ok(JsonValue::Object(out))
        }
        _ => Ok(JsonValue::Null),
    }
}

pub(crate) fn from_json<'js>(ctx: &Ctx<'js>, value: JsonValue) -> anyhow::Result<Value<'js>> {
    match value {
        JsonValue::Null => Ok(Value::new_null(ctx.clone())),
        JsonValue::Bool(value) => Ok(value.into_js(ctx)?),
        JsonValue::Number(number) => Ok(Value::new_number(ctx.clone(), number.as_f64().unwrap_or_default())),
        JsonValue::String(value) => Ok(value.into_js(ctx)?),
        JsonValue::Array(items) => {
            let array = Array::new(ctx.clone())?;
            for (index, item) in items.into_iter().enumerate() {
                array.set(index, from_json(ctx, item)?)?;
            }
            Ok(array.into_value())
        }
        JsonValue::Object(items) => {
            let object = Object::new(ctx.clone())?;
            for (key, item) in items {
                object.set(key, from_json(ctx, item)?)?;
            }
            Ok(object.into_value())
        }
    }
}

fn arg_json(value: &Value<'_>) -> anyhow::Result<JsonValue> {
    match to_json(value)? {
        JsonValue::Array(items) => Ok(JsonValue::Array(items)),
        other => Ok(other),
    }
}

/// `__ocOmpTake(ir, ...args, skeleton)` 的宿主实现（v8.rs 同语义）：任何
/// 失败都回落 `false`（降级语义 =「pragma 未被编译器处理」），不抛异常。
/// true = 循环被旁路；reduction 终值非空时改返回折叠值数组（守卫赋
/// `__ocOmpRed`）。prng 实参是页面函数值，不做 JSON 化——骨架已按原序
/// 预采进末位的 Float64Array。
fn omp_take_native<'js>(
    ctx: &Ctx<'js>,
    store: &Store,
    args: &[Value<'_>],
) -> Result<Value<'js>, JsError> {
    let fallback = || Value::new_bool(ctx.clone(), false);
    let Some(ir_v) = args.first() else {
        return Ok(fallback());
    };
    let Ok(ir) = to_json(ir_v) else {
        return Ok(fallback());
    };
    let arg_names: Vec<String> = ir
        .get("args")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let captures = ir.get("captures");
    let kind = |name: &str| captures.and_then(|c| c.get(name)).and_then(|v| v.as_str());
    let n_args = arg_names.len();
    let mut rest: Vec<JsonValue> = Vec::with_capacity(n_args);
    for (k, name) in arg_names.iter().enumerate() {
        match kind(name) {
            Some("prng") => rest.push(JsonValue::Null), // 函数值不可 JSON 化
            Some("array") => {
                // 数组槽不过 isolate（copy-in 零字节）：实参以令牌替代，
                // 写回由 take 产出的 writes 应用到真数组。len 随令牌走。
                let len = args
                    .get(k + 1)
                    .and_then(|v| Object::from_value(v.clone()).ok())
                    .and_then(|o| o.get::<_, Value>("length").ok())
                    .and_then(|v| v.as_number())
                    .unwrap_or(0.0);
                if len <= 0.0 {
                    return Ok(fallback());
                }
                rest.push(serde_json::json!({ "__ocOmpArr": k, "len": len as u64 }));
            }
            Some("ctx") => {
                // html 通道的 ctx 是垫片 Proxy：Object::get 底层 JS_GetProperty
                // 走 [[Get]] 陷阱穿透到真 face。take 只需要这两个键。
                let node = args
                    .get(k + 1)
                    .and_then(|v| Object::from_value(v.clone()).ok())
                    .and_then(|o| o.get::<_, Value>("__ocNodeId").ok())
                    .and_then(|v| v.as_string().and_then(|s| s.to_string().ok()));
                let alpha = args
                    .get(k + 1)
                    .and_then(|v| Object::from_value(v.clone()).ok())
                    .and_then(|o| o.get::<_, Value>("state").ok())
                    .and_then(|v| Object::from_value(v).ok())
                    .and_then(|o| o.get::<_, Value>("globalAlpha").ok())
                    .and_then(|v| v.as_number())
                    .unwrap_or(1.0);
                match node {
                    Some(id) => rest.push(serde_json::json!({
                        "__ocNodeId": id,
                        "state": { "globalAlpha": alpha },
                    })),
                    None => rest.push(JsonValue::Null), // 读不到 = 毒化
                }
            }
            _ => {
                let Some(v) = args.get(k + 1) else {
                    return Ok(fallback());
                };
                let json = match v.type_of() {
                    Type::Uninitialized | Type::Undefined | Type::Null => Ok(JsonValue::Null),
                    Type::String => v
                        .as_string()
                        .and_then(|s| s.to_string().ok())
                        .map(JsonValue::String)
                        .ok_or_else(|| anyhow!("string arg")),
                    Type::Int | Type::Float => Ok(json_number(v.as_number().unwrap_or(f64::NAN))),
                    Type::Bool => Ok(JsonValue::Bool(v.as_bool().unwrap_or(false))),
                    _ => to_json(v),
                };
                match json {
                    Ok(j) => rest.push(j),
                    Err(_) => return Ok(fallback()),
                }
            }
        }
    }
    // 末位 = 采集骨架（Float64Array；无采集时是 null）。
    let mut prng: Vec<f64> = Vec::new();
    if let Some(skel) = args.get(n_args + 1)
        && let Ok(typed) = rquickjs::TypedArray::<f64>::from_value(skel.clone())
    {
        let bytes = unsafe { typed.as_bytes().unwrap_or_default() };
        prng = bytes
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect();
    }
    let mut writes: Vec<crate::script::omp::protocol::IsolateWrite> = Vec::new();
    let mut reds: Vec<f64> = Vec::new();
    let mut taken = match store.lock() {
        Ok(mut store) => {
            crate::script::omp::omp_take(&mut store, &ir, &rest, &prng, &mut writes, &mut reds)
        }
        Err(_) => false,
    };
    // isolate 腿写回：take=true 时按行序把 Set/Acc 四元组应用到真数组
    // （ES [[Get]]/[[Set]] 语义——JS Array 与 TypedArray 通吃）。此时骨架
    // 已旁路原循环、prng 已按行预采消耗，应用失败无法干净降级——失败
    // 路径只为防御（false = fallback 复跑原循环）。
    if taken {
        for w in &writes {
            let Some(obj) = args
                .get(w.arg + 1)
                .and_then(|v| Object::from_value(v.clone()).ok())
            else {
                taken = false;
                break;
            };
            let cur = if w.acc {
                match obj.get::<_, Value>(w.idx) {
                    Ok(v) => v.as_number().unwrap_or(0.0) + w.val,
                    Err(_) => {
                        taken = false;
                        break;
                    }
                }
            } else {
                w.val
            };
            if obj.set(w.idx, cur).is_err() {
                taken = false;
                break;
            }
        }
    }
    // reduction 面：take=true 且有 red 终值 → 返回折叠值数组（守卫把它
    // 赋给 `__ocOmpRed`，else 臂按 ir.reduction 展开序写回页面变量）。
    if taken && !reds.is_empty() {
        return Ok(from_json(ctx, serde_json::json!(reds)).unwrap_or_else(|_| fallback()));
    }
    Ok(Value::new_bool(ctx.clone(), taken))
}

fn string_arg(args: &[Value<'_>], index: usize) -> anyhow::Result<String> {
    args.get(index).ok_or_else(|| anyhow!("missing string argument {index}"))?
        .as_string().ok_or_else(|| anyhow!("argument {index} is not a string"))?
        .to_string().map_err(Into::into)
}

fn number_arg(args: &[Value<'_>], index: usize) -> f64 {
    args.get(index).and_then(|value| value.as_number()).unwrap_or_default()
}

fn u8_bytes(value: &Value<'_>) -> anyhow::Result<Vec<u8>> {
    let typed = rquickjs::TypedArray::<u8>::from_value(value.clone())?;
    unsafe { Ok(typed.as_bytes().unwrap_or_default().to_vec()) }
}

fn f32_bytes(value: &Value<'_>) -> anyhow::Result<Vec<u8>> {
    let typed = rquickjs::TypedArray::<f32>::from_value(value.clone())?;
    unsafe { Ok(typed.as_bytes().unwrap_or_default().to_vec()) }
}

fn f64_bytes(value: &Value<'_>) -> anyhow::Result<Vec<u8>> {
    let typed = rquickjs::TypedArray::<f64>::from_value(value.clone())?;
    unsafe { Ok(typed.as_bytes().unwrap_or_default().to_vec()) }
}

fn copy_out(value: &Value<'_>, bytes: &[u8]) -> anyhow::Result<()> {
    let raw = rquickjs::TypedArray::<u8>::from_value(value.clone())
        .map_err(|_| anyhow!("destination is not a Uint8Array"))?
        .as_raw()
        .ok_or_else(|| anyhow!("destination typed array is detached"))?;
    if raw.len() < bytes.len() {
        return Err(anyhow!("destination Uint8Array is smaller than payload"));
    }
    let destination = raw.as_ptr().cast::<u8>();
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), destination, bytes.len()) };
    Ok(())
}

fn put_image_data(store: &Store, args: &[Value<'_>]) -> anyhow::Result<()> {
    let node_id = string_arg(args, 0)?;
    let key = string_arg(args, 1)?;
    let bytes = u8_bytes(args.get(2).unwrap())?;
    let id = opencat_core::ir::GeneratedImageId::from_key(&key);
    let mut store = store.lock().map_err(|e| anyhow!("store lock: {e}"))?;
    store.record_frame_put_image(
        &node_id,
        id,
        number_arg(args, 3) as f32,
        number_arg(args, 4) as f32,
        number_arg(args, 5) as u32,
        number_arg(args, 6) as u32,
        bytes.into(),
    );
    Ok(())
}

fn surface_read_into(args: &[Value<'_>]) -> anyhow::Result<usize> {
    let id = string_arg(args, 0)?;
    let bytes = opencat_core::text::surface::surface_get_rgba(
        &id,
        number_arg(args, 1),
        number_arg(args, 2),
        number_arg(args, 3),
        number_arg(args, 4),
    )?;
    copy_out(args.get(5).unwrap(), &bytes)?;
    Ok(bytes.len())
}

fn raster_canvas(store: &Store, args: &[Value<'_>]) -> anyhow::Result<usize> {
    let id = string_arg(args, 0)?;
    let (width, height) = (number_arg(args, 1), number_arg(args, 2));
    let mut guard = store.lock().map_err(|e| anyhow!("store lock: {e}"))?;
    let empty = guard.canvas_commands(&id).is_none_or(|canvas| canvas.commands.is_empty());
    let (width, height, rgba) = if empty {
        (width.max(1.0) as u32, height.max(1.0) as u32, Vec::new())
    } else {
        let canvas = guard.canvas_commands(&id).cloned().unwrap();
        crate::script::offscreen::rasterize_canvas_commands(&canvas, width as f32, height as f32, &mut guard)
            .map_err(anyhow::Error::msg)?
    };
    copy_out(args.get(3).unwrap(), &rgba)?;
    let _ = width;
    Ok(rgba.len())
}

fn raster_canvas_png(store: &Store, args: &[Value<'_>]) -> anyhow::Result<usize> {
    let id = string_arg(args, 0)?;
    let (width, height) = (number_arg(args, 1), number_arg(args, 2));
    let mut guard = store.lock().map_err(|e| anyhow!("store lock: {e}"))?;
    let empty = guard.canvas_commands(&id).is_none_or(|canvas| canvas.commands.is_empty());
    let (_, _, bytes) = if empty {
        let w = width.max(1.0) as u32;
        let h = height.max(1.0) as u32;
        let png = crate::script::offscreen::encode_rgba_png(w, h, &vec![0; (w * h * 4) as usize])
            .unwrap_or_default();
        (w, h, png)
    } else {
        let canvas = guard.canvas_commands(&id).cloned().unwrap();
        crate::script::offscreen::rasterize_canvas_commands_png(&canvas, width as f32, height as f32, &mut guard)
            .map_err(anyhow::Error::msg)?
    };
    copy_out(args.get(3).unwrap(), &bytes)?;
    Ok(bytes.len())
}

pub struct RqJsContext {
    context: Context,
    store: Store,
    ctx_obj: Persistent<Object<'static>>,
    runtime: Runtime,
}

impl RqJsContext {
    pub fn eval_json(&self, code: &str) -> anyhow::Result<JsonValue> {
        self.context.with(|ctx| -> anyhow::Result<JsonValue> {
            let result: Result<Value, JsError> = ctx.eval(code);
            let value = map_js_result(result, &ctx, "script eval")?;
            to_json(&value)
        })
    }
}

impl JsContext for RqJsContext {
    fn new() -> anyhow::Result<Self> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;
        let store = Arc::new(Mutex::new(MutationStore::default()));
        let ctx_obj = context.with(|ctx| -> anyhow::Result<_> {
            let object = Object::new(ctx.clone())?;
            ctx.globals().set("ctx", object.clone())?;
            Ok(Persistent::save(&ctx, object))
        })?;
        Ok(Self { context, store, ctx_obj, runtime })
    }

    fn eval(&self, code: &str) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let result: Result<(), JsError> = ctx.eval(code);
            match result {
                Ok(()) => Ok(()),
                Err(JsError::Exception) => Err(exception(&ctx, "script eval")),
                Err(err) => Err(err.into()),
            }
        })
    }

    fn set_ctx_field(&self, name: &str, value: JsonValue) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let object = self.ctx_obj.clone().restore(&ctx)?;
            object.set(name, from_json(&ctx, value)?)?;
            Ok(())
        })
    }

    fn call_global_fn(&self, name: &str) -> anyhow::Result<()> {
        self.context.with(|ctx| -> anyhow::Result<()> {
            let function: Function = ctx.globals().get(name)?;
            match function.call::<(), ()>(()) {
                Ok(()) => Ok(()),
                Err(JsError::Exception) => Err(exception(&ctx, name)),
                Err(err) => Err(err.into()),
            }
        })
    }

    fn install_dispatcher<F>(&self, dispatcher: F) -> anyhow::Result<()>
    where
        F: Fn(&mut MutationStore, &str, &[JsonValue]) -> anyhow::Result<JsonValue> + 'static,
    {
        let store = self.store.clone();
        self.context.with(|ctx| -> anyhow::Result<()> {
            let native_store = store.clone();
            let dispatcher: Box<dyn Fn(&mut MutationStore, &str, &[JsonValue]) -> anyhow::Result<JsonValue>> =
                Box::new(dispatcher);
            // ctx 以 clone 被 move 进闭包（Ctx 注入参数是调用期 HRTB，与
            // 注册期 'js 的返回类型无法统一）——from_json 产物借用 clone 的
            // 生命周期 = 注册 'js，IntoJsFunc 才有具体返回类型。
            let native_ctx = ctx.clone();
            let native = move |name: String, args: Value| -> Result<Value, JsError> {
                let call_args = match arg_json(&args) {
                    Ok(JsonValue::Array(items)) => items,
                    Ok(other) => vec![other],
                    Err(e) => return Err(JsError::new_from_js_message("script", "args", &e.to_string())),
                };
                let mut guard = native_store.lock().map_err(|e| JsError::new_from_js_message("script", "lock", &e.to_string()))?;
                let out = dispatcher(&mut guard, &name, &call_args).map_err(|e| JsError::new_from_js_message("script", "script", &e.to_string()))?;
                from_json(&native_ctx, out).map_err(|e| JsError::new_from_js_message("script", "out", &e.to_string()))
            };
            ctx.globals().set("__opencatCallNative", Function::new(ctx.clone(), native)?)?;

            let globals = ctx.globals();
            let put_store = store.clone();
            globals.set("__opencatPutImageData", Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                put_image_data(&put_store, &args).map_err(|e| JsError::new_from_js_message("putImageData", "args", &e.to_string()))
            })?)?;

            let read = Function::new(ctx.clone(), |Rest(args): Rest<Value>| {
                surface_read_into(&args).map_err(|e| JsError::new_from_js_message("surface.read", "args", &e.to_string()))
            })?;
            globals.set("__opencatSurfaceReadInto", read)?;

            let raster_store = store.clone();
            let raster = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                raster_canvas(&raster_store, &args).map_err(|e| JsError::new_from_js_message("rasterCanvas", "args", &e.to_string()))
            })?;
            globals.set("__opencatRasterCanvas", raster)?;

            let png_store = store.clone();
            let png = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                raster_canvas_png(&png_store, &args).map_err(|e| JsError::new_from_js_message("rasterCanvasPNG", "args", &e.to_string()))
            })?;
            globals.set("__opencatRasterCanvasPNG", png)?;

            let reset_store = store.clone();
            let reset = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                let id = string_arg(&args, 0).map_err(|e| JsError::new_from_js_message("offscreenReset", "id", &e.to_string()))?;
                reset_store.lock().map_err(|e| JsError::new_from_js_message("offscreenReset", "lock", &e.to_string()))?.reset_offscreen_canvas(&id);
                Ok::<(), JsError>(())
            })?;
            globals.set("__opencatOffscreenReset", reset)?;

            let path_store = store.clone();
            let path = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                let id = string_arg(&args, 0).map_err(|e| JsError::new_from_js_message("canvas.pathFlat", "id", &e.to_string()))?;
                let flat = f64_bytes(args.get(1).unwrap())
                    .map_err(|e| JsError::new_from_js_message("canvas.pathFlat", "flat", &e.to_string()))?;
                let flat: Vec<f64> = flat.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect();
                let mut guard = path_store.lock().map_err(|e| JsError::new_from_js_message("canvas.pathFlat", "lock", &e.to_string()))?;
                opencat_core::script::bindings::record_path_flat(
                    &mut *guard,
                    &id,
                    &flat,
                    number_arg(&args, 2),
                    number_arg(&args, 3),
                ).map_err(|e| JsError::new_from_js_message("canvas.pathFlat", "record", &e.to_string()))
            })?;
            globals.set("__opencatCanvasPathFlat", path)?;

            let gl_store = store.clone();
            let gl = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                let id = string_arg(&args, 0).map_err(|e| JsError::new_from_js_message("gl.intern", "id", &e.to_string()))?;
                let bytes = u8_bytes(args.get(1).unwrap())
                    .map_err(|e| JsError::new_from_js_message("gl.intern", "bytes", &e.to_string()))?;
                gl_store.lock().map_err(|e| JsError::new_from_js_message("gl.intern", "lock", &e.to_string()))?
                    .gl_stream_intern(&id, bytes).map(|range| range as i32)
                    .map_err(|e| JsError::new_from_js_message("gl.intern", "record", &e.to_string()))
            })?;
            globals.set("__opencatGLIntern", gl)?;

            let stat_store = store.clone();
            let stat = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                let path = string_arg(&args, 0).map_err(|e| JsError::new_from_js_message("asset.stat", "path", &e.to_string()))?;
                Ok::<i32, JsError>(stat_store.lock().map_err(|e| JsError::new_from_js_message("asset.stat", "lock", &e.to_string()))?
                    .asset_read(&path).map(|bytes| bytes.len() as i32).unwrap_or(-1))
            })?;
            globals.set("__opencatAssetStat", stat)?;

            let asset_store = store.clone();
            let asset = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                let path = string_arg(&args, 0).map_err(|e| JsError::new_from_js_message("asset.read", "path", &e.to_string()))?;
                let bytes = asset_store.lock().map_err(|e| JsError::new_from_js_message("asset.read", "lock", &e.to_string()))?
                    .asset_read(&path).map_err(|e| JsError::new_from_js_message("asset.read", "bytes", &e.to_string()))?;
                copy_out(args.get(1).unwrap(), &bytes).map_err(|e| JsError::new_from_js_message("asset.read", "out", &e.to_string()))?;
                Ok::<i32, JsError>(bytes.len() as i32)
            })?;
            globals.set("__opencatAssetReadInto", asset)?;

            let decode_store = store.clone();
            let decode_ctx = ctx.clone();
            let decode = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| -> Result<Value, JsError> {
                let result = (|| -> anyhow::Result<Option<(u32, u32)>> {
                    let bytes = u8_bytes(args.first().unwrap())
                        .map_err(|e| anyhow!("src must be a Uint8Array: {e}"))?;
                    let surface_id = string_arg(&args, 1)?;
                    let store = decode_store.lock().map_err(|e| anyhow!("store lock: {e}"))?;
                    let Ok((width, height, rgba)) = store.image_decode(&bytes) else {
                        return Ok(None);
                    };
                    if opencat_core::text::surface::surface_create_or_resize(&surface_id, width, height).is_err()
                        || opencat_core::text::surface::surface_write_rgba(&surface_id, width, height, rgba).is_err() {
                        return Ok(None);
                    }
                    Ok(Some((width, height)))
                })();
                match result {
                    Ok(Some((width, height))) => Ok(from_json(&decode_ctx, serde_json::json!({"width": width, "height": height})).unwrap()),
                    Ok(None) => Ok(Value::new_null(decode_ctx.clone())),
                    Err(e) => Err(JsError::new_from_js_message("image.decode", "args", &e.to_string())),
                }
            })?;
            globals.set("__opencatImageDecodeInto", decode)?;

            let oval_store = store.clone();
            let ovals = Function::new(ctx.clone(), move |Rest(args): Rest<Value>| {
                let id = string_arg(&args, 0).map_err(|e| JsError::new_from_js_message("fillOvalBatch", "id", &e.to_string()))?;
                let rects = f32_bytes(args.get(1).unwrap())
                    .map_err(|e| JsError::new_from_js_message("fillOvalBatch", "rects", &e.to_string()))?;
                let colors = u8_bytes(args.get(2).unwrap())
                    .map_err(|e| JsError::new_from_js_message("fillOvalBatch", "colors", &e.to_string()))?;
                let count = number_arg(&args, 3) as usize;
                let rects: Vec<f32> = rects.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
                oval_store.lock().map_err(|e| JsError::new_from_js_message("fillOvalBatch", "lock", &e.to_string()))?
                    .record_fill_oval_batch(&id, rects, colors, count);
                Ok::<(), JsError>(())
            })?;
            globals.set("__ocCanvasFillOvalBatch", ovals)?;

            globals.set("__ocStderr", Function::new(ctx.clone(), |message: String| {
                eprintln!("[js] {message}");
                Ok::<(), JsError>(())
            })?)?;
            globals.set("__ocWallMs", Function::new(ctx.clone(), || {
                Ok::<f64, JsError>(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64() * 1e3).unwrap_or_default())
            })?)?;
            let prep = Function::new(ctx.clone(), |ir: Value| -> Result<bool, JsError> {
                let j = to_json(&ir)
                    .map_err(|e| JsError::new_from_js_message("omp.prep", "ir", &e.to_string()))?;
                Ok(crate::script::omp::omp_prep(&j))
            })?;
            globals.set("__ocOmpPrep", prep)?;

            // @omp 循环派发（v8.rs 同语义）：守卫真值 = 数组写回完成。
            // ctx 同 __opencatCallNative——clone 捕获统一生命周期。
            let omp_store = store.clone();
            let omp_ctx = ctx.clone();
            let take = Function::new(
                ctx.clone(),
                move |Rest(args): Rest<Value>| -> Result<Value, JsError> {
                    omp_take_native(&omp_ctx, &omp_store, args.as_ref()).map_err(|e| {
                        JsError::new_from_js_message("omp.take", "args", &e.to_string())
                    })
                },
            )?;
            globals.set("__ocOmpTake", take)?;
            Ok(())
        })
    }

    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
        let mut guard = self.store.lock().expect("script store lock poisoned");
        f(&mut guard)
    }

    fn run_microtasks(&self) {
        while self.runtime.execute_pending_job().unwrap_or(false) {}
    }
}
