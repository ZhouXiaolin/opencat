//! Holds the CanvasKit JS module handle. JS 启动时通过
//! `globalThis.__canvasKit = CanvasKitInit(...)` 注入；Rust 端通过
//! `init_canvaskit()` 把它装到全局 OnceCell 里。

#![cfg(target_arch = "wasm32")]

use once_cell::sync::OnceCell;
use wasm_bindgen::prelude::*;

static CK_MODULE: OnceCell<JsValue> = OnceCell::new();

/// 暴露给 JS 调用：把 `globalThis.__canvasKit` 装载到 Rust 侧 OnceCell。
/// 在 `CanvasKitInit()` 完成、`init()`（wasm bindgen 启动）之后调用。
#[wasm_bindgen]
pub fn init_canvaskit() -> Result<(), JsValue> {
    let global = js_sys::global();
    let ck = js_sys::Reflect::get(&global, &JsValue::from_str("__canvasKit"))?;
    if ck.is_undefined() {
        return Err(JsValue::from_str(
            "__canvasKit not set; call CanvasKitInit first",
        ));
    }
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

use crate::canvaskit::bindings::CKTypefaceJs;
use wasm_bindgen::JsCast;

const EMBEDDED_FONT: &[u8] = include_bytes!("../../../../assets/NotoSansSC-Regular.otf");

thread_local! {
    static DEFAULT_TYPEFACE: std::cell::RefCell<Option<CKTypefaceJs>> = std::cell::RefCell::new(None);
}

fn ensure_default_typeface() {
    DEFAULT_TYPEFACE.with(|tf| {
        if tf.borrow().is_none() {
            if let Some(typeface) = crate::canvaskit::bindings::ck_make_typeface_from_data(EMBEDDED_FONT) {
                *tf.borrow_mut() = Some(typeface);
            }
        }
    });
}

/// Set the default Typeface (overrides embedded NotoSansSC).
pub fn set_default_typeface(typeface: CKTypefaceJs) {
    DEFAULT_TYPEFACE.with(|tf| {
        *tf.borrow_mut() = Some(typeface);
    });
}

/// Get the default Typeface. Falls back to embedded NotoSansSC if none was injected.
pub fn default_typeface() -> Option<CKTypefaceJs> {
    ensure_default_typeface();
    DEFAULT_TYPEFACE.with(|tf| {
        tf.borrow().as_ref().map(|t| {
            let js: &JsValue = t.unchecked_ref();
            js.clone().unchecked_into::<CKTypefaceJs>()
        })
    })
}
