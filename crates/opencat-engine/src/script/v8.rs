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
pub(crate) fn ensure_v8_initialized() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        // 诊断开关：进程级 V8 flags（如 --always-opt 强制全函数优化）。
        if let Ok(f) = std::env::var("OC_V8_FLAGS") {
            v8::V8::set_flags_from_string(&f);
        }
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

/// `__opencatCallNative` 的参数数组 → `serde_json::Value` 列表。原始值（数字/
/// 字符串/布尔/null/undefined）逐元素直读，免整包 JSON.stringify + serde 解析
/// 往返——canvas 热绑定每帧数十万次，往返是单次调用的大头；对象/数组元素
/// 仍走 [`v8_to_json`]，语义不变。
fn v8_args_to_json(scope: &mut PinScope<'_, '_>, v: Local<Value>) -> anyhow::Result<Vec<JsonValue>> {
    let Ok(array) = Local::<v8::Array>::try_from(v) else {
        return Ok(match v8_to_json(scope, v)? {
            JsonValue::Array(arr) => arr,
            other => vec![other],
        });
    };
    let mut out = Vec::with_capacity(array.length() as usize);
    for i in 0..array.length() {
        let Some(item) = array.get_index(scope, i) else {
            out.push(JsonValue::Null);
            continue;
        };
        let value = if item.is_null_or_undefined() {
            JsonValue::Null
        } else if item.is_string() {
            JsonValue::String(item.to_rust_string_lossy(scope))
        } else if item.is_number() {
            json_number(item.number_value(scope).unwrap_or(f64::NAN))
        } else if item.is_boolean() {
            JsonValue::Bool(item.boolean_value(scope))
        } else {
            v8_to_json(scope, item)?
        };
        out.push(value);
    }
    Ok(out)
}

/// 数字按 `JSON.stringify` 的文本口径落值：非有限 → null；|n| < 1e21 的整数
/// 值写成整数文本，serde 读回整数（`u32`/`u8` 参数只认整数形态）；其余浮点。
fn json_number(n: f64) -> JsonValue {
    if !n.is_finite() {
        return JsonValue::Null;
    }
    if n.fract() == 0.0 && n.abs() < 1e21 {
        if n >= 0.0 && n < 18_446_744_073_709_551_616.0 {
            return JsonValue::from(n as u64);
        }
        if n < 0.0 && n >= -9_223_372_036_854_775_808.0 {
            return JsonValue::from(n as i64);
        }
    }
    serde_json::Number::from_f64(n).map_or(JsonValue::Null, JsonValue::Number)
}

