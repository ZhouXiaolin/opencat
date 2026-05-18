//! `#[wasm_bindgen]` extern "C" 绑定 CanvasKit JS API。
//!
//! 本文件只声明 类型 + 方法签名 + 工厂函数。语义实现都在 `canvas2d.rs`。
//! 添加新绑定的流程：① 在对应 extern 块加 `#[wasm_bindgen(method, ...)]` 行
//! ② 在 `canvas2d.rs` 对应方法里调用它。

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

use crate::canvaskit::handle::{CKHandle, CkImageMarker};

#[wasm_bindgen]
extern "C" {
    // ── Canvas ───────────────────────────────────────────────────

    pub type CKCanvas;

    #[wasm_bindgen(method, js_name = "save")]
    pub fn save(this: &CKCanvas) -> i32;
    #[wasm_bindgen(method, js_name = "restore")]
    pub fn restore(this: &CKCanvas);
    #[wasm_bindgen(method, js_name = "restoreToCount")]
    pub fn restore_to_count(this: &CKCanvas, count: i32);
    #[wasm_bindgen(method, js_name = "getSaveCount")]
    pub fn save_count(this: &CKCanvas) -> i32;
    #[wasm_bindgen(method, js_name = "saveLayer")]
    pub fn save_layer(this: &CKCanvas, paint: &JsValue, bounds: &JsValue) -> i32;
    #[wasm_bindgen(method, js_name = "translate")]
    pub fn translate(this: &CKCanvas, dx: f32, dy: f32);
    #[wasm_bindgen(method, js_name = "scale")]
    pub fn scale(this: &CKCanvas, sx: f32, sy: f32);
    #[wasm_bindgen(method, js_name = "rotate")]
    pub fn rotate(this: &CKCanvas, degrees: f32, cx: f32, cy: f32);
    #[wasm_bindgen(method, js_name = "skew")]
    pub fn skew(this: &CKCanvas, sx: f32, sy: f32);
    #[wasm_bindgen(method, js_name = "concat")]
    pub fn concat(this: &CKCanvas, m: &JsValue);
    #[wasm_bindgen(method, js_name = "clipRect")]
    pub fn clip_rect(this: &CKCanvas, rect: &JsValue, op: &JsValue, aa: bool);
    #[wasm_bindgen(method, js_name = "clipRRect")]
    pub fn clip_rrect(this: &CKCanvas, rrect: &JsValue, op: &JsValue, aa: bool);
    #[wasm_bindgen(method, js_name = "clipPath")]
    pub fn clip_path(this: &CKCanvas, path: &JsValue, op: &JsValue, aa: bool);
    #[wasm_bindgen(method, js_name = "clear")]
    pub fn clear(this: &CKCanvas, color: &JsValue);
    #[wasm_bindgen(method, js_name = "drawPaint")]
    pub fn draw_paint(this: &CKCanvas, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawRect")]
    pub fn draw_rect(this: &CKCanvas, rect: &JsValue, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawRRect")]
    pub fn draw_rrect(this: &CKCanvas, rrect: &JsValue, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawDRRect")]
    pub fn draw_drrect(this: &CKCanvas, outer: &JsValue, inner: &JsValue, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawOval")]
    pub fn draw_oval(this: &CKCanvas, oval: &JsValue, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawCircle")]
    pub fn draw_circle(this: &CKCanvas, cx: f32, cy: f32, r: f32, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawArc")]
    pub fn draw_arc(
        this: &CKCanvas,
        oval: &JsValue,
        start: f32,
        sweep: f32,
        use_center: bool,
        paint: &JsValue,
    );
    #[wasm_bindgen(method, js_name = "drawLine")]
    pub fn draw_line(this: &CKCanvas, x0: f32, y0: f32, x1: f32, y1: f32, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawPoints")]
    pub fn draw_points(this: &CKCanvas, mode: &JsValue, pts: &JsValue, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawPath")]
    pub fn draw_path(this: &CKCanvas, path: &JsValue, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawImage")]
    pub fn draw_image(this: &CKCanvas, image: &JsValue, x: f32, y: f32, paint: &JsValue);
    #[wasm_bindgen(method, js_name = "drawImageRect")]
    pub fn draw_image_rect(
        this: &CKCanvas,
        image: &JsValue,
        src: &JsValue,
        dst: &JsValue,
        paint: &JsValue,
    );
    #[wasm_bindgen(method, js_name = "drawPicture")]
    pub fn draw_picture(this: &CKCanvas, picture: &JsValue);
    #[wasm_bindgen(method, js_name = "drawSimpleText")]
    pub fn draw_simple_text(
        this: &CKCanvas,
        text: &str,
        x: f32,
        y: f32,
        font: &JsValue,
        paint: &JsValue,
    );
    #[wasm_bindgen(method, js_name = "drawGlyphs")]
    pub fn draw_glyphs(
        this: &CKCanvas,
        glyphs: &JsValue,
        positions: &JsValue,
        x: f32,
        y: f32,
        font: &JsValue,
        paint: &JsValue,
    );

    // ── Paint（轻量 JS 对象，由 V8 GC 管理，不走 CKHandle）──

    pub type CKPaint;

    #[wasm_bindgen(constructor)]
    pub fn new() -> CKPaint;
    #[wasm_bindgen(method, js_name = "setColor")]
    pub fn set_color(this: &CKPaint, color: &JsValue);
    #[wasm_bindgen(method, js_name = "setAlphaf")]
    pub fn set_alpha(this: &CKPaint, a: f32);
    #[wasm_bindgen(method, js_name = "setAntiAlias")]
    pub fn set_anti_alias(this: &CKPaint, aa: bool);
    #[wasm_bindgen(method, js_name = "setBlendMode")]
    pub fn set_blend_mode(this: &CKPaint, mode: &JsValue);
    #[wasm_bindgen(method, js_name = "setStyle")]
    pub fn set_style(this: &CKPaint, style: &JsValue);
    #[wasm_bindgen(method, js_name = "setStrokeWidth")]
    pub fn set_stroke_width(this: &CKPaint, w: f32);
    #[wasm_bindgen(method, js_name = "setStrokeCap")]
    pub fn set_stroke_cap(this: &CKPaint, cap: &JsValue);
    #[wasm_bindgen(method, js_name = "setStrokeJoin")]
    pub fn set_stroke_join(this: &CKPaint, join: &JsValue);
    #[wasm_bindgen(method, js_name = "setStrokeMiter")]
    pub fn set_stroke_miter(this: &CKPaint, limit: f32);
    #[wasm_bindgen(method, js_name = "setShader")]
    pub fn set_shader(this: &CKPaint, shader: &JsValue);
    #[wasm_bindgen(method, js_name = "setImageFilter")]
    pub fn set_image_filter(this: &CKPaint, filter: &JsValue);
    #[wasm_bindgen(method, js_name = "setColorFilter")]
    pub fn set_color_filter(this: &CKPaint, filter: &JsValue);
    #[wasm_bindgen(method, js_name = "setMaskFilter")]
    pub fn set_mask_filter(this: &CKPaint, filter: &JsValue);
    #[wasm_bindgen(method, js_name = "setPathEffect")]
    pub fn set_path_effect(this: &CKPaint, effect: &JsValue);
}

// ── 工厂函数（包装 CK 模块上的全局函数）──

/// `CanvasKit.MakeImageFromEncoded(bytes)` → `Option<CKImage>`。
pub fn ck_make_image_from_encoded(bytes: &[u8]) -> Option<CKHandle<CkImageMarker>> {
    let m = crate::canvaskit::module::ck();
    let arr = js_sys::Uint8Array::from(bytes);
    let f = js_sys::Reflect::get(m, &JsValue::from_str("MakeImageFromEncoded")).ok()?;
    let func = f.dyn_ref::<js_sys::Function>()?;
    let r = func.call1(m, &arr).ok()?;
    if r.is_null() || r.is_undefined() {
        return None;
    }
    Some(CKHandle::wrap(r))
}

/// `CanvasKit.LTRBRect(l, t, r, b)`。
pub fn ck_ltrb_rect(l: f32, t: f32, r: f32, b: f32) -> JsValue {
    let m = crate::canvaskit::module::ck();
    let f = js_sys::Reflect::get(m, &JsValue::from_str("LTRBRect"))
        .expect("LTRBRect missing on CanvasKit module");
    let func = f
        .dyn_ref::<js_sys::Function>()
        .expect("LTRBRect not callable");
    func.call4(
        m,
        &JsValue::from_f64(l as f64),
        &JsValue::from_f64(t as f64),
        &JsValue::from_f64(r as f64),
        &JsValue::from_f64(b as f64),
    )
    .unwrap_or(JsValue::UNDEFINED)
}

/// `CanvasKit.Color4f(r, g, b, a)`。
pub fn ck_color4f(r: f32, g: f32, b: f32, a: f32) -> JsValue {
    let m = crate::canvaskit::module::ck();
    let f = js_sys::Reflect::get(m, &JsValue::from_str("Color4f"))
        .expect("Color4f missing on CanvasKit module");
    let func = f
        .dyn_ref::<js_sys::Function>()
        .expect("Color4f not callable");
    func.call4(
        m,
        &JsValue::from_f64(r as f64),
        &JsValue::from_f64(g as f64),
        &JsValue::from_f64(b as f64),
        &JsValue::from_f64(a as f64),
    )
    .unwrap_or(JsValue::UNDEFINED)
}
