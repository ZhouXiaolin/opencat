//! `PaintSpec` → `CKPaint` 转换。仅覆盖 Solid / Stroke / AA / BlendMode / PaintStyle。
//! Shader / ImageFilter / ColorFilter / MaskFilter / PathEffect 在 Plan C/D 填实。

#![cfg(target_arch = "wasm32")]

use opencat_core::canvas::paint::{FillSpec, PaintStyle, PaintSpec};

use crate::canvaskit::bindings::{ck_color4f, CKPaint};
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
        FillSpec::Shader(_shader_spec) => {
            // TODO(Plan C/D): build_ck_shader(shader_spec) 后 target.set_shader(&shader);
            // 暂时 fall back 到透明黑色，行为可视化为 shader 未生效，但不 panic。
            target.set_color(&ck_color4f(0.0, 0.0, 0.0, 0.0));
            target.set_shader(&wasm_bindgen::JsValue::NULL);
        }
    }

    // Stroke 处理
    if let Some(stroke) = spec.stroke.as_ref() {
        target.set_stroke_width(stroke.width);
        target.set_stroke_cap(&ck_stroke_cap(stroke.cap));
        target.set_stroke_join(&ck_stroke_join(stroke.join));
        target.set_stroke_miter(stroke.miter_limit);
    }

    // 子 spec（Plan C/D 在此接入），先 reset 防残留：
    target.set_image_filter(&wasm_bindgen::JsValue::NULL);
    target.set_color_filter(&wasm_bindgen::JsValue::NULL);
    target.set_mask_filter(&wasm_bindgen::JsValue::NULL);
    target.set_path_effect(&wasm_bindgen::JsValue::NULL);

    target
}