/// 绑定返回值写回 JS：null/布尔/数字/字符串直设，其余走 [`json_to_v8`]。
/// 绝大多数绑定返回 null，原路是 `"null"` 序列化 + `JSON.parse` 一圈。
fn set_return(
    scope: &mut PinScope<'_, '_>,
    rv: &mut v8::ReturnValue<'_, Value>,
    value: &JsonValue,
) -> anyhow::Result<()> {
    match value {
        JsonValue::Null => rv.set_null(),
        JsonValue::Bool(b) => rv.set_bool(*b),
        JsonValue::Number(n) => match n.as_i64().and_then(|i| i32::try_from(i).ok()) {
            Some(i) => rv.set_int32(i),
            None => rv.set_double(n.as_f64().unwrap_or(f64::NAN)),
        },
        JsonValue::String(s) => {
            let s = V8String::new(scope, s).ok_or_else(|| anyhow!("v8: string allocation failed"))?;
            rv.set(s.into());
        }
        _ => rv.set(json_to_v8(scope, value)?),
    }
    Ok(())
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
    let args_json = match v8_args_to_json(scope, args.get(1)) {
        Ok(arr) => arr,
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
        Ok(value) => {
            if let Err(e) = set_return(scope, &mut rv, &value) {
                throw(scope, &format!("script: {e}"));
            }
        }
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
    // Canvas 2D putImageData 是像素直写（无 alpha 混合）：Src blend 的
    // ImageRect，不是 SrcOver 的 DrawOp::Image——否则底下已画内容会透出
    // （heatmap：径向渐变瓦片上 putImageData 着色覆盖）。
    store.record_frame_put_image(
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

/// `__opencatRasterCanvas(id, w, h, out, key)` — offscreen canvas fast path.
///
/// `document.createElement('canvas')` surfaces record standard draw ops into
/// `MutationStore::canvases` (same `__canvas_*` bindings as DOM canvases).
/// When script synchronously consumes pixels (getImageData / toDataURL /
/// createPattern / drawImage with a canvas source) this rasterizes the
/// recorded ops via the engine's Skia replay path, registers the result as a
/// frame generated image (deterministic id from `key`), and fills the
/// caller's preallocated `Uint8Array` in place. Zero new IR ops: the main
/// frame consumes the pixels as a standard ImageRect{Generated}.
fn raster_canvas(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "rasterCanvas: realm native state missing");
        return;
    };
    let id = args.get(0).to_rust_string_lossy(scope);
    let number = |index: i32| {
        args.get(index)
            .to_number(scope)
            .map(|n| n.value())
            .unwrap_or(0.0)
    };
    let (w, h) = (number(1), number(2));
    let out: Local<v8::Uint8Array> = match args.get(3).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "rasterCanvas: `out` must be a Uint8Array");
            return;
        }
    };
    let Ok(mut store) = state.store.lock() else {
        throw(scope, "rasterCanvas: store lock poisoned");
        return;
    };
    // 处女画布（从未落笔）：浏览器 getImageData 返回全透明——wordCloud 的
    // 碰撞板在布局起步时就会先读后写。空画布直接返回零字节，不 raster
    // 也不抛错（out 本就是零初始化的 Uint8Array）。
    let empty_canvas = match store.canvas_commands(&id) {
        None => true,
        Some(canvas) => canvas.commands.is_empty(),
    };
    if empty_canvas {
        let need = (w as f32 * h as f32) as usize * 4;
        if out.byte_length() < need {
            throw(scope, "rasterCanvas: destination Uint8Array smaller than canvas");
            return;
        }
        return;
    }
    let Some(canvas) = store.canvas_commands(&id).cloned() else {
        unreachable!("canvas_commands vanished between empty check and clone");
    };
    let (width, height, rgba) =
        match crate::script::offscreen::rasterize_canvas_commands(&canvas, w as f32, h as f32, &mut store) {
            Ok(result) => result,
            Err(e) => {
                throw(scope, &format!("rasterCanvas: {e}"));
                return;
            }
        };
    if out.byte_length() < rgba.len() {
        throw(scope, "rasterCanvas: destination Uint8Array smaller than canvas");
        return;
    }
    // Read-only binding (getImageData/probes): rasterize and copy out. No
    // generated-image registration — a read must not mutate the frame's
    // generated table (two different-size canvases would collide on a
    // shared key; consuming paths register their own pixels explicitly).
    let destination = out.data() as *mut u8;
    // SAFETY: `out` is rooted by this call's arguments, so its backing store
    // stays alive (and cannot be detached) across the copy.
    unsafe {
        std::ptr::copy_nonoverlapping(rgba.as_ptr(), destination, rgba.len());
    }
    rv.set_int32(rgba.len() as i32);
}

/// `__opencatOffscreenReset(id)` — 浏览器 width/height 赋值语义的清流端：
/// 丢弃该离屏画布的持久命令流（配合 JS 侧 raster 缓存失效，见 dom_api 的
/// width/height setter）。
fn offscreen_canvas_reset(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let _ = rv;
    let Some(state) = native_state(scope) else {
        throw(scope, "offscreenReset: realm native state missing");
        return;
    };
    let id = args.get(0).to_rust_string_lossy(scope);
    let Ok(mut store) = state.store.lock() else {
        throw(scope, "offscreenReset: store lock poisoned");
        return;
    };
    store.reset_offscreen_canvas(&id);
}

