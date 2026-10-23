//! 颜色空间换算——真库 `utilities/transformations.ts` + `gpu/kit/colorMixing.ts` 的 CPU 侧。
//!
//! 三件事：
//! 1. CSS 颜色 → **Display-P3 linear**（真库默认工作空间，`colorSpaceMode = 'p3-linear'`）。
//! 2. P3 linear → 混合空间（oklab 等），喂 `convertedColorsArray`。
//! 3. 位置 Y 翻转（真库 `transformPosition` 末尾 `1 - y`）。
//!
//! 系数逐字抄真库，不做「更准」的替换——两侧必须给出同一个 f32。

/// sRGB 十六进制 → Display-P3 linear RGB（alpha 固定 1）。
///
/// 链：sRGB(γ→linear) → XYZ(D65) → P3(linear)。与 colorjs.io 的
/// `new Color(hex).to('p3-linear')` 一致到 f32 精度（已用手算核对 `#a533f8`）。
pub fn p3_linear(rgb: [u8; 3]) -> [f32; 3] {
    let lin = |v: u8| -> f64 {
        let v = v as f64 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let src = [lin(rgb[0]), lin(rgb[1]), lin(rgb[2])];
    const SRGB_TO_XYZ: [[f64; 3]; 3] = [
        [0.41239079926595934, 0.357584339383878, 0.1804807884018343],
        [0.21263900587151027, 0.715168678767756, 0.07219231536073371],
        [0.01933081871559182, 0.11919477979462598, 0.9505321522496607],
    ];
    const XYZ_TO_P3: [[f64; 3]; 3] = [
        [2.493496911941425, -0.9313836179191239, -0.40271078445071684],
        [-0.8294889695615747, 1.7626640603183463, 0.023624685841943577],
        [0.03584583024378447, -0.07617238926804182, 0.9568845240076872],
    ];
    let mul = |m: &[[f64; 3]; 3], v: [f64; 3]| -> [f64; 3] {
        [
            m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
            m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
            m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
        ]
    };
    let p3 = mul(&XYZ_TO_P3, mul(&SRGB_TO_XYZ, src));
    [p3[0] as f32, p3[1] as f32, p3[2] as f32]
}

/// P3 linear → 混合工作空间（真库 `convertP3ToMixSpaceCPU`，`colorSpaceMode` 编号见原文件）。
///
/// 0 = 线性（P3 本身就是工作空间）、1 = oklch、2 = oklab、3 = hsl、4 = hsv、5 = lch。
pub fn p3_to_mix_space(rgb: [f32; 3], mode: f32) -> [f32; 3] {
    // 真库先把 P3 当成 sRGB-linear 转回 sRGB-linear（矩阵乘积近似恒等，但它是权威口径）。
    let sr = rgb[0] * 1.2249401 - rgb[1] * 0.2249404;
    let sg = rgb[0] * -0.0420569 + rgb[1] * 1.0420571;
    let sb = rgb[0] * -0.0196376 - rgb[1] * 0.0786361 + rgb[2] * 1.0982735;
    match mode as i32 {
        2 => rgb_to_oklab(sr, sg, sb),
        // 其它模式本 crate 尚未用到；线性直通是安全默认（mode 0 的真库行为）。
        _ => rgb,
    }
}

fn rgb_to_oklab(r: f32, g: f32, b: f32) -> [f32; 3] {
    let l = (r * 0.4122214708 + g * 0.5363325363 + b * 0.0514459929).cbrt();
    let m = (r * 0.2119034982 + g * 0.6806995451 + b * 0.1073969566).cbrt();
    let s = (r * 0.0883024619 + g * 0.2817188376 + b * 0.6299787005).cbrt();
    [
        l * 0.2104542553 + m * 0.7936177850 - s * 0.0040720468,
        l * 1.9779984951 - m * 2.4285922050 + s * 0.4505937099,
        l * 0.0259040371 + m * 0.7827717662 - s * 0.8086757660,
    ]
}

/// CSS 颜色 → P3-linear rgba（`convertedColorsArray`/`colorsArray` 的统一口径）。
pub fn p3_linear_rgba(rgb: [u8; 3], alpha: f32) -> Vec<f32> {
    let c = p3_linear(rgb);
    vec![c[0], c[1], c[2], alpha]
}

/// 位置 Y 翻转（真库 `transformPosition`：`finalY = 1.0 - y`）。
pub fn flip_y(y: f32) -> f32 {
    1.0 - y
}

/// 散点星座的一帧位置（真库 `utilities/scatterAnchors.ts` 的 `spiralScatterCpu`）。
///
/// 只在 count/seed/drift/aspect/anim_time 变化时重算——像素不变的那一半，真库把它
/// 从 `onBeforeRender` 拷进 `meshAnchors` extraField。这里同样在烘焙期算好。
/// `f32` 折叠逐位对齐真库的 `Math.fround`。
pub fn spiral_scatter(fi: f32, count: f32, seed: f32, drift: f32, aspect: f32, anim_time: f32) -> [f32; 2] {
    const GOLDEN: f32 = 2.3999632;
    const TAU: f32 = 6.2831855;
    let theta = (fi * GOLDEN) + (hash11((seed * 0.7311) + 1.618) * TAU);
    let r = (((fi + 0.5) / count) as f32).sqrt();
    let jh = hash22((fi * 3.77) + 0.917, (seed * 1.093) + 2.236);
    let jx = (jh[0] - 0.5) * 0.26;
    let jy = (jh[1] - 0.5) * 0.26;
    let ph = hash22((fi * 7.13) + 1.618, (seed * 0.531) + 3.71);
    let phx = ph[0] * TAU;
    let phy = ph[1] * TAU;
    let w1 = 0.5 + (hash11(((fi * 5.417) + (seed * 0.291)) + 0.917) * 0.7);
    let tw = (anim_time * 0.11) * w1;
    let dx = (tw + phx).sin();
    let dy = ((tw * 1.37) + phy).cos();
    let nx = (((((theta.cos() * r) * 0.62) + jx) + ((dx * drift) * 0.16)) as f32) as f32;
    let ny = (((((theta.sin() * r) * 0.62) + jy) + ((dy * drift) * 0.16)) as f32) as f32;
    [((0.5 + nx) * aspect), (0.5 + ny)]
}

/// 8 个 anchor 打包成 `array<vec4f, 4>`（偶数 index → `.xy`，奇数 → `.zw`）。
pub fn pack_scatter_anchors(count: f32, seed: f32, drift: f32, aspect: f32, anim_time: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; 16];
    for i in 0..8 {
        let [x, y] = spiral_scatter(i as f32, count, seed, drift, aspect, anim_time);
        out[i * 2] = x;
        out[i * 2 + 1] = y;
    }
    out
}

