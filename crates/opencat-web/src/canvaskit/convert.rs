//! Rust 枚举 ↔ CanvasKit JS 常量映射。
//! CanvasKit 把 BlendMode/PaintStyle 等暴露成 `CK.BlendMode.SrcOver` 形式的 JS 对象，
//! 不是数字。本文件统一通过 `Reflect::get` 拿到对应 JS 值供 `setBlendMode` 等使用。

use wasm_bindgen::JsValue;

use opencat_core::canvas::paint::{PaintStyle, StrokeCap, StrokeJoin};
use opencat_core::canvas::{BlendMode, ClipOp, FillType, PointMode};

use crate::canvaskit::module::ck;

/// 从 `CK.<group>.<variant>` 路径拿一个 JS 值。失败返回 `undefined`，调用者负责处理。
fn lookup(group: &str, variant: &str) -> JsValue {
    let g = match js_sys::Reflect::get(ck(), &JsValue::from_str(group)) {
        Ok(v) if !v.is_undefined() => v,
        _ => return JsValue::UNDEFINED,
    };
    js_sys::Reflect::get(&g, &JsValue::from_str(variant)).unwrap_or(JsValue::UNDEFINED)
}

pub fn ck_blend_mode(m: BlendMode) -> JsValue {
    let v = match m {
        BlendMode::Clear => "Clear",
        BlendMode::Src => "Src",
        BlendMode::Dst => "Dst",
        BlendMode::SrcOver => "SrcOver",
        BlendMode::DstOver => "DstOver",
        BlendMode::SrcIn => "SrcIn",
        BlendMode::DstIn => "DstIn",
        BlendMode::SrcOut => "SrcOut",
        BlendMode::DstOut => "DstOut",
        BlendMode::SrcATop => "SrcATop",
        BlendMode::DstATop => "DstATop",
        BlendMode::Xor => "Xor",
        BlendMode::Plus => "Plus",
        BlendMode::Modulate => "Modulate",
        BlendMode::Screen => "Screen",
        BlendMode::Overlay => "Overlay",
        BlendMode::Darken => "Darken",
        BlendMode::Lighten => "Lighten",
        BlendMode::ColorDodge => "ColorDodge",
        BlendMode::ColorBurn => "ColorBurn",
        BlendMode::HardLight => "HardLight",
        BlendMode::SoftLight => "SoftLight",
        BlendMode::Difference => "Difference",
        BlendMode::Exclusion => "Exclusion",
        BlendMode::Multiply => "Multiply",
        BlendMode::Hue => "Hue",
        BlendMode::Saturation => "Saturation",
        BlendMode::Color => "Color",
        BlendMode::Luminosity => "Luminosity",
    };
    lookup("BlendMode", v)
}

pub fn ck_paint_style(s: PaintStyle) -> JsValue {
    match s {
        PaintStyle::Fill => lookup("PaintStyle", "Fill"),
        PaintStyle::Stroke => lookup("PaintStyle", "Stroke"),
    }
}

pub fn ck_stroke_cap(c: StrokeCap) -> JsValue {
    match c {
        StrokeCap::Butt => lookup("StrokeCap", "Butt"),
        StrokeCap::Round => lookup("StrokeCap", "Round"),
        StrokeCap::Square => lookup("StrokeCap", "Square"),
    }
}

pub fn ck_stroke_join(j: StrokeJoin) -> JsValue {
    match j {
        StrokeJoin::Miter => lookup("StrokeJoin", "Miter"),
        StrokeJoin::Round => lookup("StrokeJoin", "Round"),
        StrokeJoin::Bevel => lookup("StrokeJoin", "Bevel"),
    }
}

pub fn ck_clip_op(op: ClipOp) -> JsValue {
    match op {
        ClipOp::Intersect => lookup("ClipOp", "Intersect"),
        ClipOp::Difference => lookup("ClipOp", "Difference"),
    }
}

pub fn ck_point_mode(m: PointMode) -> JsValue {
    match m {
        PointMode::Points => lookup("PointMode", "Points"),
        PointMode::Lines => lookup("PointMode", "Lines"),
        PointMode::Polygon => lookup("PointMode", "Polygon"),
    }
}

pub fn ck_fill_type(f: FillType) -> JsValue {
    match f {
        FillType::Winding => lookup("FillType", "Winding"),
        FillType::EvenOdd => lookup("FillType", "EvenOdd"),
    }
}

/// CanvasKit RRect 表示为 [l, t, r, b, rx_lt, ry_lt, rx_rt, ry_rt, rx_rb, ry_rb, rx_lb, ry_lb]
/// 共 12 个 f32 的 Float32Array。从 kurbo::RoundedRect 提取 4 角各自半径（kurbo
/// 的 radii 是单一 f64，不区分 x/y，故每个角的 rx = ry）。
pub fn ck_rrect_from_kurbo(rrect: &opencat_core::canvas::RRect) -> JsValue {
    let rect = rrect.rect();
    let radii = rrect.radii();
    let arr = js_sys::Float32Array::new_with_length(12);
    arr.set_index(0, rect.x0 as f32);
    arr.set_index(1, rect.y0 as f32);
    arr.set_index(2, rect.x1 as f32);
    arr.set_index(3, rect.y1 as f32);
    // top-left (index 4,5)
    arr.set_index(4, radii.top_left as f32);
    arr.set_index(5, radii.top_left as f32);
    // top-right (index 6,7)
    arr.set_index(6, radii.top_right as f32);
    arr.set_index(7, radii.top_right as f32);
    // bottom-right (index 8,9)
    arr.set_index(8, radii.bottom_right as f32);
    arr.set_index(9, radii.bottom_right as f32);
    // bottom-left (index 10,11)
    arr.set_index(10, radii.bottom_left as f32);
    arr.set_index(11, radii.bottom_left as f32);
    arr.into()
}