/// `__opencatCanvasPathFlat(id, flat, dx, dy)` — `canvas_path_flat` 的
/// typed-array 快路径：fillText 烘焙的整段字形轮廓动辄数千个数，走 JSON
/// 派发每次要数百微秒。
fn canvas_path_flat(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    _rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "canvas.pathFlat: realm native state missing");
        return;
    };
    let id = args.get(0).to_rust_string_lossy(scope);
    let flat_v8: Local<v8::Float64Array> = match args.get(1).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "canvas.pathFlat: `flat` must be a Float64Array");
            return;
        }
    };
    let mut raw = vec![0u8; flat_v8.byte_length()];
    let copied = v8::ArrayBufferView::copy_contents(&flat_v8, &mut raw);
    let flat: Vec<f64> = raw[..copied]
        .chunks_exact(8)
        .map(|c| f64::from_ne_bytes(c.try_into().expect("8-byte chunk")))
        .collect();
    let number = |index: i32| {
        args.get(index)
            .to_number(scope)
            .map(|n| n.value())
            .unwrap_or(0.0)
    };
    let (dx, dy) = (number(2), number(3));
    let Ok(mut store) = state.store.lock() else {
        throw(scope, "canvas.pathFlat: store lock poisoned");
        return;
    };
    if let Err(e) = opencat_core::script::bindings::record_path_flat(&mut *store, &id, &flat, dx, dy) {
        throw(scope, &format!("canvas.pathFlat: {e}"));
    }
}

/// `__opencatRasterCanvasPNG(id, w, h, out)` — offscreen canvas → PNG bytes.
///
/// Same rasterization as `raster_canvas`, but the caller gets an encoded PNG
/// (for `toDataURL`/inline `ScriptImage` bytes). `out` is a preallocated
/// `Uint8Array`; returns the PNG byte length.
fn raster_canvas_png(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "rasterCanvasPNG: realm native state missing");
        return;
    };
    let id = args.get(0).to_rust_string_lossy(scope);
    let number = |index: i32| {
        args.get(index)
            .to_number(scope)
            .map(|n| n.value())
            .unwrap_or(0.0)
    };
    let (w, h) = (number(1), number(2));
    let out: Local<v8::Uint8Array> = match args.get(3).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "rasterCanvasPNG: `out` must be a Uint8Array");
            return;
        }
    };

    let Ok(mut store) = state.store.lock() else {
        throw(scope, "rasterCanvasPNG: store lock poisoned");
        return;
    };
    // 处女画布：toDataURL 应得到全透明图，不抛错。
    let empty_canvas = match store.canvas_commands(&id) {
        None => true,
        Some(canvas) => canvas.commands.is_empty(),
    };
    if empty_canvas {
        let (iw, ih) = ((w as u32).max(1), (h as u32).max(1));
        let png = crate::script::offscreen::encode_rgba_png(iw, ih, &vec![0u8; (iw * ih * 4) as usize])
            .unwrap_or_else(|_| vec![0u8; 8]);
        if out.byte_length() < png.len() {
            throw(scope, "rasterCanvasPNG: destination Uint8Array smaller than PNG");
            return;
        }
        let destination = out.data() as *mut u8;
        // SAFETY: `out` is rooted by this call's arguments.
        unsafe {
            std::ptr::copy_nonoverlapping(png.as_ptr(), destination, png.len());
        }
        rv.set_int32(png.len() as i32);
        return;
    }
    let Some(canvas) = store.canvas_commands(&id).cloned() else {
        unreachable!("canvas_commands vanished between empty check and clone");
    };
    let (width, height, png) = match crate::script::offscreen::rasterize_canvas_commands_png(
        &canvas, w as f32, h as f32, &mut store,
    ) {
        Ok(result) => result,
        Err(e) => {
            throw(scope, &format!("rasterCanvasPNG: {e}"));
            return;
        }
    };
    if out.byte_length() < png.len() {
        throw(scope, "rasterCanvasPNG: destination Uint8Array smaller than PNG");
        return;
    }
    let destination = out.data() as *mut u8;
    // SAFETY: `out` is rooted by this call's arguments, so its backing store
    // stays alive (and cannot be detached) across the copy.
    unsafe {
        std::ptr::copy_nonoverlapping(png.as_ptr(), destination, png.len());
    }
    // Keep width/height discoverable from JS: pack into the return value as
    // `length | (width << 20) | height << 4` is fragile — JS side instead
    // tracks width/height itself; return just the length.
    let _ = (width, height);
    rv.set_int32(png.len() as i32);
}