/// 真库 `noise.ts` `hash11`：f32 → [0,1]。u32 乘法按 2^32 回绕。
fn hash11(p: f32) -> f32 {
    let u = (p * 3141592653.0f32).to_bits();
    let h = u.wrapping_mul(u).wrapping_mul(3141592653u32);
    // f32(4294967295) 在 f32 里等于 2^32。
    (h as f32) / 4294967296.0
}

/// 真库 `noise.ts` `hash22`：vec2f → [0,1]²。
fn hash22(x: f32, y: f32) -> [f32; 2] {
    let ux = (x * 141421356.0f32).to_bits();
    let uy = (y * 2718281828.0f32).to_bits();
    let h = ux ^ uy;
    [
        (h.wrapping_mul(3141592653u32) as f32) / 4294967296.0,
        (h.wrapping_mul(1618033988u32) as f32) / 4294967296.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 用手算过的 `#a533f8` 钉住 P3 矩阵（Aurora 的 colorA）。
    #[test]
    fn p3_linear_matches_colorjs() {
        let c = p3_linear([0xa5, 0x33, 0xf8]);
        assert!((c[0] - 0.3153386).abs() < 1e-5, "r = {}", c[0]);
        assert!((c[1] - 0.0444956).abs() < 1e-5, "g = {}", c[1]);
        assert!((c[2] - 0.8635163).abs() < 1e-5, "b = {}", c[2]);
    }
}
