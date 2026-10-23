//! Aurora 的配方——逐行翻译真库 `packages/core/src/shaders/Aurora/index.ts`。
//!
//! 真库用 TS 名词组合（`wavyLine` / `rayBands` / `softBand` / `layered`），
//! TypeGPU 再把它们编成 WGSL。这里做同一件事，只是手写成「拼字符串」：
//!
//! - `curtain(layer, timeOffset)` 每层一段（真库注释：「curtainCount is
//!   compile-time, so disabled layers aren't emitted」——所以层数是**生成期**
//!   参数，不是 uniform）。
//! - 层权重 1 的那一层不乘（真库 `weight === 1 ? alpha : mul(alpha, weight)`）。
//! - 字符串保留两位小数的字面量按真库原文抄（如 `31.7` 在 WGSL 里是
//!   `31.700000762939453`，那是 f32 折叠的结果；直接写 `31.7` 由 WGSL 编译
//!   器折成同一个 f32，语义一致）。

use crate::layout::{Field, Group, Uniform, Val};
use crate::colors::p3_linear_rgba;
use crate::{Baked, BakedPass, Props, assemble};

/// 真库 `ARC_WAVES`：路径波浪栈（每层带它自己的历史 hash-phase 盐）。
const ARC_WAVES: &[(f32, f32, f32, f32, f32)] = &[
    // (freq, speed, amount, salt.k, salt.big)
    (0.7, 0.4, 0.15, 12.9, 4758.5),
    (1.6, 0.65, 0.08, 78.2, 2847.1),
    (2.9, 0.3, 0.04, 41.6, 1593.7),
];
const RAY_SALT: (f32, f32) = (53.7, 3847.2);
/// 真库 `LAYERS`：时间偏移 + 淡出权重。
const LAYERS: &[(f32, f32)] = &[(0.0, 1.0), (3.1, 0.65), (6.7, 0.45), (9.4, 0.3)];

fn get<'a>(p: &'a Props, k: &str) -> &'a Val {
    p.get(k).unwrap_or_else(|| panic!("Aurora props 缺 {k}"))
}

fn f(p: &Props, k: &str) -> f32 {
    get(p, k).as_f32()
}

fn fs(p: &Props, k: &str) -> Vec<f32> {
    get(p, k).as_fs()
}

/// 烘焙 Aurora。
///
/// `size` 进 `_sys.viewportSize`（片元里 aspect 由它算）。
/// `time` 是本层挂的**帧时钟**（`f(t)`）；`_animTime` 由它乘 `speed` 得到，
/// 与真库 `animatedTime: { speed: 'speed' }` 的累加语义等价。
pub fn bake(props: &Props, width: f32, height: f32, time: f32) -> Baked {
    let curtain_count = f(props, "curtainCount").round().clamp(1.0, 4.0) as usize;
    let anim_time = time * f(props, "speed");

    let uniform = Uniform::new(vec![
        Group::new("n_root", vec![Field::f32("_opacity", 1.0)]),
        Group::new(
            "n_n0",
            vec![
                Field::vec4s("colorA", fs(props, "colorA")),
                Field::vec4s("colorB", fs(props, "colorB")),
                Field::vec4s("colorC", fs(props, "colorC")),
                Field::f32("colorSpace", f(props, "colorSpace")),
                Field::f32("balance", f(props, "balance")),
                Field::f32("intensity", f(props, "intensity")),
                Field::f32("curtainCount", f(props, "curtainCount")),
                Field::f32("speed", f(props, "speed")),
                Field::f32("waviness", f(props, "waviness")),
                Field::f32("rayDensity", f(props, "rayDensity")),
                Field::f32("height", f(props, "height")),
                Field::vec2("center", fs(props, "center")[0], fs(props, "center")[1]),
                Field::f32("seed", f(props, "seed")),
                Field::f32("_opacity", 1.0),
                Field::f32("_animTime", anim_time),
            ],
        ),
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
        ),
    ]);

    let body = fragment(&uniform, curtain_count);
    let wgsl = assemble(
        &uniform,
        &["mxNoiseFloat3", "mixColors", "linearToSrgb", "tone_linear"],
        "",
        &body,
    );

    Baked {
        passes: vec![BakedPass { entry: "final_1".into(), wgsl, reads: vec![] }],
        uniform_bytes: uniform.bytes().expect("Aurora uniform 字节"),
        final_reads: vec![],
        consumes_input: false,
    }
}

