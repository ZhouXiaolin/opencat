//! 浏览器原生 JS 引擎后端：`JsContext` 的具体实现。
//!
//! Isolation note (issue #20): core owns one [`opencat_core::script::ScriptRealm`]
//! per pipeline and never rebinds dispatchers across runners. The web host holds
//! a single active pipeline (`WebRenderer`); replacing a design constructs a new
//! `WebJsContext`. Correctness does not depend on rebinding a shared
//! `globalThis` between drivers — drivers share one realm via per-driver globals.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use opencat_core::script::js_context::JsContext;
use opencat_core::script::omp::protocol::{SharedBuf, SharedKind};
use opencat_core::script::recorder::MutationStore;

pub struct WebJsContext {
    store: Rc<RefCell<MutationStore>>,
    _dispatcher_handle: RefCell<Option<Closure<dyn FnMut(JsValue, JsValue) -> JsValue>>>,
    _natives_handles: RefCell<Vec<JsValue>>,
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
        // The facade bundle is an ES module, so its realm is strict and a
        // direct `js_sys::eval` inherits that strictness. Pages contribute
        // classic scripts: a UMD header's `window.window = ...` assignment
        // must silently no-op (Replaceable accessor, sloppy mode) instead of
        // throwing. An indirect eval (`(0,eval)`) evaluates the code in
        // global scope with the code's own strictness — the same semantics
        // the native V8 leg and a real `<script>` tag give it.
        let run = js_sys::Function::new_no_args("(0,eval)(arguments[0])");
        run.call1(&JsValue::UNDEFINED, &JsValue::from_str(code))
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
        // Host capability: this realm's `location` is the real browser
        // navigation face — assigning `location.href` navigates the page.
        // html_std routes the G13 document-URL fill through the synthetic
        // `document.location` face instead of the global (see
        // `__ocHtmlSetLocationFace`).
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__ocHtmlLocationIsNav"),
            &JsValue::TRUE,
        );
        let _ = js_sys::Reflect::set(
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
        // The runtime calls these positionally
        // (`__opencatPutImageData(nodeId, key, pixels, x, y, w, h)`), matching
        // the native V8 fast path; pack them back into one array so the
        // existing index-based readers stay shared with the JSON side.
        let put = Closure::wrap(Box::new(
            move |a: JsValue,
                  b: JsValue,
                  c: JsValue,
                  d: JsValue,
                  e: JsValue,
                  f: JsValue,
                  g: JsValue|
                  -> JsValue {
                let args = js_sys::Array::new();
                for v in [a, b, c, d, e, f, g] {
                    args.push(&v);
                }
                put_image_data(&put_store, &args);
                JsValue::UNDEFINED
            },
        )
            as Box<
                dyn FnMut(JsValue, JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) -> JsValue,
            >);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatPutImageData"),
            put.as_ref().unchecked_ref(),
        );
        natives.push(put.into_js_value());

        let native_canvas_store = self.store.clone();
        let native_canvas = Closure::wrap(Box::new(
            move |a: JsValue,
                  b: JsValue,
                  c: JsValue,
                  d: JsValue,
                  e: JsValue,
                  f: JsValue,
                  g: JsValue|
                  -> JsValue {
                let args = js_sys::Array::new();
                for v in [a, b, c, d, e, f, g] {
                    args.push(&v);
                }
                record_native_canvas(&native_canvas_store, &args);
                JsValue::UNDEFINED
            },
        )
            as Box<
                dyn FnMut(JsValue, JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) -> JsValue,
            >);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatRecordNativeCanvas"),
            native_canvas.as_ref().unchecked_ref(),
        );
        natives.push(native_canvas.into_js_value());

        let read = Closure::wrap(Box::new(
            |a: JsValue, b: JsValue, c: JsValue, d: JsValue, e: JsValue, f: JsValue| -> JsValue {
                let args = js_sys::Array::new();
                for v in [a, b, c, d, e, f] {
                    args.push(&v);
                }
                surface_read_into(&args);
                JsValue::UNDEFINED
            },
        )
            as Box<
                dyn FnMut(JsValue, JsValue, JsValue, JsValue, JsValue, JsValue) -> JsValue,
            >);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatSurfaceReadInto"),
            read.as_ref().unchecked_ref(),
        );
        natives.push(read.into_js_value());

        // `__opencatRasterCanvas(id, w, h, out)` — the stub canvas realm reads
        // pixels (getImageData / probes) from canvases whose draw calls were
        // recorded into the MutationStore. Compile the commands into an
        // offscreen DrawOpFrame, pack it as an OCIR envelope, and hand it to
        // the TS replay hook (`globalThis.__ocRasterCanvasReplay`) which
        // draws it through the shared CanvasKit interpreter and returns
        // premultiplied RGBA. Same pixel contract as the engine's
        // `raster_canvas` (RGBA8888/Premul), no generated-table mutation.
        let raster_store = self.store.clone();
        let raster = Closure::wrap(Box::new(
            move |a: JsValue, b: JsValue, c: JsValue, d: JsValue| -> JsValue {
                let args = js_sys::Array::new();
                for v in [a, b, c, d] {
                    args.push(&v);
                }
                raster_canvas(&raster_store, &args)
            },
        )
            as Box<dyn FnMut(JsValue, JsValue, JsValue, JsValue) -> JsValue>);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__opencatRasterCanvas"),
            raster.as_ref().unchecked_ref(),
        );
        natives.push(raster.into_js_value());

        // @omp natives（engine v8.rs 同语义）。prep = bundle 编译缓存（true
        // = 骨架可跑）；take = 循环派发——变参（`ir, ...args, skeleton`）经
        // JS 薄壳收进单数组转发给定参闭包，行为面与 V8 positional native
        // 一致：任何失败回落 false = 降级，原循环照跑。
        let prep = Closure::wrap(Box::new(|ir: JsValue| -> JsValue {
            omp_prep_native(&ir)
        }) as Box<dyn Fn(JsValue) -> JsValue>);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__ocOmpPrep"),
            prep.as_ref().unchecked_ref(),
        );
        natives.push(prep.into_js_value());

        let take_store = self.store.clone();
        let take = Closure::wrap(Box::new(move |args: JsValue| -> JsValue {
            omp_take_native(&take_store, &args)
        }) as Box<dyn Fn(JsValue) -> JsValue>);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__ocOmpTakeArgs"),
            take.as_ref().unchecked_ref(),
        );
        natives.push(take.into_js_value());
        let fwd = js_sys::Function::new_with_args(
            "",
            "return globalThis.__ocOmpTakeArgs(Array.from(arguments));",
        );
        let _ = js_sys::Reflect::set(&global, &JsValue::from_str("__ocOmpTake"), &fwd);

        // `__ocCanvasFillOvalBatch(id, rects, colors, n)` — context2d 批量
        // 椭圆填充的二进制通道（engine quickjs.rs/v8.rs 同名 native 的
        // wasm 侧对位）。TypedArray 直收，落流与逐点同序同参。
        let oval_store = self.store.clone();
        let ovals = Closure::wrap(Box::new(
            move |a: JsValue, b: JsValue, c: JsValue, d: JsValue| -> JsValue {
                let id = a.as_string().unwrap_or_default();
                let rects = b
                    .dyn_into::<js_sys::Float32Array>()
                    .ok()
                    .map(|ta| ta.to_vec())
                    .unwrap_or_default();
                let colors = c
                    .dyn_into::<js_sys::Uint8Array>()
                    .ok()
                    .map(|ta| ta.to_vec())
                    .unwrap_or_default();
                let count = d.as_f64().unwrap_or(0.0) as usize;
                oval_store
                    .borrow_mut()
                    .record_fill_oval_batch(&id, rects, colors, count);
                JsValue::UNDEFINED
            },
        ) as Box<dyn FnMut(JsValue, JsValue, JsValue, JsValue) -> JsValue>);
        let _ = js_sys::Reflect::set(
            &global,
            &JsValue::from_str("__ocCanvasFillOvalBatch"),
            ovals.as_ref().unchecked_ref(),
        );
        natives.push(ovals.into_js_value());

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
        web_sys::console::error_1(&JsValue::from_str(
            "putImageData: `pixels` must be a Uint8Array",
        ));
        return;
    };
    let (x, y, w, h) = (
        num_arg(args, 3),
        num_arg(args, 4),
        num_arg(args, 5),
        num_arg(args, 6),
    );
    let id = opencat_core::ir::GeneratedImageId::from_key(&key);
    // Same semantics as the native fast path: putImageData is a direct pixel
    // write (Src ImageRect), not a generated image registration.
    store.borrow_mut().record_frame_put_image(
        &node_id,
        id,
        x as f32,
        y as f32,
        w as u32,
        h as u32,
        pixels.to_vec().into(),
    );
}

