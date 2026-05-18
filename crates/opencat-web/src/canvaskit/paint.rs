//! `PaintSpec` → `CKPaint` 转换。M1 仅占位；M2 时镜像 engine 的
//! `backend.rs::apply_spec` + `build_skia_*` 实现各 sub-spec 构造。

#![cfg(target_arch = "wasm32")]

use opencat_core::canvas::PaintSpec;

use crate::canvaskit::bindings::CKPaint;

/// 把 `spec` 应用到 `fill_paint` / `stroke_paint`，返回应被使用的 paint 引用。
///
/// M1: 占位实现，不读 spec、不改 paint。M2 时按 engine 端 apply_spec
/// （`crates/opencat-engine/src/backend.rs`）的等价语义填实。
pub fn apply_to<'a>(
    fill_paint: &'a CKPaint,
    _stroke_paint: &'a CKPaint,
    _spec: &PaintSpec,
) -> &'a CKPaint {
    fill_paint
}