/// 单层 curtain 的 WGSL 片段（真库 `curtain(layerIdx, timeOffset)`）。
fn curtain_wgsl(layer: usize, time_offset: f32) -> String {
    let t_slow = format!("tSlow_{layer}");
    let l_seed = format!("lSeed_{layer}");
    let wx = format!("wx_{layer}");
    let noise = format!("curtainNoise_{layer}");
    let dist = format!("distFromPath_{layer}");
    let rays = format!("curtainRays_{layer}");
    let top = format!("topExtent_{layer}");
    let color_t = format!("colorT_{layer}");
    let alpha = format!("curtainAlpha_{layer}");

    // 真库 `Math.fround(layerIdx * Math.fround(31.7))`：f32 折叠。
    let seed_off = (layer as f32) * 31.7f32;

    // ARC_WAVES 展开成 wavyLine 的和式。
    let mut arc = String::new();
    for (i, (freq, speed, amount, salt_k, salt_big)) in ARC_WAVES.iter().enumerate() {
        if i > 0 {
            arc.push_str(" + ");
        }
        // phase = fract(sin(lSeed * salt.k) * salt.big) * TAU
        arc.push_str(&format!(
            "(sin(((({wx} * {freq}) + ({t_slow} * {speed})) + (fract((sin(({l_seed} * {salt_k})) * {salt_big})) * 6.283185307179586))) * {amount})"
        ));
    }

    // rayBands：frequency = mix(6,20,rayAmt)*0.15；phase = hashPhase(lSeed,RAY_SALT)；
    // sharpness 1.5 → pow(sin*0.5+0.5, 1.5)
    let rays_expr = format!(
        "pow(((sin((((({wx} + ({noise} * 2.5)) * (mix(6.0, 20.0, (uniforms.n_n0.rayDensity * 0.01)) * 0.15)) + ({t_slow} * 0.1)) + (fract((sin(({l_seed} * {rk})) * {rb})) * 6.283185307179586))) * 0.5) + 0.5), 1.5)",
        rk = RAY_SALT.0,
        rb = RAY_SALT.1
    );

    let mut s = String::new();
    s.push_str(&format!("  let {wx} = (((uv.x - uniforms.n_n0.center.x) * aspect) * 2.0);\n"));
    s.push_str(&format!("  let {t_slow} = ((auroraTime * 0.06) + {time_offset});\n"));
    s.push_str(&format!("  let {l_seed} = (uniforms.n_n0.seed + {seed_off});\n"));
    s.push_str(&format!(
        "  let {noise} = mxNoiseFloat3(vec3f(({wx} * 0.5), ({t_slow} * 0.25), ({l_seed} * 0.1)));\n"
    ));
    // pathY = (1 - center.y) + arc*wav + noiseWarp*0.14*wav
    s.push_str(&format!(
        "  let {dist} = (uv.y - (((1.0 - uniforms.n_n0.center.y) + (({arc}) * (uniforms.n_n0.waviness * 0.01))) + (({noise} * 0.14) * (uniforms.n_n0.waviness * 0.01))));\n"
    ));
    s.push_str(&format!("  let {rays} = {rays_expr};\n"));
    // localHeight = auroraHeight * mix(0.4,1,rays); topExtent = localHeight*(rays*0.4+0.6)
    // 真库把 auroraHeight 直接合进第二步（uniforms.height*0.01）。
    s.push_str(&format!(
        "  let {top} = (((uniforms.n_n0.height * 0.01) * mix(0.4, 1.0, {rays})) * (({rays} * 0.4) + 0.6));\n"
    ));
    s.push_str(&format!(
        "  let {color_t} = clamp(({dist} / ({top} + 0.001)), 0.0, 1.0);\n"
    ));
    // envelope(softBand below -0.04..0.015, above ..top) × rays × sectionBright × coreBright
    s.push_str(&format!(
        "  let {alpha} = ((((smoothstep(-0.04, 0.015, {dist}) * (1.0 - smoothstep(0.0, {top}, {dist}))) * {rays}) * mix(0.4, 1.0, (({noise} * 0.5) + 0.5))) * ((exp(((({color_t} - 0.2) * ({color_t} - 0.2)) * -6.0)) * 0.3) + 1.0));\n"
    ));
    s
}

