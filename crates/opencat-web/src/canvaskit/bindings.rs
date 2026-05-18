//! `#[wasm_bindgen]` extern "C" 绑定 CanvasKit JS API。
//!
//! 本文件只声明 类型 + 方法签名 + 工厂函数。语义实现都在 `canvas2d.rs`。
//! 添加新绑定的流程：① 在对应 extern 块加 `#[wasm_bindgen(method, ...)]` 行
//! ② 在 `canvas2d.rs` 对应方法里调用它。

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

use crate::canvaskit::handle::{CKHandle, CkColorFilterMarker, CkImageFilterMarker, CkImageMarker, CkMaskFilterMarker, CkPathEffectMarker, CkPathMarker, CkRuntimeEffectMarker, CkShaderMarker};

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

    // ── Path（CanvasKit 的 SkPath，由 CKHandle<CkPathMarker> 持有）──

    pub type CKPath;

    #[wasm_bindgen(method, js_name = "moveTo")]
    pub fn move_to(this: &CKPath, x: f32, y: f32);
    #[wasm_bindgen(method, js_name = "lineTo")]
    pub fn line_to(this: &CKPath, x: f32, y: f32);
    #[wasm_bindgen(method, js_name = "cubicTo")]
    pub fn cubic_to(this: &CKPath, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32);
    #[wasm_bindgen(method, js_name = "quadTo")]
    pub fn quad_to(this: &CKPath, cx: f32, cy: f32, x: f32, y: f32);
    #[wasm_bindgen(method, js_name = "close")]
    pub fn close_path(this: &CKPath);
    #[wasm_bindgen(method, js_name = "setFillType")]
    pub fn set_fill_type(this: &CKPath, fill: &JsValue);

    // ── Picture / PictureRecorder / Surface 实例方法 ──
    // （不走 CKHandle：调用者在录制/绘制完成后手动 delete）

    pub type CKPictureRecorder;

    #[wasm_bindgen(method, js_name = "beginRecording")]
    pub fn begin_recording(this: &CKPictureRecorder, bounds: &JsValue) -> CKCanvas;
    #[wasm_bindgen(method, js_name = "finishRecordingAsPicture")]
    pub fn finish_recording_as_picture(this: &CKPictureRecorder) -> JsValue;
    #[wasm_bindgen(method, js_name = "delete")]
    pub fn delete_recorder(this: &CKPictureRecorder);

    // CKImageJs：裸 JS 类型，用于访问 width/height；CKImage = CKHandle<CkImageMarker> 是句柄
    pub type CKImageJs;
    #[wasm_bindgen(method, js_name = "width")]
    pub fn image_width(this: &CKImageJs) -> u32;
    #[wasm_bindgen(method, js_name = "height")]
    pub fn image_height(this: &CKImageJs) -> u32;

    pub type CKSurfaceJs;

    #[wasm_bindgen(method, js_name = "getCanvas")]
    pub fn surface_get_canvas(this: &CKSurfaceJs) -> CKCanvas;
    #[wasm_bindgen(method, js_name = "makeImageSnapshot")]
    pub fn make_image_snapshot(this: &CKSurfaceJs) -> JsValue;
    #[wasm_bindgen(method, js_name = "flush")]
    pub fn surface_flush(this: &CKSurfaceJs);
    #[wasm_bindgen(method, js_name = "delete")]
    pub fn delete_surface(this: &CKSurfaceJs);

    // ── RuntimeEffect ──
    pub type CKRuntimeEffectJs;

    #[wasm_bindgen(method, js_name = "makeShader")]
    pub fn make_shader(this: &CKRuntimeEffectJs, uniforms: &JsValue, children: &JsValue) -> JsValue;
    #[wasm_bindgen(method, js_name = "delete")]
    pub fn delete_effect(this: &CKRuntimeEffectJs);

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

