//! MeshGradient 的配方——逐行翻译真库 `packages/core/src/shaders/MeshGradient/index.ts`。
//!
//! 与 Aurora 的差别：这个组件读**子层**的 `meshAnchors`（像素不变的那一半在 CPU 上
//! 算好，见 [`crate::colors::pack_scatter_anchors`]），并且用 `gradientStopsInSpace`
//! 的运行时多停靠点循环，而不是三层颜色梯子。

use crate::colors::{p3_linear_rgba, p3_to_mix_space, pack_scatter_anchors};
use crate::layout::{Field, Group, Uniform, Val};
use crate::{Baked, BakedPass, Props, assemble};

/// 真库常量（这些数字就是「这个 look」）。
const DOMAIN_SCALE: f32 = 1.6;
const WARP_AMOUNT: f32 = 0.48;
const WARP_RATE: f32 = 0.125;
const SWIRL_MAX: f32 = 3.0;
const VAR_FREQ: f32 = 1.9;
const VAR_STRENGTH: f32 = 1.4;
const VAR_RATE: f32 = 0.045;
const WRAP_CYCLES: f32 = 9.0;

/// 真库 `DEFAULT_STOPS`（暗→亮多色带，包裹的接缝要扫过这些色相）。
const DEFAULT_STOPS: &[([u8; 3], f32)] = &[
    ([0x1a, 0x05, 0x33], 0.0),
    ([0x6d, 0x2f, 0xd1], 0.26),
    ([0xe0, 0x4b, 0x9e], 0.52),
    ([0xff, 0x8c, 0x42], 0.76),
    ([0xff, 0xdf, 0x8e], 1.0),
];

fn get<'a>(p: &'a Props, k: &str) -> &'a Val {
    p.get(k).unwrap_or_else(|| panic!("MeshGradient props 缺 {k}"))
}

fn f(p: &Props, k: &str) -> f32 {
    get(p, k).as_f32()
}

fn fs(p: &Props, k: &str) -> Vec<f32> {
    get(p, k).as_fs()
}

pub fn bake(props: &Props, width: f32, height: f32, time: f32) -> Baked {
    let anim_time = time * f(props, "speed");
    let aspect = width / height.max(1e-6);
    // 真库 `scatteredAnchors` 在 onBeforeRender 里用当前帧的 count/seed/drift/aspect/time 重算。
    let anchors = pack_scatter_anchors(
        f(props, "count"),
        f(props, "seed"),
        f(props, "drift"),
        aspect,
        anim_time,
    );

    let uniform = Uniform::new(vec![
        Group::new("n_root", vec![Field::f32("_opacity", 1.0)]),
        Group::new(
            "n_n0",
            vec![
                Field::vec4s("colorA", fs(props, "colorA")),
                Field::vec4s("colorB", fs(props, "colorB")),
                Field::vec4a("colorsArray", fs(props, "colorsArray")),
                Field::vec4a("positionsArray", fs(props, "positionsArray")),
                Field::vec3s("convertedColorsArray", fs(props, "convertedColorsArray")),
                Field::f32("stopCount", f(props, "stopCount")),
                Field::f32("colorSpace", f(props, "colorSpace")),
                Field::f32("count", f(props, "count")),
                Field::f32("smoothness", f(props, "smoothness")),
                Field::f32("variation", f(props, "variation")),
                Field::f32("swirl", f(props, "swirl")),
                Field::f32("drift", f(props, "drift")),
                Field::f32("wrapping", f(props, "wrapping")),
                Field::f32("speed", f(props, "speed")),
                Field::f32("seed", f(props, "seed")),
                Field::f32("_opacity", 1.0),
                Field::f32("_animTime", anim_time),
                Field::vec4a("meshAnchors", anchors),
            ],
        ),
        Group::new(
            "_sys",
            vec![
                Field::f32("time", time),
                Field::vec2("viewportSize", width, height),
                Field::vec2("logicalViewportSize", width, height),
                Field::f32("aspect", aspect),
                Field::vec2("pointer", 0.5, 0.5),
                Field::f32("pointerActive", 0.0),
            ],
        ),
    ]);

    // `MixStep` 是 kit 函数返回的结构体，收割 fn 时被跳过了（它不是 fn），这里补上。
    const MIX_STEP: &str = "struct MixStep {\n  conv: vec3f,\n  alpha: f32,\n}\n\n";
    let wgsl = assemble(
        &uniform,
        &[
            "warpDomain",
            "scatterField",
            "rampWrap",
            "gradientStopsInSpace",
            "oklabToRgb",
            "sRGBToP3",
            "rgbDither",
            "linearToSrgb",
            "tone_linear",
        ],
        MIX_STEP,
        &fragment(f(props, "variation") != 0.0),
    );

    Baked {
        passes: vec![BakedPass { entry: "final_1".into(), wgsl, reads: vec![] }],
        uniform_bytes: uniform.bytes().expect("MeshGradient uniform 字节"),
        final_reads: vec![],
        consumes_input: false,
    }
}

