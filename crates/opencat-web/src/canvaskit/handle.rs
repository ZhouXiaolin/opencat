//! CKHandle<T>：CanvasKit C++ 对象的 Rust 持有句柄。
//!
//! - 用 `Arc<Inner>` 共享所有权，`Drop` 时调用 JS 端 `.delete()` 释放内存。
//! - 类型参数 `T` 是 phantom marker（CkPathMarker、CkImageMarker 等），区分
//!   不同 CanvasKit 类型而不增加运行时开销。
//! - **CKPaint 不走 CKHandle**：CanvasKit Paint 是普通 JS 对象（V8 GC 管理），
//!   没有 `.delete()` 方法。`CanvasKitCanvas2D` 直接持有 `bindings::CKPaint`。


use std::marker::PhantomData;
use std::sync::Arc;
use wasm_bindgen::{JsCast, JsValue};

pub struct CKHandle<T> {
    inner: Arc<CKHandleInner>,
    _marker: PhantomData<T>,
}

struct CKHandleInner(JsValue);

impl Drop for CKHandleInner {
    fn drop(&mut self) {
        if let Some(obj) = self.0.dyn_ref::<js_sys::Object>() {
            if let Ok(delete_fn) = js_sys::Reflect::get(obj, &JsValue::from_str("delete")) {
                if let Some(f) = delete_fn.dyn_ref::<js_sys::Function>() {
                    let _ = f.call0(obj);
                }
            }
        }
    }
}

impl<T> CKHandle<T> {
    pub fn wrap(js: JsValue) -> Self {
        Self {
            inner: Arc::new(CKHandleInner(js)),
            _marker: PhantomData,
        }
    }

    pub fn as_js(&self) -> &JsValue {
        &self.inner.0
    }
}

impl<T> Clone for CKHandle<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _marker: PhantomData,
        }
    }
}

// ── Phantom markers ──────────────────────────────────────────────

pub enum CkPathMarker {}
pub enum CkImageMarker {}
pub enum CkPictureMarker {}
pub enum CkRuntimeEffectMarker {}
pub enum CkShaderMarker {}
pub enum CkImageFilterMarker {}
pub enum CkColorFilterMarker {}
pub enum CkMaskFilterMarker {}
pub enum CkPathEffectMarker {}
pub enum CkSurfaceMarker {}

pub type CKPath = CKHandle<CkPathMarker>;
pub type CKImage = CKHandle<CkImageMarker>;
pub type CKPicture = CKHandle<CkPictureMarker>;
pub type CKRuntimeEffect = CKHandle<CkRuntimeEffectMarker>;
pub type CKShaderHandle = CKHandle<CkShaderMarker>;
pub type CKImageFilterHandle = CKHandle<CkImageFilterMarker>;
pub type CKColorFilterHandle = CKHandle<CkColorFilterMarker>;
pub type CKMaskFilterHandle = CKHandle<CkMaskFilterMarker>;
pub type CKPathEffectHandle = CKHandle<CkPathEffectMarker>;
pub type CKSurface = CKHandle<CkSurfaceMarker>;