/// `__opencatRecordNativeCanvas(nodeId, key, pixels, x, y, w, h)`.
///
/// Web native-GL mode: the browser executed the composition's real WebGL2
/// calls, and the frame-end hook read the drawing buffer back. Register it
/// through the normal generated-image channel (the draw op sits on the canvas
/// node, so the element's stacking/opacity still applies).
fn record_native_canvas(store: &Rc<RefCell<MutationStore>>, args: &JsValue) {
    let node_id = str_arg(args, 0);
    let key = str_arg(args, 1);
    let Some(pixels) = bytes_arg(args, 2) else {
        web_sys::console::error_1(&JsValue::from_str(
            "recordNativeCanvas: `pixels` must be a Uint8Array",
        ));
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
        web_sys::console::error_1(&JsValue::from_str(
            "surface.read: `out` must be a Uint8Array",
        ));
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

/// `__opencatRasterCanvas(id, x, y, w, h, out)` — fill `out` with the
/// recorded canvas commands' pixels (premultiplied RGBA), returning the byte
/// length. An empty (never-drawn) canvas stays all-transparent: browsers
/// return zeros there, and the out array is already zero-initialized.
fn raster_canvas(store: &Rc<RefCell<MutationStore>>, args: &JsValue) -> JsValue {
    use opencat_core::ir::draw_encoding::{encode_draw_sections, pack_ir_envelope};
    use opencat_core::ir::draw_frame::DrawFrameScratch;
    use opencat_core::render_offscreen::compile_offscreen_commands;

    let id = str_arg(args, 0);
    let (w, h) = (num_arg(args, 1), num_arg(args, 2));
    let Some(out) = bytes_arg(args, 3) else {
        web_sys::console::error_1(&JsValue::from_str("rasterCanvas: `out` must be a Uint8Array"));
        return JsValue::from(0);
    };
    let w = w.max(1.0) as i32;
    let h = h.max(1.0) as i32;

    let (canvas, empty) = {
        let mut guard = store.borrow_mut();
        match guard.canvas_commands(&id) {
            None => (None, true),
            Some(c) if c.commands.is_empty() => (None, true),
            Some(c) => (Some(c.clone()), false),
        }
    };
    let need = (w as f32 * h as f32) as usize * 4;
    if out.length() < need as u32 {
        web_sys::console::error_1(&JsValue::from_str(
            "rasterCanvas: destination Uint8Array smaller than canvas",
        ));
        return JsValue::from(0);
    }
    if empty {
        // All-transparent canvas (out is zero-initialized — nothing to fill).
        // 诊断：屏幕 CanvasTexture 靠这里回放 2D 命令——空命令=透明黑贴图。
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "rasterCanvas: no/empty commands for '{id}' ({w}x{h})"
        )));
        return JsValue::from(0);
    }
    let Some(canvas) = canvas else {
        unreachable!("canvas_commands vanished between empty check and clone");
    };

    let frame = match compile_offscreen_commands(&canvas, w as f32, h as f32) {
        Ok(f) => f,
        Err(e) => {
            web_sys::console::error_1(&JsValue::from_str(&format!("rasterCanvas compile: {e}")));
            return JsValue::from(0);
        }
    };
    let mut scratch = DrawFrameScratch::default();
    let encoded = encode_draw_sections(&frame, &mut scratch);
    let bytes = match pack_ir_envelope(&frame, &[], None, &encoded) {
        Ok(b) => b,
        Err(e) => {
            web_sys::console::error_1(&JsValue::from_str(&format!("rasterCanvas encode: {e}")));
            return JsValue::from(0);
        }
    };

    let global = js_sys::global();
    let hook = js_sys::Reflect::get(&global, &JsValue::from_str("__ocRasterCanvasReplay"))
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
    let Some(hook) = hook else {
        web_sys::console::error_1(&JsValue::from_str(
            "rasterCanvas: host replay hook __ocRasterCanvasReplay unavailable",
        ));
        return JsValue::from(0);
    };
    let js_bytes = js_sys::Uint8Array::from(&bytes[..]);
    match hook.call4(
        &JsValue::UNDEFINED,
        &JsValue::from_str(&id),
        &JsValue::from(w as u32),
        &JsValue::from(h as u32),
        &js_bytes,
    ) {
        Ok(promise_or_val) => {
            // The hook is synchronous (CPU raster surface); a stray promise
            // would mean the host broke its contract — surface the mismatch.
            let rgba = promise_or_val.dyn_into::<js_sys::Uint8Array>().ok();
            let Some(rgba) = rgba else {
                web_sys::console::error_1(&JsValue::from_str(
                    "rasterCanvas: replay hook returned no pixels (CanvasKit unavailable?)",
                ));
                return JsValue::from(0);
            };
            let len = (rgba.length() as usize).min(out.length() as usize);
            out.set(&rgba.subarray(0, len as u32), 0);
            JsValue::from(len as u32)
        }
        Err(e) => {
            web_sys::console::error_1(&JsValue::from_str(&format!(
                "rasterCanvas replay: {}",
                js_err_to_string(&e)
            )));
            JsValue::from(0)
        }
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

// -- @omp natives（engine v8.rs 同语义的 js_sys 版） --

fn js_value_to_json(v: &JsValue) -> anyhow::Result<JsonValue> {
    let s = js_sys::JSON::stringify(v)
        .map_err(|e| anyhow::anyhow!("stringify: {}", js_err_to_string(&e)))?
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("stringify: not a string"))?;
    Ok(serde_json::from_str(&s)?)
}