/// `__opencatGLIntern(targetId, bytes)` — binary fast path for GL stream
/// intern payloads (VBO/texture bytes, up to tens of MB). Returns the range id.
fn gl_intern(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "gl.intern: realm native state missing");
        return;
    };
    let target_id = args.get(0).to_rust_string_lossy(scope);
    let bytes_v8: Local<v8::Uint8Array> = match args.get(1).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "gl.intern: `bytes` must be a Uint8Array");
            return;
        }
    };
    let mut bytes = vec![0u8; bytes_v8.byte_length()];
    let copied = v8::ArrayBufferView::copy_contents(&bytes_v8, &mut bytes);
    bytes.truncate(copied);
    let Ok(mut store) = state.store.lock() else {
        throw(scope, "gl.intern: store lock poisoned");
        return;
    };
    match store.gl_stream_intern(&target_id, bytes) {
        Ok(range) => rv.set_int32(range as i32),
        Err(e) => throw(scope, &format!("gl.intern: {e}")),
    }
}

/// `__opencatAssetStat(path)` — byte length of a document-relative asset, or
/// −1 when unreadable. The two-step stat/read shape keeps the fill-in-place
/// contract (caller preallocates, native fills) used by `surface_read_into`.
fn asset_stat(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "asset.stat: realm native state missing");
        return;
    };
    let path = args.get(0).to_rust_string_lossy(scope);
    let Ok(store) = state.store.lock() else {
        throw(scope, "asset.stat: store lock poisoned");
        return;
    };
    match store.asset_read(&path) {
        Ok(bytes) => rv.set_int32(bytes.len() as i32),
        Err(_) => rv.set_int32(-1),
    }
}

/// `__opencatAssetReadInto(path, out)` — fill `out` with the asset bytes.
fn asset_read_into(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "asset.read: realm native state missing");
        return;
    };
    let path = args.get(0).to_rust_string_lossy(scope);
    let out: Local<v8::Uint8Array> = match args.get(1).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "asset.read: `out` must be a Uint8Array");
            return;
        }
    };
    let Ok(store) = state.store.lock() else {
        throw(scope, "asset.read: store lock poisoned");
        return;
    };
    let bytes = match store.asset_read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            throw(scope, &format!("asset.read `{path}`: {e}"));
            return;
        }
    };
    if out.byte_length() < bytes.len() {
        throw(scope, "asset.read: destination Uint8Array smaller than asset");
        return;
    }
    let destination = out.data() as *mut u8;
    // SAFETY: `out` is rooted by this call's arguments (see surface_read_into).
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), destination, bytes.len());
    }
    rv.set_int32(bytes.len() as i32);
}

/// `__ocStderr(msg)` — realm console.error 的宿主 stderr 出口（CLI 渲染
/// 没有别的诊断通道，资源加载链错误曾因此全部静默）。
fn stderr_log(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let msg = args.get(0).to_rust_string_lossy(scope);
    eprintln!("[js] {msg}");
    rv.set(v8::undefined(scope).into());
}

/// `__ocWallMs()` — 宿主墙钟毫秒。realm 内所有时钟面（performance.now/
/// Date.now）都绑虚拟钟（frame=f(t) 确定性），性能测量探针需要一个
/// 不进确定性的出口——只在测量分支被调用，正常渲染路径不存在。
fn wall_clock(
    scope: &mut PinScope<'_, '_>,
    _args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1e3)
        .unwrap_or(0.0);
    rv.set(v8::Number::new(scope, ms).into());
}