fn fragment(uniform: &Uniform, count: usize) -> String {
    let mut s = String::new();
    s.push_str("struct final_Input {\n  @location(0) uv: vec2f,\n}\n\n");
    s.push_str("@fragment fn final_1(in: final_Input) -> @location(0) vec4f {\n");
    s.push_str("  var uv = in.uv;\n");
    s.push_str("  let aspect = (uniforms._sys.viewportSize.x / max(uniforms._sys.viewportSize.y, 0.000001));\n");
    s.push_str("  let auroraTime = uniforms.n_n0._animTime;\n");

    // 逐层展开。
    let mut alphas = Vec::new();
    let mut colors = Vec::new();
    for (i, (time_offset, _weight)) in LAYERS.iter().take(count).enumerate() {
        s.push_str(&curtain_wgsl(i, *time_offset));
        alphas.push(format!("curtainAlpha_{i}"));
        colors.push(format!("colorT_{i}"));
    }

    // 层加权和：weight 1 不乘（与真库一致）。
    let mut total = String::new();
    let mut weighted = String::new();
    for (i, (_t, weight)) in LAYERS.iter().take(count).enumerate() {
        let a = &alphas[i];
        let c = &colors[i];
        let term_a = if (*weight - 1.0).abs() < f32::EPSILON {
            a.clone()
        } else {
            format!("({a} * {weight})")
        };
        if i > 0 {
            total.push_str(" + ");
            weighted.push_str(" + ");
        }
        total.push_str(&term_a);
        weighted.push_str(&format!("({c} * {term_a})"));
    }

    s.push_str(&format!("  let auroraAlpha = ({total});\n"));
    s.push_str(&format!(
        "  let auroraAvgT = pow((({weighted}) / (auroraAlpha + 0.001)), mix(2.5, 0.4, (uniforms.n_n0.balance * 0.01)));\n"
    ));
    s.push_str("  let auroraGlow = (auroraAlpha * (uniforms.n_n0.intensity * 0.018));\n");
    s.push_str(
        "  var composed = vec4f(clamp(((mixColors(mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, smoothstep(0.0, 0.2, auroraAvgT)), uniforms.n_n0.colorC, smoothstep(0.4, 0.85, auroraAvgT))).rgb * auroraGlow), vec3f(0.0), vec3f(1.0)), clamp((auroraGlow * 0.5), 0.0, 1.0));\n",
    );
    s.push_str("  return vec4f(linearToSrgb(tone_linear(composed.rgb)) * composed.a, composed.a);\n");
    s.push_str("}\n");
    let _ = uniform;
    s
}

/// Aurora 的 props 表（默认值抄真库 `props` 声明）。
pub fn default_props() -> Props {
    let mut p = Props::new();
    p.insert("colorA".into(), Val::Fs(p3_linear_rgba([0xa5, 0x33, 0xf8], 1.0)));
    p.insert("colorB".into(), Val::Fs(p3_linear_rgba([0x22, 0xee, 0x88], 1.0)));
    p.insert("colorC".into(), Val::Fs(p3_linear_rgba([0x16, 0x94, 0xe8], 1.0)));
    p.insert("colorSpace".into(), Val::F(0.0));
    p.insert("balance".into(), Val::F(50.0));
    p.insert("intensity".into(), Val::F(80.0));
    p.insert("curtainCount".into(), Val::F(4.0));
    p.insert("speed".into(), Val::F(5.0));
    p.insert("waviness".into(), Val::F(50.0));
    p.insert("rayDensity".into(), Val::F(20.0));
    p.insert("height".into(), Val::F(120.0));
    p.insert("center".into(), Val::Fs(vec![0.5, 1.0]));
    p.insert("seed".into(), Val::F(0.0));
    p
}