/// `__ocOmpPrep(ir)` — core omp_prep（bundle 编译缓存）。true = 骨架可跑；
/// false = 编译失败（条目毒化），原循环照跑。
fn omp_prep_native(ir_js: &JsValue) -> JsValue {
    let ok = js_value_to_json(ir_js)
        .map(|ir| opencat_core::script::omp::omp_prep(&ir))
        .unwrap_or(false);
    JsValue::from_bool(ok)
}

/// 数字按 `JSON.stringify` 文本口径落值（engine `json_number` 同码）：
/// 非有限 → null；整数值 |n|<1e21 落整数，其余浮点。
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
    JsonValue::from(n)
}

/// TypedArray 节点 → (kind, 字节快照)。九种具名 TA 逐一 instanceof；
/// 非 TA = None。
fn ta_kind_bytes(v: &JsValue) -> Option<(SharedKind, Vec<u8>)> {
    macro_rules! try_ta {
        ($t:ident, $k:expr) => {
            if let Ok(ta) = v.clone().dyn_into::<js_sys::$t>() {
                let bytes = js_sys::Uint8Array::new_with_byte_offset_and_length(
                    &ta.buffer(),
                    ta.byte_offset(),
                    ta.byte_length(),
                );
                return Some(($k, bytes.to_vec()));
            }
        };
    }
    try_ta!(Float64Array, SharedKind::F64);
    try_ta!(Float32Array, SharedKind::F32);
    try_ta!(Uint8ClampedArray, SharedKind::U8Clamped);
    try_ta!(Uint8Array, SharedKind::U8);
    try_ta!(Int8Array, SharedKind::I8);
    try_ta!(Uint16Array, SharedKind::U16);
    try_ta!(Int16Array, SharedKind::I16);
    try_ta!(Uint32Array, SharedKind::U32);
    try_ta!(Int32Array, SharedKind::I32);
    None
}