/// `__ocCanvasFillOvalBatch(id, rects, colors, n)` — canvas 2D 批量椭圆填充
/// 的二进制快路。`rects` 是 n×4 的 Float32Array（x, y, w, h），`colors` 是
/// n×4 的 Uint8Array（r, g, b, a）。与逐点快路落同一条 op 序列
/// （SetFillStyle+BeginPath+AddOval+FillPath），位图逐位一致；一次跨界
/// memcpy 替代每点两次 Value 化调用（halftone 语料每帧 ~30 万点）。
fn fill_oval_batch(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "fillOvalBatch: realm native state missing");
        return;
    };
    let node_id = args.get(0).to_rust_string_lossy(scope);
    let rects: Local<v8::Float32Array> = match args.get(1).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "fillOvalBatch: `rects` must be a Float32Array");
            return;
        }
    };
    let colors: Local<v8::Uint8Array> = match args.get(2).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "fillOvalBatch: `colors` must be a Uint8Array");
            return;
        }
    };
    let count = args.get(3).to_number(scope).map(|n| n.value()).unwrap_or(0.0);
    if !(count >= 0.0 && count <= 1.0e7) {
        throw(scope, "fillOvalBatch: bad count");
        return;
    }
    let count = count as usize;
    let mut rect_bytes = vec![0u8; rects.byte_length()];
    let copied = v8::ArrayBufferView::copy_contents(&rects, &mut rect_bytes);
    rect_bytes.truncate(copied);
    let mut color_bytes = vec![0u8; colors.byte_length()];
    let copied = v8::ArrayBufferView::copy_contents(&colors, &mut color_bytes);
    color_bytes.truncate(copied);
    // f32 从拷贝的 u8 buffer 逐个转（x86 小端，from_le_bytes 编译成直接
    // load；120 万次/帧在 ns 级）。此后数据 Arc 化进一条 op，全链零深拷贝。
    let rects_f32: Vec<f32> = rect_bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    let Ok(mut store) = state.store.lock() else {
        throw(scope, "fillOvalBatch: store lock poisoned");
        return;
    };
    store.record_fill_oval_batch(&node_id, rects_f32, color_bytes, count);
    rv.set(v8::undefined(scope).into());
}

/// `__opencatImageDecodeInto(src, surfaceId)` — decode an encoded image
/// (webp/png/jpeg, the GLTFLoader texture payload) straight into a session
/// surface; resolves to `{width, height}` or `null` on decode failure.
fn image_decode_into(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        throw(scope, "image.decode: realm native state missing");
        return;
    };
    let src: Local<v8::Uint8Array> = match args.get(0).try_into() {
        Ok(array) => array,
        Err(_) => {
            throw(scope, "image.decode: `src` must be a Uint8Array");
            return;
        }
    };
    let mut bytes = vec![0u8; src.byte_length()];
    let copied = v8::ArrayBufferView::copy_contents(&src, &mut bytes);
    bytes.truncate(copied);
    let surface_id = args.get(1).to_rust_string_lossy(scope);
    let Ok(store) = state.store.lock() else {
        throw(scope, "image.decode: store lock poisoned");
        return;
    };
    let decoded = match store.image_decode(&bytes) {
        Ok((w, h, rgba)) => (w, h, rgba),
        Err(e) => {
            eprintln!("[js] image_decode failed: {e} ({} bytes)", bytes.len());
            let null = v8::null(scope);
            rv.set(null.into());
            return;
        }
    };
    let (w, h, rgba) = decoded;
    if opencat_core::text::surface::surface_create_or_resize(&surface_id, w, h).is_err()
        || opencat_core::text::surface::surface_write_rgba(&surface_id, w, h, rgba).is_err()
    {
        let null = v8::null(scope);
        rv.set(null.into());
        return;
    }
    let json = serde_json::json!({ "width": w, "height": h });
    match json_to_v8(scope, &json) {
        Ok(v) => rv.set(v),
        Err(e) => throw(scope, &format!("image.decode: {e}")),
    }
}

/// `__ocOmpPrep(ir)` — @omp 编译缓存（提取期守卫第一步）。ir = 提取期
/// 注入的 `{loop, fns, args, captures}`。true = 骨架可跑；false = 编译
/// 失败（条目毒化，后续帧直接短路），原循环照跑。
fn omp_prep(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let ir = v8_to_json(scope, args.get(0));
    match ir {
        Ok(j) => rv.set_bool(crate::script::omp::omp_prep(&j)),
        Err(_) => rv.set_bool(false),
    }
}

