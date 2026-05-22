//! CanvasKit 后端：用 `#[wasm_bindgen]` 直接绑 CanvasKit JS API，
//! 提供 `CanvasKitCanvas2D: Canvas2D` 实现。
//!
//! 仅在 wasm32 平台编译；native build 跳过整棵子树。

pub mod bindings;
pub mod canvas2d;
pub mod convert;
pub mod handle;
pub mod module;
pub mod paint;

pub use canvas2d::CanvasKitCanvas2D;
pub use handle::{
    CKColorFilterHandle, CKHandle, CKImage, CKImageFilterHandle, CKMaskFilterHandle, CKPath,
    CKPathEffectHandle, CKPicture, CKRuntimeEffect, CKShaderHandle, CKSurface,
};
pub use module::init_canvaskit;