enum Walk {
    Val(JsonValue),
    /// stringify 省略的面：对象成员的 function/undefined 值（键省略）。
    Omit,
    /// walker 不复刻的特例（toJSON / Date / 环 / 深度）→ 整子树回落
    /// stringify 老路（环 → stringify 抛 → null，与旧路径同值）。
    Fallback,
}

/// plain 实参树 walker（engine `omp_plain_walk` 同语义）：TypedArray 转
/// 描述符 + 字节快照，其余按 stringify 语义递归。`visited` = identity 栈
/// （`Object.is` 精确比较，比 engine 的 identity hash 无假阳性）。
fn omp_walk(
    v: &JsValue,
    shared: &mut Vec<SharedBuf>,
    visited: &mut Vec<JsValue>,
    depth: u32,
) -> Walk {
    if let Some((kind, bytes)) = ta_kind_bytes(v) {
        let len = bytes.len() / kind.elem_size();
        let slot = shared.len();
        shared.push(SharedBuf {
            kind,
            data: bytes.into(),
        });
        return Walk::Val(serde_json::json!({
            "__ocOmpShared": { "slot": slot, "len": len }
        }));
    }
    if v.is_function() || v.is_undefined() {
        return Walk::Omit;
    }
    if v.is_null() {
        return Walk::Val(JsonValue::Null);
    }
    if let Some(s) = v.as_string() {
        return Walk::Val(JsonValue::String(s));
    }
    if let Some(n) = v.as_f64() {
        return Walk::Val(json_number(n));
    }
    if let Some(b) = v.as_bool() {
        return Walk::Val(JsonValue::Bool(b));
    }
    if depth > 64 || v.is_instance_of::<js_sys::Date>() {
        return Walk::Fallback;
    }
    if js_sys::Reflect::get(v, &JsValue::from_str("toJSON"))
        .map(|t| t.is_function())
        .unwrap_or(false)
    {
        return Walk::Fallback;
    }
    if visited.iter().any(|w| js_sys::Object::is(w, v)) {
        return Walk::Fallback;
    }
    visited.push(v.clone());
    if v.is_array() {
        let arr = js_sys::Array::from(v);
        let mut items = Vec::with_capacity(arr.length() as usize);
        for i in 0..arr.length() {
            match omp_walk(&arr.get(i), shared, visited, depth + 1) {
                Walk::Val(j) => items.push(j),
                // 空洞与函数元素 → null（stringify 同）。
                Walk::Omit => items.push(JsonValue::Null),
                Walk::Fallback => {
                    visited.pop();
                    return Walk::Fallback;
                }
            }
        }
        visited.pop();
        return Walk::Val(JsonValue::Array(items));
    }
    // plain object：own enumerable string 键（Object.keys = stringify 键集）。
    let obj = js_sys::Object::from(v.clone());
    let keys = js_sys::Object::keys(&obj);
    let mut map = serde_json::Map::new();
    for k in keys.iter() {
        let Ok(val) = js_sys::Reflect::get(v, &k) else {
            visited.pop();
            return Walk::Fallback;
        };
        if val.is_function() || val.is_undefined() {
            continue; // stringify 省略该键
        }
        match omp_walk(&val, shared, visited, depth + 1) {
            Walk::Val(j) => {
                map.insert(k.as_string().unwrap_or_default(), j);
            }
            Walk::Omit => {}
            Walk::Fallback => {
                visited.pop();
                return Walk::Fallback;
            }
        }
    }
    visited.pop();
    Walk::Val(JsonValue::Object(map))
}