/// 片元体。`variation_active` 对应真库 `recompile: crosses(0)`——variation 恰为 0 时
/// 那次 Perlin-3D 读取被编译掉（`exp(0) = 1`，结果相同）。
fn fragment(variation_active: bool) -> String {
    let softness = if variation_active {
        "clamp(((8.0 / (uniforms.n_n0.smoothness + 0.5)) * exp(((mxNoiseFloat3(vec3f((meshQ.x * 1.9), (meshQ.y * 1.9), ((uniforms.n_n0.seed * 3.17) + (meshTime * 0.045)))) * uniforms.n_n0.variation) * 1.4))), 0.6, 16.0)"
    } else {
        "clamp((8.0 / (uniforms.n_n0.smoothness + 0.5)), 0.6, 16.0)"
    };
    format!(
        r#"struct final_Input {{
  @location(0) uv: vec2f,
}}

@fragment fn final_1(in: final_Input) -> @location(0) vec4f {{
  var uv = in.uv;
  let aspect = (uniforms._sys.viewportSize.x / max(uniforms._sys.viewportSize.y, 0.000001));
  let meshTime = uniforms.n_n0._animTime;
  let meshCentred = vec2f(((uv.x * aspect) - (aspect * 0.5)), (uv.y - 0.5));
  let swirlAngle = ((-{SWIRL_MAX} * uniforms.n_n0.swirl) * smoothstep(0.0, 1.0, length(meshCentred)));
  let meshRot = vec2f(((meshCentred.x * cos(swirlAngle)) - (meshCentred.y * sin(swirlAngle))), ((meshCentred.x * sin(swirlAngle)) + (meshCentred.y * cos(swirlAngle))));
  let meshSwirled = vec2f((meshRot.x + (aspect * 0.5)), (meshRot.y + 0.5));
  let meshWarp = warpDomain(vec3f((meshSwirled.x * {DOMAIN_SCALE}), (meshSwirled.y * {DOMAIN_SCALE}), (uniforms.n_n0.seed * 7.31)), (meshTime * {WARP_RATE}), {WARP_AMOUNT});
  let meshQ = vec2f((meshWarp.x / {DOMAIN_SCALE}), (meshWarp.y / {DOMAIN_SCALE}));
  let softness = {softness};
  let field = scatterField(meshQ, uniforms.n_n0.count, uniforms.n_n0.seed, uniforms.n_n0.meshAnchors, softness);
  let wrapped = rampWrap(field, (1.0 + (uniforms.n_n0.wrapping * {WRAP_CYCLES})));
  let stopMix = gradientStopsInSpace(wrapped, uniforms.n_n0.colorsArray, uniforms.n_n0.positionsArray, uniforms.n_n0.convertedColorsArray, uniforms.n_n0.stopCount);
  var composed = rgbDither(vec4f(sRGBToP3(oklabToRgb(stopMix.conv)), stopMix.alpha), uv, uniforms._sys.viewportSize, uniforms.n_n0._animTime);
  return vec4f(linearToSrgb(tone_linear(composed.rgb)) * composed.a, composed.a);
}}
"#
    )
}

/// 真库 `props` 的默认值，已做 transform（颜色进 P3-linear、停靠点进工作空间）。
pub fn default_props() -> Props {
    let mut p = Props::new();
    p.insert("colorA".into(), Val::Fs(p3_linear_rgba([0x1a, 0x05, 0x33], 1.0)));
    p.insert("colorB".into(), Val::Fs(p3_linear_rgba([0xff, 0xdf, 0x8e], 1.0)));

    // colorsArray: array<vec4f,8>，前 5 个是默认停靠点，其余补 0。
    let mut colors = vec![0.0f32; 32];
    let mut positions = vec![0.0f32; 8];
    let color_space_mode = 2.0; // oklab
    let mut converted = vec![0.0f32; 24];
    for (i, (hexv, pos)) in DEFAULT_STOPS.iter().enumerate() {
        let rgba = p3_linear_rgba(*hexv, 1.0);
        colors[i * 4..i * 4 + 4].copy_from_slice(&rgba);
        // 停靠点位置不做 Y 翻转——`transformPosition` 只作用于位置类 prop（center/start）。
        positions[i] = *pos;
        let mix = p3_to_mix_space([rgba[0], rgba[1], rgba[2]], color_space_mode);
        converted[i * 3..i * 3 + 3].copy_from_slice(&mix);
    }
    p.insert("colorsArray".into(), Val::Fs(colors));
    p.insert("positionsArray".into(), Val::Fs(positions));
    p.insert("convertedColorsArray".into(), Val::Fs(converted));
    p.insert("stopCount".into(), Val::F(DEFAULT_STOPS.len() as f32));
    p.insert("colorSpace".into(), Val::F(color_space_mode));
    p.insert("count".into(), Val::F(5.0));
    p.insert("smoothness".into(), Val::F(2.0));
    p.insert("variation".into(), Val::F(0.35));
    p.insert("swirl".into(), Val::F(0.3));
    p.insert("drift".into(), Val::F(0.5));
    p.insert("wrapping".into(), Val::F(0.0));
    p.insert("speed".into(), Val::F(1.0));
    p.insert("seed".into(), Val::F(0.0));
    p
}