/// `__ocOmpTake(ir, ...args, skeleton)` — @omp 循环派发入口（提取期把
/// 标注的 for 包进
/// `if (!(... && __ocOmpPrep(ir) && __ocOmpTake(ir, args, skeleton)))`）。
/// 返回 true = 循环被旁路（Rust rayon 已直推 op 流）；false = 降级，原
/// 循环照跑。任何失败都回落 false，不抛异常——降级语义是「pragma 未被
/// 编译器处理」。prng 类 args 是页面函数（闭包值），不做 JSON 化——骨架
/// 已按原序预采进末位的 Float64Array。
fn omp_take(
    scope: &mut PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
    mut rv: v8::ReturnValue<'_, Value>,
) {
    let Some(state) = native_state(scope) else {
        rv.set_bool(false);
        return;
    };
    let Ok(ir) = v8_to_json(scope, args.get(0)) else {
        rv.set_bool(false);
        return;
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
    let is_prng = |name: &str| {
        captures.and_then(|c| c.get(name)).and_then(|v| v.as_str()) == Some("prng")
    };
    let is_ctx = |name: &str| {
        captures.and_then(|c| c.get(name)).and_then(|v| v.as_str()) == Some("ctx")
    };
    let is_array = |name: &str| {
        captures.and_then(|c| c.get(name)).and_then(|v| v.as_str()) == Some("array")
    };
    let n_args = arg_names.len();
    let mut rest: Vec<JsonValue> = Vec::with_capacity(n_args);
    for (k, name) in arg_names.iter().enumerate() {
        if is_prng(name) {
            rest.push(JsonValue::Null); // 函数值不可 JSON 化；骨架已预采
            continue;
        }
        if is_array(name) {
            // 数组槽不过 isolate（copy-in 零字节）：实参以令牌替代，写回
            // 由 take 产出的 writes 在下方应用到真数组。要求实参是对象
            // （Array / TypedArray）——上行校验，失败走降级（take 未发生）。
            // len 随令牌走：worker 行缓冲按它分配。
            let av = args.get(k as i32 + 1);
            let len = av
                .to_object(scope)
                .and_then(|obj| {
                    let key = V8String::new(scope, "length")?;
                    obj.get(scope, key.into())
                })
                .and_then(|lv| lv.number_value(scope))
                .unwrap_or(0.0);
            if len <= 0.0 {
                rv.set_bool(false);
                return;
            }
            rest.push(serde_json::json!({ "__ocOmpArr": k, "len": len as u64 }));
            continue;
        }
        if is_ctx(name) {
            // html 通道的 ctx 是垫片 Proxy（html_api `getContext` 返回面，
            // get 钩子转发到 core 的 context2d face）：JSON.stringify 对
            // Proxy 走目标 ownKeys 序列化得 `{}`，具名属性直读才经钩子到
            // 真 face。take 只需要这两个键。
            let v = args.get(k as i32 + 1);
            // Proxy 不是 JSObject：to_object 对它恒等返回（ToObject 语义），
            // 随后的 get 走 [[Get]] 陷阱 = 垫片钩子。
            let face = v.to_object(scope);
            let node_id = face.and_then(|o| {
                V8String::new(scope, "__ocNodeId").and_then(|key| {
                    o.get(scope, key.into()).and_then(|val| {
                        val.is_string().then(|| val.to_rust_string_lossy(scope))
                    })
                })
            });
            let global_alpha = face.and_then(|o| {
                V8String::new(scope, "state").and_then(|key| {
                    o.get(scope, key.into()).and_then(|st| st.to_object(scope)).and_then(|st| {
                        V8String::new(scope, "globalAlpha").and_then(|gk| {
                            st.get(scope, gk.into()).and_then(|ga| {
                                ga.is_number().then(|| ga.number_value(scope)).flatten()
                            })
                        })
                    })
                })
            });
            rest.push(match node_id {
                Some(id) => serde_json::json!({
                    "__ocNodeId": id,
                    "state": { "globalAlpha": global_alpha.unwrap_or(1.0) },
                }),
                None => JsonValue::Null, // 读不到 = 不是 ctx face → 毒化
            });
            continue;
        }
        let v = args.get(k as i32 + 1);
        let json = if v.is_null_or_undefined() {
            Ok(JsonValue::Null)
        } else if v.is_string() {
            Ok(JsonValue::String(v.to_rust_string_lossy(scope)))
        } else if v.is_number() {
            Ok(json_number(v.number_value(scope).unwrap_or(f64::NAN)))
        } else if v.is_boolean() {
            Ok(JsonValue::Bool(v.boolean_value(scope)))
        } else {
            v8_to_json(scope, v)
        };
        match json {
            Ok(j) => rest.push(j),
            Err(_) => {
                rv.set_bool(false);
                return;
            }
        }
    }
    // 末位 = 采集骨架（Float64Array；无采集时是 null）。
    let mut prng: Vec<f64> = Vec::new();
    let skel = args.get(n_args as i32 + 1);
    if let Ok(fa) = v8::Local::<v8::Float64Array>::try_from(skel) {
        let mut bytes = vec![0u8; fa.byte_length()];
        let copied = v8::ArrayBufferView::copy_contents(&fa, &mut bytes);
        bytes.truncate(copied);
        prng = bytes
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect();
    }
    let mut writes: Vec<crate::script::omp::protocol::IsolateWrite> = Vec::new();
    let mut reds: Vec<f64> = Vec::new();
    let mut taken = match state.store.lock() {
        Ok(mut store) => {
            crate::script::omp::omp_take(&mut store, &ir, &rest, &prng, &mut writes, &mut reds)
        }
        Err(_) => false,
    };
    // isolate 腿的写回：take=true 时按行序把 Set/Acc 四元组应用到真数组
    // （ES [[Get]]/[[Set]] 语义——JS Array 与 TypedArray 通吃）。此时骨架
    // 已旁路原循环、prng 已按行预采消耗，应用失败无法干净降级——上行已
    // 校验对象性，这里的失败路径只为防御（false = fallback 复跑原循环）。
    if taken {
        for w in &writes {
            let Some(obj) = args.get(w.arg as i32 + 1).to_object(scope) else {
                taken = false;
                break;
            };
            let cur = if w.acc {
                match obj
                    .get_index(scope, w.idx)
                    .and_then(|v| v.number_value(scope))
                {
                    Some(c) => c + w.val,
                    None => {
                        taken = false;
                        break;
                    }
                }
            } else {
                w.val
            };
            if obj
                .set_index(scope, w.idx, v8::Number::new(scope, cur).into())
                .is_none()
            {
                taken = false;
                break;
            }
        }
    }
    // reduction 面：take=true 且有 red 终值 → 返回折叠值数组（守卫把它
    // 赋给 `__ocOmpRed`，else 臂按 ir.reduction 展开序取 `__ocOmpRed[k]`
    // 写回页面变量）。true/null（无 red）照旧走布尔。
    if taken && !reds.is_empty() {
        match set_return(scope, &mut rv, &serde_json::json!(reds)) {
            Ok(()) => return,
            Err(_) => rv.set_bool(false), // 返回面失败 = fallback 复跑原循环
        }
        return;
    }
    rv.set_bool(taken);
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
    /// 注入真 GL 反射器（engine 专用，core 只定义转发通道）。见
    /// [`crate::gpu::stream_executor::install_glsl_reflector`]。
    pub fn set_glsl_reflector(
        &self,
        f: Box<dyn Fn(&str, &str) -> anyhow::Result<serde_json::Value> + Send>,
    ) {
        let mut guard = self.natives.store.lock().expect("script store lock poisoned");
        guard.set_glsl_reflector(f);
    }

    /// 注入资产读取器（文档相对路径 → 字节）。见
    /// [`crate::gpu::stream_executor::install_asset_bridge`]。
    pub fn set_asset_reader(&self, f: Box<dyn Fn(&str) -> anyhow::Result<Vec<u8>> + Send>) {
        let mut guard = self.natives.store.lock().expect("script store lock poisoned");
        guard.set_asset_reader(f);
    }

    /// 注入图像解码器（编码字节 → (w, h, rgba8)）。见
    /// [`crate::gpu::stream_executor::install_asset_bridge`]。
    pub fn set_image_decoder(
        &self,
        f: Box<dyn Fn(&[u8]) -> anyhow::Result<(u32, u32, Vec<u8>)> + Send>,
    ) {
        let mut guard = self.natives.store.lock().expect("script store lock poisoned");
        guard.set_image_decoder(f);
    }

    /// 注入视频帧解码器（(文档相对路径, 媒体时刻秒) → RGBA 帧）。见
    /// [`crate::script::bridge::install_asset_bridge`]。
    pub fn set_video_frame_decoder(
        &self,
        f: Box<dyn Fn(&str, f64) -> anyhow::Result<(Vec<u8>, u32, u32)> + Send>,
    ) {
        let mut guard = self.natives.store.lock().expect("script store lock poisoned");
        guard.set_video_frame_decoder(f);
    }

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
            let ok = function.call(tc, receiver.into(), &[]).is_some();
            if ok { Ok(()) } else { Err(anyhow!(caught!(tc, name))) }
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
            let raster = Function::new(scope, raster_canvas)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatRasterCanvas"))?;
            set_global(scope, "__opencatRasterCanvas", raster.into())?;
            let raster_png = Function::new(scope, raster_canvas_png)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatRasterCanvasPNG"))?;
            set_global(scope, "__opencatRasterCanvasPNG", raster_png.into())?;
            let oreset = Function::new(scope, offscreen_canvas_reset)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatOffscreenReset"))?;
            set_global(scope, "__opencatOffscreenReset", oreset.into())?;
            let path_flat = Function::new(scope, canvas_path_flat)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatCanvasPathFlat"))?;
            set_global(scope, "__opencatCanvasPathFlat", path_flat.into())?;

            // GL stream / asset / image natives (binary fast paths, same shape
            // as the two above).
            let intern = Function::new(scope, gl_intern)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatGLIntern"))?;
            set_global(scope, "__opencatGLIntern", intern.into())?;

            let stat = Function::new(scope, asset_stat)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatAssetStat"))?;
            set_global(scope, "__opencatAssetStat", stat.into())?;

            let aread = Function::new(scope, asset_read_into)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatAssetReadInto"))?;
            set_global(scope, "__opencatAssetReadInto", aread.into())?;

            let idecode = Function::new(scope, image_decode_into)
                .ok_or_else(|| anyhow!("v8: failed to create __opencatImageDecodeInto"))?;
            set_global(scope, "__opencatImageDecodeInto", idecode.into())?;

            let stderr = Function::new(scope, stderr_log)
                .ok_or_else(|| anyhow!("v8: failed to create __ocStderr"))?;
            set_global(scope, "__ocStderr", stderr.into())?;

            // 测量探针用墙钟（见 wall_clock 文档）。
            let wall = Function::new(scope, wall_clock)
                .ok_or_else(|| anyhow!("v8: failed to create __ocWallMs"))?;
            set_global(scope, "__ocWallMs", wall.into())?;

            // Canvas 2D 批量椭圆填充（二进制快路，同 putImageData 形状）。
            let obatch = Function::new(scope, fill_oval_batch)
                .ok_or_else(|| anyhow!("v8: failed to create __ocCanvasFillOvalBatch"))?;
            set_global(scope, "__ocCanvasFillOvalBatch", obatch.into())?;

            // @omp 循环派发（script/omp.rs；未命中 native 时 JS 侧降级跑原循环）。
            let omp = Function::new(scope, omp_take)
                .ok_or_else(|| anyhow!("v8: failed to create __ocOmpTake"))?;
            set_global(scope, "__ocOmpTake", omp.into())?;
            let omp_prep = Function::new(scope, omp_prep)
                .ok_or_else(|| anyhow!("v8: failed to create __ocOmpPrep"))?;
            set_global(scope, "__ocOmpPrep", omp_prep.into())?;
            Ok(())
        })
    }

    fn with_store_mut<R>(&self, f: impl FnOnce(&mut MutationStore) -> R) -> R {
        let mut guard = self.natives.store.lock().expect("script store lock poisoned");
        f(&mut guard)
    }

    fn run_microtasks(&self) {
        let mut guard = self.isolate.lock();
        guard.perform_microtask_checkpoint();
    }

    fn eval_json(&self, code: &str) -> anyhow::Result<JsonValue> {
        V8JsContext::eval_json(self, code)
    }
}