/// `__ocOmpTake(ir, ...args, skeleton)`（变参经转发壳收进单数组）。
/// v8.rs omp_take 同流程：槽分类采集 → core omp_take（WebOmpTeam）→
/// 行序写回 → reds 返回。任何失败 = false（降级，原循环照跑）。
fn omp_take_native(store: &Rc<RefCell<MutationStore>>, args_js: &JsValue) -> JsValue {
    let fallback = JsValue::from_bool(false);
    let all = js_sys::Array::from(args_js);
    let get = |i: u32| all.get(i);

    let Ok(ir) = js_value_to_json(&get(0)) else {
        return fallback;
    };
    let arg_names: Vec<String> = ir
        .get("args")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let captures = ir.get("captures");
    let cap_is =
        |name: &str, kind: &str| captures.and_then(|c| c.get(name)).and_then(|v| v.as_str()) == Some(kind);
    let n_args = arg_names.len();

    // 槽分类采集：prng → null（骨架已预采）；array → 令牌（len 随令牌走）；
    // ctx → `{"__ocNodeId", "state"}`；plain → walker（TA 二进制旁路）。
    let mut rest: Vec<JsonValue> = Vec::with_capacity(n_args);
    let mut shared: Vec<SharedBuf> = Vec::new();
    for (k, name) in arg_names.iter().enumerate() {
        let v = get(k as u32 + 1);
        if cap_is(name, "prng") {
            rest.push(JsonValue::Null);
            continue;
        }
        if cap_is(name, "array") {
            let len = js_sys::Reflect::get(&v, &JsValue::from_str("length"))
                .ok()
                .and_then(|l| l.as_f64())
                .unwrap_or(0.0);
            if len <= 0.0 {
                return fallback;
            }
            rest.push(serde_json::json!({ "__ocOmpArr": k, "len": len as u64 }));
            continue;
        }
        if cap_is(name, "ctx") {
            let node_id = js_sys::Reflect::get(&v, &JsValue::from_str("__ocNodeId"))
                .ok()
                .and_then(|s| s.as_string());
            let alpha = js_sys::Reflect::get(&v, &JsValue::from_str("state"))
                .ok()
                .and_then(|st| js_sys::Reflect::get(&st, &JsValue::from_str("globalAlpha")).ok())
                .and_then(|ga| ga.as_f64());
            rest.push(match node_id {
                Some(id) => serde_json::json!({
                    "__ocNodeId": id,
                    "state": { "globalAlpha": alpha.unwrap_or(1.0) },
                }),
                None => JsonValue::Null, // 读不到 = 不是 ctx face → 毒化
            });
            continue;
        }
        if v.is_null() || v.is_undefined() {
            rest.push(JsonValue::Null);
            continue;
        }
        if let Some(s) = v.as_string() {
            rest.push(JsonValue::String(s));
            continue;
        }
        if let Some(n) = v.as_f64() {
            rest.push(json_number(n));
            continue;
        }
        if let Some(b) = v.as_bool() {
            rest.push(JsonValue::Bool(b));
            continue;
        }
        let mut visited = Vec::new();
        match omp_walk(&v, &mut shared, &mut visited, 0) {
            Walk::Val(j) => rest.push(j),
            Walk::Omit => rest.push(JsonValue::Null),
            Walk::Fallback => rest.push(js_value_to_json(&v).unwrap_or(JsonValue::Null)),
        }
    }

    // 末位 = 采集骨架（Float64Array；无采集时是 null）。
    let mut prng: Vec<f64> = Vec::new();
    if let Ok(fa) = get(n_args as u32 + 1).dyn_into::<js_sys::Float64Array>() {
        prng = fa.to_vec();
    }

    let mut writes: Vec<opencat_core::script::omp::protocol::IsolateWrite> = Vec::new();
    let mut reds: Vec<f64> = Vec::new();
    let shared: Arc<[SharedBuf]> = Arc::from(shared);
    let mut taken = match store.try_borrow_mut() {
        Ok(mut s) => opencat_core::script::omp::omp_take(
            &crate::omp_team::WebOmpTeam,
            &mut s,
            &ir,
            &rest,
            shared,
            &prng,
            &mut writes,
            &mut reds,
        ),
        Err(_) => false,
    };

    // take=true 时按行序把 Set/Acc 四元组应用到真数组。统一 API 路径
    // （Reflect get/set）：f64 精确读写；整型 TA 由浏览器 [[Set]] 原生
    // 转换（U8Clamped 的 ToUint8Clamp 同 ES 规范）；JS Array 与 engine
    // 同校验（acc 读洞 = false）。
    if taken {
        for w in &writes {
            let target = get(w.arg as u32 + 1);
            let key = JsValue::from(w.idx);
            let val = if w.acc {
                match js_sys::Reflect::get(&target, &key)
                    .ok()
                    .and_then(|c| c.as_f64())
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
            if !js_sys::Reflect::set(&target, &key, &JsValue::from(val)).unwrap_or(false) {
                taken = false;
                break;
            }
        }
    }

    // reduction 面：返回折叠值数组（守卫赋 `__ocOmpRed`）；无 red 走布尔。
    if taken && !reds.is_empty() {
        let arr = js_sys::Array::new();
        for r in &reds {
            arr.push(&JsValue::from(*r));
        }
        return arr.into();
    }
    JsValue::from_bool(taken)
}
