//! ZoomBlur 的配方——逐行翻译真库 `packages/core/src/shaders/ZoomBlur/index.ts`。
//!
//! 双 pass：`rtt_rtt_0` 把子层画进 `rtt_0`，`final_1` 从中心做 32 tap 的等比缩放采样
//! （权重硬编码，就是「这个 look」）。`dispersion/falloff/focus/bias/highlights/spiral/jitter`
//! 在快照里都没进入 WGSL——真库此版本只发射 intensity/center。

use crate::filter::{child_group, f, fs, root_group, sys_group, CHILD_KIT, CHILD_RTT};
use crate::layout::{Field, Group, Uniform, Val};
use crate::{Baked, BakedPass, Props, assemble};

/// 32 tap 高斯权重（与 tap 序号一一对应，字面量逐字抄自快照）。
const TAP_WEIGHTS: [f32; 32] = [
    0.01833924571930286,
    0.02021849882901347,
    0.022145824255032632,
    0.024099625573710187,
    0.026055790002929013,
    0.027988118222713798,
    0.029868860752810644,
    0.03166934789372045,
    0.033360694846059885,
    0.03491455881584358,
    0.03630392101960902,
    0.03750386381383466,
    0.03849231191611351,
    0.039250707006906665,
    0.03976458694921195,
    0.04002404438318769,
    0.04002404438318769,
    0.03976458694921195,
    0.039250707006906665,
    0.03849231191611351,
    0.03750386381383466,
    0.03630392101960902,
    0.03491455881584358,
    0.033360694846059885,
    0.03166934789372045,
    0.029868860752810648,
    0.027988118222713798,
    0.026055790002929013,
    0.024099625573710187,
    0.022145824255032632,
    0.02021849882901347,
    0.01833924571930286,
];

pub fn bake(props: &Props, width: f32, height: f32, time: f32) -> Baked {
    let uniform = Uniform::new(vec![
        root_group(),
        Group::new(
            "n_n0",
            vec![
                Field::f32("intensity", f(props, "intensity")),
                // 原样传 props 的 center；Y 翻转在 `zoomBlurTapCoord` 里做（快照同款）。
                Field::vec2("center", fs(props, "center")[0], fs(props, "center")[1]),
                Field::f32("dispersion", f(props, "dispersion")),
                Field::f32("falloff", f(props, "falloff")),
                Field::f32("focus", f(props, "focus")),
                Field::f32("bias", f(props, "bias")),
                Field::f32("highlights", f(props, "highlights")),
                Field::f32("spiral", f(props, "spiral")),
                Field::f32("jitter", f(props, "jitter")),
                Field::f32("_opacity", 1.0),
            ],
        ),
        child_group(),
        sys_group(width, height, time),
    ]);

    let mut child_kit = CHILD_KIT.to_vec();
    child_kit.push("overComposite");
    let child_wgsl = assemble(&uniform, &child_kit, "", CHILD_RTT);

    let final_wgsl = assemble(
        &uniform,
        &["zoomBlurTapCoord", "unpremultiplyAlpha", "linearToSrgb", "tone_linear"],
        "",
        &final_pass(),
    );

    Baked {
        passes: vec![
            BakedPass { entry: "rtt_rtt_0".into(), wgsl: child_wgsl, reads: vec![] },
            BakedPass { entry: "final_1".into(), wgsl: final_wgsl, reads: vec!["rtt_0".into()] },
        ],
        uniform_bytes: uniform.bytes().expect("ZoomBlur uniform 字节"),
        final_reads: vec!["rtt_0".into()],
        consumes_input: true,
    }
}

fn final_pass() -> String {
    // 快照里是 32 项右结合的加法链 `((t0+w0) + t1*w1) + ...`，左折叠即同序。
    let mut acc = String::new();
    for (i, w) in TAP_WEIGHTS.iter().enumerate() {
        let tap = format!(
            "(textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, {i}.0)) * {w})"
        );
        acc = if acc.is_empty() { tap } else { format!("(({acc}) + {tap})") };
    }
    format!(
        r#"@group(1) @binding(0) var rtt_0: texture_2d<f32>;
@group(2) @binding(0) var linearClamp: sampler;

struct final_Input {{
  @location(0) uv: vec2f,
}}

@fragment fn final_1(in: final_Input) -> @location(0) vec4f {{
  var uv = in.uv;
  var composed_0 = unpremultiplyAlpha({acc});
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}}
"#
    )
}

pub fn default_props() -> Props {
    let mut p = Props::new();
    p.insert("intensity".into(), Val::F(30.0));
    p.insert("center".into(), Val::Fs(vec![0.5, 0.5]));
    for k in ["dispersion", "falloff", "focus", "bias", "highlights", "spiral", "jitter"] {
        p.insert(k.into(), Val::F(0.0));
    }
    p
}
