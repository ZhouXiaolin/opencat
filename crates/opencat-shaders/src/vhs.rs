//! VHS 的配方——逐行翻译真库 `packages/core/src/shaders/VHS/index.ts`。
//!
//! 双 pass：`rtt_rtt_0` 画子层，`final_1` 做磁带抖动 + YIQ 色度分离 + AC 亮度拍。
//! 时间来自 `_sys.time`（VHS 没有 `animatedTime`，快照同款）。

use crate::filter::{child_group, f, root_group, sys_group, CHILD_KIT, CHILD_RTT};
use crate::layout::{Field, Group, Uniform, Val};
use crate::{Baked, BakedPass, Props, assemble};

pub fn bake(props: &Props, width: f32, height: f32, time: f32) -> Baked {
    let uniform = Uniform::new(vec![
        root_group(),
        Group::new(
            "n_n0",
            vec![
                Field::f32("wobble", f(props, "wobble")),
                Field::f32("scanlineNoise", f(props, "scanlineNoise")),
                Field::f32("smear", f(props, "smear")),
                Field::f32("speed", f(props, "speed")),
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
        &[
            "vhsSampleUVs",
            "vhsChromaTapUV",
            "yiqRecombine",
            "vhsAcBeat",
            "beatShade",
            "unpremultiplyAlpha",
            "linearToSrgb",
            "tone_linear",
        ],
        "",
        FINAL,
    );

    Baked {
        passes: vec![
            BakedPass { entry: "rtt_rtt_0".into(), wgsl: child_wgsl, reads: vec![] },
            BakedPass { entry: "final_1".into(), wgsl: final_wgsl, reads: vec!["rtt_0".into()] },
        ],
        uniform_bytes: uniform.bytes().expect("VHS uniform 字节"),
        final_reads: vec!["rtt_0".into()],
    }
}

/// 快照 `shader-1.wgsl` 的 `final_1` 逐字搬运（只改了缩进）。
const FINAL: &str = r#"@group(1) @binding(0) var rtt_0: texture_2d<f32>;
@group(2) @binding(0) var linearClamp: sampler;

struct final_Input {
  @location(0) uv: vec2f,
}

@fragment fn final_1(in: final_Input) -> @location(0) vec4f {
  var uv = in.uv;
  let tapeUVs_0 = vhsSampleUVs(uv, uniforms._sys.time, uniforms.n_n0.speed, uniforms.n_n0.wobble, uniforms.n_n0.scanlineNoise);
  var composed_1 = unpremultiplyAlpha(beatShade(vec4f(yiqRecombine(textureSample(rtt_0, linearClamp, tapeUVs_0.xy), textureSample(rtt_0, linearClamp, vhsChromaTapUV(tapeUVs_0.zw, (uniforms.n_n0.smear * 0.0075), 1.0)), textureSample(rtt_0, linearClamp, vhsChromaTapUV(tapeUVs_0.zw, (uniforms.n_n0.smear * 0.0075), 2.0)), textureSample(rtt_0, linearClamp, vhsChromaTapUV(tapeUVs_0.zw, (uniforms.n_n0.smear * 0.0075), 3.0)), textureSample(rtt_0, linearClamp, vhsChromaTapUV(tapeUVs_0.zw, (uniforms.n_n0.smear * 0.0075), 4.0)), textureSample(rtt_0, linearClamp, vhsChromaTapUV(tapeUVs_0.zw, (uniforms.n_n0.smear * 0.0075), 5.0))), (textureSample(rtt_0, linearClamp, tapeUVs_0.xy)).a), vhsAcBeat(uv, uniforms._sys.time, uniforms.n_n0.speed, uniforms.n_n0.wobble)));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}
"#;

pub fn default_props() -> Props {
    let mut p = Props::new();
    p.insert("wobble".into(), Val::F(1.0));
    p.insert("scanlineNoise".into(), Val::F(0.6));
    p.insert("smear".into(), Val::F(0.2));
    p.insert("speed".into(), Val::F(1.0));
    p
}
