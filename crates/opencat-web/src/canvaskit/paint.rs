//! `PaintSpec` → `CKPaint` 转换。覆盖 Solid / Stroke / AA / BlendMode / PaintStyle。
//! Shader / ImageFilter / ColorFilter / MaskFilter / PathEffect 已实现。

use opencat_core::canvas::paint::{FillSpec, PaintSpec, PaintStyle};

use crate::canvaskit::bindings::{CKPaint, ck_color4f};
use crate::canvaskit::convert::{ck_blend_mode, ck_paint_style, ck_stroke_cap, ck_stroke_join};

/// 应用 `spec` 到 fill_paint 或 stroke_paint，返回应被使用的 paint 引用。
///
/// 策略：根据 spec.style 选目标 paint，把所有 spec 字段写到它上面。
/// fill_paint 与 stroke_paint 每帧复用（不创建新对象），属性会被下一次 apply_to
/// 覆盖，调用方不需要 reset。
pub fn apply_to<'a>(
    fill_paint: &'a CKPaint,
    stroke_paint: &'a CKPaint,
    spec: &PaintSpec,
) -> &'a CKPaint {
    let target = match spec.style {
        PaintStyle::Fill => fill_paint,
        PaintStyle::Stroke => stroke_paint,
    };

    target.set_anti_alias(spec.anti_alias);
    target.set_style(&ck_paint_style(spec.style));
    target.set_blend_mode(&ck_blend_mode(spec.blend_mode));

    // Fill 处理：Solid 走 setColor，Shader 在 Plan C/D 接入后调 set_shader。
    match &spec.fill {
        FillSpec::Solid([r, g, b, a]) => {
            target.set_color(&ck_color4f(*r, *g, *b, *a));
            // 重置 shader（防止上一次设过 shader 留在 paint 上）
            target.set_shader(&wasm_bindgen::JsValue::NULL);
        }
        FillSpec::Shader(shader_spec) => {
            if let Some(shader_handle) = crate::canvaskit::bindings::build_ck_shader(shader_spec) {
                target.set_shader(shader_handle.as_js());
            } else {
                target.set_color(&ck_color4f(0.0, 0.0, 0.0, 0.0));
                target.set_shader(&wasm_bindgen::JsValue::NULL);
            }
        }
    }

    // Stroke 处理
    if let Some(stroke) = spec.stroke.as_ref() {
        target.set_stroke_width(stroke.width);
        target.set_stroke_cap(&ck_stroke_cap(stroke.cap));
        target.set_stroke_join(&ck_stroke_join(stroke.join));
        target.set_stroke_miter(stroke.miter_limit);
    }

    // ImageFilter
    if let Some(ref if_spec) = spec.image_filter {
        if let Some(handle) = crate::canvaskit::bindings::build_ck_image_filter(if_spec) {
            target.set_image_filter(handle.as_js());
        } else {
            target.set_image_filter(&wasm_bindgen::JsValue::NULL);
        }
    } else {
        target.set_image_filter(&wasm_bindgen::JsValue::NULL);
    }

    // ColorFilter
    if let Some(ref cf_spec) = spec.color_filter {
        if let Some(handle) = crate::canvaskit::bindings::build_ck_color_filter(cf_spec) {
            target.set_color_filter(handle.as_js());
        } else {
            target.set_color_filter(&wasm_bindgen::JsValue::NULL);
        }
    } else {
        target.set_color_filter(&wasm_bindgen::JsValue::NULL);
    }

    // MaskFilter
    if let Some(ref mf_spec) = spec.mask_filter {
        if let Some(handle) = crate::canvaskit::bindings::build_ck_mask_filter(mf_spec) {
            target.set_mask_filter(handle.as_js());
        } else {
            target.set_mask_filter(&wasm_bindgen::JsValue::NULL);
        }
    } else {
        target.set_mask_filter(&wasm_bindgen::JsValue::NULL);
    }

    // PathEffect
    if let Some(ref pe_spec) = spec.path_effect {
        if let Some(handle) = crate::canvaskit::bindings::build_ck_path_effect(pe_spec) {
            target.set_path_effect(handle.as_js());
        } else {
            target.set_path_effect(&wasm_bindgen::JsValue::NULL);
        }
    } else {
        target.set_path_effect(&wasm_bindgen::JsValue::NULL);
    }

    target
}
