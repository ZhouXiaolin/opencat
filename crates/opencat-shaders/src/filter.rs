//! 带子层的两个 pass 组件共用的小工具：LinearGradient 子层 + 双 pass 拼装。
//!
//! 快照里 `ZoomBlur` 与 `VHS` 的 `shader-0` 完全一样——它们都把唯一子层
//! （catalog 的 `LinearGradient` preset）画进 `rtt_0`，然后 `shader-1` 拿它当输入。
//! 子层不是本 crate 的组件，所以这里手写它的两张片元体，不引入 recipe 层。

use crate::colors::p3_linear_rgba;
use crate::layout::{Field, Group, Val};
use crate::Props;

/// `LinearGradient` 子层的 uniform 组（真库 `utilities/transformations.ts` 已做 transform：
/// 颜色进 P3-linear、`start`/`end` 做 Y 翻转）。
pub fn child_group() -> Group {
    // 默认子层 = 真库 catalog 的 `LinearGradient { colorA:#312e81, colorB:#f59e0b, angle:35 }`
    // （在 shader-diff 里所有 filter 组件都用这个 preset 喂自己的第一层）。
    Group::new(
        "n_n0c0",
        vec![
            Field::vec4s("colorA", p3_linear_rgba([0x31, 0x2e, 0x81], 1.0)),
            Field::vec4s("colorB", p3_linear_rgba([0xf5, 0x9e, 0x0b], 1.0)),
            Field::vec4a("colorsArray", vec![0.0; 32]),
            Field::vec4a("positionsArray", vec![0.0; 8]),
            Field::vec3s("convertedColorsArray", vec![0.0; 24]),
            Field::f32("stopCount", 0.0),
            Field::vec2("start", 0.0, 0.5),
            Field::vec2("end", 1.0, 0.5),
            Field::f32("angle", 35.0),
            Field::f32("edges", 0.0),
            Field::f32("colorSpace", 0.0),
            Field::f32("_opacity", 1.0),
        ],
    )
}

/// 子层的 `rtt_rtt_0` 片元体（逐字对应快照 `shader-0.wgsl`）。
pub const CHILD_RTT: &str = r#"struct rtt_rtt_0_Input {
  @location(0) uv: vec2f,
}

@fragment fn rtt_rtt_0(in: rtt_rtt_0_Input) -> @location(0) vec4f {
  var uv = in.uv;
  return blend_normal(vec4f(1.0, 1.0, 1.0, 0.0), mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, uv, uniforms._sys.viewportSize)))), uniforms.n_n0c0._opacity);
}
"#;

/// 子层 pass 需要哪些 kit 函数。
pub const CHILD_KIT: &[&'static str] = &[
    "blend_normal",
    "mixColors",
    "gradientClamp01",
    "gradientRawT",
    "unflipPosition",
    "geom_rotate2",
];

/// 子层 props 的默认值（供配方取用，避免各处重复字面量）。
pub fn child_props() -> Props {
    Props::new()
}

/// `_sys` 组——所有组件的公共尾巴。
pub fn sys_group(width: f32, height: f32, time: f32) -> Group {
    Group::new(
        "_sys",
        vec![
            Field::f32("time", time),
            Field::vec2("viewportSize", width, height),
            Field::vec2("logicalViewportSize", width, height),
            Field::f32("aspect", width / height.max(1e-6)),
            Field::vec2("pointer", 0.5, 0.5),
            Field::f32("pointerActive", 0.0),
        ],
    )
}

/// `n_root` 组——所有组件都有的根节点（只有 `_opacity`）。
pub fn root_group() -> Group {
    Group::new("n_root", vec![Field::f32("_opacity", 1.0)])
}

/// 取 props 里的浮点值。
pub fn f(p: &Props, k: &str) -> f32 {
    p.get(k)
        .unwrap_or_else(|| panic!("props 缺 {k}"))
        .as_f32()
}

/// 取 props 里的浮点数组。
pub fn fs(p: &Props, k: &str) -> Vec<f32> {
    p.get(k)
        .unwrap_or_else(|| panic!("props 缺 {k}"))
        .as_fs()
}

/// 取 props 里的 `Val`（写入 layout 用）。
pub fn val(p: &Props, k: &str) -> Val {
    p.get(k)
        .unwrap_or_else(|| panic!("props 缺 {k}"))
        .clone()
}