/// `new CanvasKit.Path()` —— 创建空 Path。
pub fn ck_new_path() -> Option<CKHandle<CkPathMarker>> {
    let m = crate::canvaskit::module::ck();
    let ctor = js_sys::Reflect::get(m, &JsValue::from_str("Path")).ok()?;
    let ctor_fn = ctor.dyn_ref::<js_sys::Function>()?;
    let args = js_sys::Array::new();
    let path = js_sys::Reflect::construct(ctor_fn, &args).ok()?;
    if path.is_null() || path.is_undefined() {
        return None;
    }
    Some(CKHandle::wrap(path))
}

/// `CanvasKit.Path.MakeFromSVGString(svg)` —— 解析 SVG path data。
pub fn ck_path_from_svg(svg: &str) -> Option<CKHandle<CkPathMarker>> {
    let m = crate::canvaskit::module::ck();
    let path_class = js_sys::Reflect::get(m, &JsValue::from_str("Path")).ok()?;
    let f = js_sys::Reflect::get(&path_class, &JsValue::from_str("MakeFromSVGString")).ok()?;
    let func = f.dyn_ref::<js_sys::Function>()?;
    let r = func.call1(&path_class, &JsValue::from_str(svg)).ok()?;
    if r.is_null() || r.is_undefined() {
        return None;
    }
    Some(CKHandle::wrap(r))
}

/// `new CanvasKit.PictureRecorder()`。
pub fn ck_new_picture_recorder() -> Option<CKPictureRecorder> {
    let m = crate::canvaskit::module::ck();
    let class = js_sys::Reflect::get(m, &JsValue::from_str("PictureRecorder")).ok()?;
    let ctor = class.dyn_ref::<js_sys::Function>()?;
    let args = js_sys::Array::new();
    let inst = js_sys::Reflect::construct(ctor, &args).ok()?;
    if inst.is_null() || inst.is_undefined() {
        return None;
    }
    Some(inst.unchecked_into::<CKPictureRecorder>())
}

