//! Holds the CanvasKit JS module handle. JS 启动时通过
//! `globalThis.__canvasKit = CanvasKitInit(...)` 注入；Rust 端通过
//! `init_canvaskit()` 把它装到全局 OnceCell 里。


use once_cell::sync::OnceCell;
use wasm_bindgen::prelude::*;

static CK_MODULE: OnceCell<JsValue> = OnceCell::new();

/// 暴露给 JS 调用：把 `globalThis.__canvasKit` 装载到 Rust 侧 OnceCell，
/// 同时把 `CKPaint` 注册为全局变量（wasm-bindgen 构造器需要）。
#[wasm_bindgen]
pub fn init_canvaskit() -> Result<(), JsValue> {
    let global = js_sys::global();
    let ck = js_sys::Reflect::get(&global, &JsValue::from_str("__canvasKit"))?;
    if ck.is_undefined() {
        return Err(JsValue::from_str(
            "__canvasKit not set; call CanvasKitInit first",
        ));
    }

    // wasm-bindgen constructor for CKPaint generates `new CKPaint()` in JS,
    // so we need CKPaint as a global alias for CanvasKit.Paint.
    let ck_paint = js_sys::Reflect::get(&ck, &JsValue::from_str("Paint"))?;
    js_sys::Reflect::set(&global, &JsValue::from_str("CKPaint"), &ck_paint)?;

    CK_MODULE
        .set(ck)
        .map_err(|_| JsValue::from_str("canvaskit already initialized"))?;
    Ok(())
}

/// 取得已装载的 CanvasKit JS 模块。第一次调用前必须先 [`init_canvaskit`]。
pub(crate) fn ck() -> &'static JsValue {
    CK_MODULE
        .get()
        .expect("init_canvaskit() must be called before any CanvasKit usage")
}