/// `CanvasKit.MakeImage(info, bytes, bytesPerRow)` → `Option<CKImage>`。
pub fn ck_make_image_from_rgba(
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Option<CKHandle<CkImageMarker>> {
    let m = crate::canvaskit::module::ck();

    let info = js_sys::Object::new();
    js_sys::Reflect::set(
        &info,
        &JsValue::from_str("width"),
        &JsValue::from_f64(width as f64),
    )
    .ok()?;
    js_sys::Reflect::set(
        &info,
        &JsValue::from_str("height"),
        &JsValue::from_f64(height as f64),
    )
    .ok()?;

    let alpha_type = {
        let at = js_sys::Reflect::get(m, &JsValue::from_str("AlphaType")).ok()?;
        js_sys::Reflect::get(&at, &JsValue::from_str("Unpremul")).ok()?
    };
    js_sys::Reflect::set(&info, &JsValue::from_str("alphaType"), &alpha_type).ok()?;

    let color_type = {
        let ct = js_sys::Reflect::get(m, &JsValue::from_str("ColorType")).ok()?;
        js_sys::Reflect::get(&ct, &JsValue::from_str("RGBA_8888")).ok()?
    };
    js_sys::Reflect::set(&info, &JsValue::from_str("colorType"), &color_type).ok()?;

    let arr = js_sys::Uint8Array::from(bytes);
    let f = js_sys::Reflect::get(m, &JsValue::from_str("MakeImage")).ok()?;
    let func = f.dyn_ref::<js_sys::Function>()?;
    let row_bytes = JsValue::from_f64((width * 4) as f64);
    let result = func.call3(m, &info, &arr, &row_bytes).ok()?;
    if result.is_null() || result.is_undefined() {
        return None;
    }
    Some(CKHandle::wrap(result))
}

/// `CanvasKit.MakeSurface(width, height)` —— offscreen raster surface。
pub fn ck_make_surface(width: u32, height: u32) -> Option<CKSurfaceJs> {
    let m = crate::canvaskit::module::ck();
    let f = js_sys::Reflect::get(m, &JsValue::from_str("MakeSurface")).ok()?;
    let func = f.dyn_ref::<js_sys::Function>()?;
    let result = func
        .call2(
            m,
            &JsValue::from_f64(width as f64),
            &JsValue::from_f64(height as f64),
        )
        .ok()?;
    if result.is_null() || result.is_undefined() {
        return None;
    }
    Some(result.unchecked_into::<CKSurfaceJs>())
}

/// `CanvasKit.RuntimeEffect.Make(sksl)` → `Option<CKHandle<CkRuntimeEffectMarker>>`.
pub fn ck_make_runtime_effect(
    sksl: &str,
) -> Option<CKHandle<CkRuntimeEffectMarker>> {
    let m = crate::canvaskit::module::ck();
    let re_class = js_sys::Reflect::get(m, &JsValue::from_str("RuntimeEffect")).ok()?;
    let make_fn = js_sys::Reflect::get(&re_class, &JsValue::from_str("Make")).ok()?;
    let func = make_fn.dyn_ref::<js_sys::Function>()?;
    let result = func.call1(&re_class, &JsValue::from_str(sksl)).ok()?;
    if result.is_null() || result.is_undefined() {
        return None;
    }
    Some(CKHandle::wrap(result))
}

fn ck_tile_mode(tm: opencat_core::canvas::paint::TileMode) -> JsValue {
    use opencat_core::canvas::paint::TileMode;
    let v = match tm {
        TileMode::Clamp => "Clamp",
        TileMode::Repeat => "Repeat",
        TileMode::Mirror => "Mirror",
        TileMode::Decal => "Decal",
    };
    let m = crate::canvaskit::module::ck();
    let group = js_sys::Reflect::get(m, &JsValue::from_str("TileMode")).unwrap_or(JsValue::UNDEFINED);
    js_sys::Reflect::get(&group, &JsValue::from_str(v)).unwrap_or(JsValue::UNDEFINED)
}

/// Convert core `ShaderSpec` to CanvasKit Shader handle.
pub fn build_ck_shader(
    spec: &opencat_core::canvas::paint::ShaderSpec,
) -> Option<CKHandle<CkShaderMarker>> {
    use opencat_core::canvas::paint::ShaderSpec;
    let m = crate::canvaskit::module::ck();

    // Shared: flatten RGBA color tuples into a Float32Array.
    let colors_to_arr = |colors: &Vec<[f32; 4]>| -> js_sys::Float32Array {
        let color_count = colors.len() as u32;
        let arr = js_sys::Float32Array::new_with_length(color_count * 4);
        for (i, c) in colors.iter().enumerate() {
            arr.set_index((i * 4) as u32, c[0]);
            arr.set_index((i * 4 + 1) as u32, c[1]);
            arr.set_index((i * 4 + 2) as u32, c[2]);
            arr.set_index((i * 4 + 3) as u32, c[3]);
        }
        arr
    };

    // Shared: convert stop positions to JsValue (Float32Array or NULL).
    let stops_to_js = |stops: &Vec<f32>| -> JsValue {
        if stops.is_empty() {
            JsValue::NULL
        } else {
            let arr = js_sys::Float32Array::new_with_length(stops.len() as u32);
            for (i, &v) in stops.iter().enumerate() { arr.set_index(i as u32, v); }
            arr.into()
        }
    };

    match spec {
        ShaderSpec::LinearGradient { from, to, stops, colors, tile_mode, .. } => {
            let shader_class = js_sys::Reflect::get(m, &JsValue::from_str("Shader")).ok()?;
            let make_fn = js_sys::Reflect::get(&shader_class, &JsValue::from_str("MakeLinearGradient")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;

            let color_arr = colors_to_arr(colors);
            let pos_js = stops_to_js(stops);

            let start_arr = js_sys::Array::new();
            start_arr.push(&JsValue::from_f64(from[0] as f64));
            start_arr.push(&JsValue::from_f64(from[1] as f64));
            let end_arr = js_sys::Array::new();
            end_arr.push(&JsValue::from_f64(to[0] as f64));
            end_arr.push(&JsValue::from_f64(to[1] as f64));

            let tile_js = ck_tile_mode(*tile_mode);

            let result = func.call5(
                &shader_class,
                &start_arr, &end_arr, &color_arr, &pos_js, &tile_js,
            ).ok()?;
            if result.is_null() || result.is_undefined() { return None; }
            Some(CKHandle::wrap(result))
        }
        ShaderSpec::RadialGradient { center, radius, stops, colors, tile_mode, .. } => {
            let shader_class = js_sys::Reflect::get(m, &JsValue::from_str("Shader")).ok()?;
            let make_fn = js_sys::Reflect::get(&shader_class, &JsValue::from_str("MakeRadialGradient")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;

            let color_arr = colors_to_arr(colors);
            let pos_js = stops_to_js(stops);

            let center_arr = js_sys::Array::new();
            center_arr.push(&JsValue::from_f64(center[0] as f64));
            center_arr.push(&JsValue::from_f64(center[1] as f64));

            let tile_js = ck_tile_mode(*tile_mode);

            let result = func.call5(
                &shader_class,
                &center_arr,
                &JsValue::from_f64(*radius as f64),
                &color_arr,
                &pos_js,
                &tile_js,
            ).ok()?;
            if result.is_null() || result.is_undefined() { return None; }
            Some(CKHandle::wrap(result))
        }
    }
}

/// Convert core `ColorFilterSpec` to CanvasKit ColorFilter handle.
pub fn build_ck_color_filter(
    spec: &opencat_core::canvas::paint::ColorFilterSpec,
) -> Option<CKHandle<CkColorFilterMarker>> {
    use opencat_core::canvas::paint::ColorFilterSpec;
    let m = crate::canvaskit::module::ck();
    let cf_class = js_sys::Reflect::get(m, &JsValue::from_str("ColorFilter")).ok()?;

    match spec {
        ColorFilterSpec::Matrix(matrix) => {
            let arr = js_sys::Float32Array::new_with_length(20);
            for (i, &v) in matrix.iter().enumerate() {
                arr.set_index(i as u32, v);
            }
            let make_fn = js_sys::Reflect::get(&cf_class, &JsValue::from_str("MakeMatrix")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let result = func.call1(&cf_class, &arr).ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
        ColorFilterSpec::BlendColor { color, mode } => {
            let c = ck_color4f(color[0], color[1], color[2], color[3]);
            let bm = crate::canvaskit::convert::ck_blend_mode(*mode);
            let make_fn = js_sys::Reflect::get(&cf_class, &JsValue::from_str("MakeBlend")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let result = func.call2(&cf_class, &c, &bm).ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
        ColorFilterSpec::LinearToSrgbGamma => {
            let make_fn =
                js_sys::Reflect::get(&cf_class, &JsValue::from_str("MakeLinearToSRGBGamma"))
                    .ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let result = func.call0(&cf_class).ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
        ColorFilterSpec::SrgbToLinearGamma => {
            let make_fn =
                js_sys::Reflect::get(&cf_class, &JsValue::from_str("MakeSRGBToLinearGamma"))
                    .ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let result = func.call0(&cf_class).ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
    }
}

/// Convert core `ImageFilterSpec` to CanvasKit ImageFilter handle.
pub fn build_ck_image_filter(
    spec: &opencat_core::canvas::paint::ImageFilterSpec,
) -> Option<CKHandle<CkImageFilterMarker>> {
    use opencat_core::canvas::paint::ImageFilterSpec;
    let m = crate::canvaskit::module::ck();
    let if_class = js_sys::Reflect::get(m, &JsValue::from_str("ImageFilter")).ok()?;

    match spec {
        ImageFilterSpec::Blur {
            sigma_x,
            sigma_y,
            ..
        } => {
            let make_fn = js_sys::Reflect::get(&if_class, &JsValue::from_str("MakeBlur")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let tile_mode = ck_tile_mode(opencat_core::canvas::paint::TileMode::Clamp);
            let result = func
                .call3(
                    &if_class,
                    &JsValue::from_f64(*sigma_x as f64),
                    &JsValue::from_f64(*sigma_y as f64),
                    &tile_mode,
                )
                .ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
        ImageFilterSpec::DropShadow {
            dx,
            dy,
            sigma_x,
            sigma_y,
            color,
        } => {
            let make_fn =
                js_sys::Reflect::get(&if_class, &JsValue::from_str("MakeDropShadow")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let color_js = ck_color4f(color[0], color[1], color[2], color[3]);
            let result = func
                .call5(
                    &if_class,
                    &JsValue::from_f64(*dx as f64),
                    &JsValue::from_f64(*dy as f64),
                    &JsValue::from_f64(*sigma_x as f64),
                    &JsValue::from_f64(*sigma_y as f64),
                    &color_js,
                )
                .ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
        ImageFilterSpec::ColorFilter(cf) => {
            let color_filter = build_ck_color_filter(cf)?;
            let make_fn =
                js_sys::Reflect::get(&if_class, &JsValue::from_str("MakeColorFilter")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let result = func.call1(&if_class, color_filter.as_js()).ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
        ImageFilterSpec::Compose(outer, inner) => {
            let outer_h = build_ck_image_filter(outer)?;
            let inner_h = build_ck_image_filter(inner)?;
            let make_fn =
                js_sys::Reflect::get(&if_class, &JsValue::from_str("MakeCompose")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;
            let result = func
                .call2(&if_class, outer_h.as_js(), inner_h.as_js())
                .ok()?;
            if result.is_null() || result.is_undefined() {
                return None;
            }
            Some(CKHandle::wrap(result))
        }
    }
}

/// Convert core `MaskFilterSpec` to CanvasKit MaskFilter handle.
pub fn build_ck_mask_filter(
    spec: &opencat_core::canvas::paint::MaskFilterSpec,
) -> Option<CKHandle<CkMaskFilterMarker>> {
    use opencat_core::canvas::paint::MaskFilterSpec;
    let m = crate::canvaskit::module::ck();

    match spec {
        MaskFilterSpec::Blur { sigma, style, respect_ctm } => {
            let mf_class = js_sys::Reflect::get(m, &JsValue::from_str("MaskFilter")).ok()?;
            let make_fn = js_sys::Reflect::get(&mf_class, &JsValue::from_str("MakeBlur")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;

            let style_name = match style {
                opencat_core::canvas::paint::BlurStyle::Normal => "Normal",
                opencat_core::canvas::paint::BlurStyle::Inner => "Inner",
                opencat_core::canvas::paint::BlurStyle::Solid => "Solid",
                opencat_core::canvas::paint::BlurStyle::Outer => "Outer",
            };
            let bs_group = js_sys::Reflect::get(m, &JsValue::from_str("BlurStyle"))
                .unwrap_or(JsValue::UNDEFINED);
            let style_js = js_sys::Reflect::get(&bs_group, &JsValue::from_str(style_name))
                .unwrap_or(JsValue::UNDEFINED);

            let result = func.call3(
                &mf_class,
                &JsValue::from_f64(*sigma as f64),
                &style_js,
                &JsValue::from_bool(*respect_ctm),
            ).ok()?;
            if result.is_null() || result.is_undefined() { return None; }
            Some(CKHandle::wrap(result))
        }
    }
}

/// Convert core `PathEffectSpec` to CanvasKit PathEffect handle.
pub fn build_ck_path_effect(
    spec: &opencat_core::canvas::paint::PathEffectSpec,
) -> Option<CKHandle<CkPathEffectMarker>> {
    use opencat_core::canvas::paint::PathEffectSpec;
    let m = crate::canvaskit::module::ck();

    match spec {
        PathEffectSpec::Dash { intervals, phase } => {
            let pe_class = js_sys::Reflect::get(m, &JsValue::from_str("PathEffect")).ok()?;
            let make_fn = js_sys::Reflect::get(&pe_class, &JsValue::from_str("MakeDash")).ok()?;
            let func = make_fn.dyn_ref::<js_sys::Function>()?;

            let arr = js_sys::Float32Array::new_with_length(intervals.len() as u32);
            for (i, &v) in intervals.iter().enumerate() { arr.set_index(i as u32, v); }
            let result = func.call2(&pe_class, &arr, &JsValue::from_f64(*phase as f64)).ok()?;
            if result.is_null() || result.is_undefined() { return None; }
            Some(CKHandle::wrap(result))
        }
    }
}

