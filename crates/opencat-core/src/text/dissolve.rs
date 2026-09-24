//! 脚本引擎 dissolve 效果 op —— k3-promo 场景 H 的 scramble dissolve。
//!
//! 参考实现 `index.html drawDissolve`（HyperFrames k3-promo，1448-1523 行）与
//! `rebuildDissolveField`（1254-1314 行）。架构原则（handoff §16）：像素缓冲
//! 永不跨进 JS、逐像素循环永不落在 JS —— 文本排版由 JS 以 `surface.fillText`
//! 薄标记记录（真字体光栅化在 [`super::surface`]），mask 阈值 + 3-4 chamfer
//! 距离场 + 逐帧 scramble 噪声（h01 哈希 / bandP 概率表 / scanline banding /
//! dust / y=540 加性 glow 带）全部在本模块（Rust）内计算，产物直接走
//! `DrawOp::Image { Generated }` 管线绘制。
//!
//! JS 侧对应薄标记：`surface.buildDissolve(x, y, w, h)`（构建一次）与
//! `canvas.applyDissolve({ surface, t, ... })`（每帧）。算法是帧号的纯函数
//! （seek 安全，与参考一致）。
//!
//! 逐位语义锚定（JS Number = f64，运算顺序逐行对齐参考 JS）：
//! - `h01`：`Math.imul` = 32 位 wrapping 乘的低 32 位；`>>>` = 逻辑右移；
//! - 输出量化：`v >= 40` → `(v|0, 255)`；`v > 0.8` → `(40, (v*6.375)|0)`；
//!   其余像素**保持全零**（JS 的 ImageData 初值；v≤0.8 分支不写）。
//!   注意离线 oracle `tools/k3dissolve.py` 在 alpha=0 时会留下 RGB=40 ——
//!   alpha=0 不可见，合成等价，但本实现按 JS 语义输出全零。

use std::cell::RefCell;
use std::sync::Arc;

use anyhow::{Result, bail};
use hashbrown::HashMap;

use super::surface::{surface_dimensions, surface_get_rgba};

/// 一次构建、逐帧复用的 mask + 距离场（参考 `DIS.mask` / `DIS.dist`）。
pub struct DissolveField {
    /// 区域左上角在 surface 坐标系的位置（floor 后整数）。
    pub x0: u32,
    pub y0: u32,
    pub w: u32,
    pub h: u32,
    /// 源 alpha（0..255），长度 w*h。
    pub mask: Vec<u8>,
    /// 3-4 chamfer 距离，单位 px*3（参考 Int16Array，背景 3000）。
    pub dist: Vec<i16>,
}

thread_local! {
    static FIELDS: RefCell<HashMap<String, Arc<DissolveField>>> =
        RefCell::new(HashMap::new());
}

/// 从一张离屏 surface 的子矩形构建 mask + chamfer 距离场。
///
/// 语义对齐参考 `rebuildDissolveField` 尾段：`mask = alpha`、
/// `dist = mask > 120 ? 0 : 3000`、前向+反向 3-4 chamfer。同一 surface id
/// 重复 build 直接覆盖（重建幂等）。
pub fn dissolve_build(surface_id: &str, x: f64, y: f64, w: f64, h: f64) -> Result<()> {
    if w <= 0.0 || h <= 0.0 {
        bail!("surface `{surface_id}`: buildDissolve with empty rect");
    }
    let (sw, sh) = surface_dimensions(surface_id)?;
    // 与 surface_get_rgba 相同的向内裁剪，保证区域完全落在 surface 内
    let ix0 = (x.floor() as i64).max(0);
    let iy0 = (y.floor() as i64).max(0);
    let ix1 = ((x + w).ceil() as i64).min(sw as i64);
    let iy1 = ((y + h).ceil() as i64).min(sh as i64);
    if ix1 <= ix0 || iy1 <= iy0 {
        bail!("surface `{surface_id}`: buildDissolve rect out of bounds");
    }
    let rw = (ix1 - ix0) as usize;
    let rh = (iy1 - iy0) as usize;
    let rgba = surface_get_rgba(surface_id, x, y, w, h)?;
    let n = rw * rh;
    debug_assert_eq!(rgba.len(), n * 4);
    let mut mask = vec![0u8; n];
    let mut dist = vec![3000i16; n];
    for i in 0..n {
        let a = rgba[i * 4 + 3];
        mask[i] = a;
        if a > 120 {
            dist[i] = 0;
        }
    }
    chamfer_3_4(&mut dist, rw, rh);
    let field = Arc::new(DissolveField {
        x0: ix0 as u32,
        y0: iy0 as u32,
        w: rw as u32,
        h: rh as u32,
        mask,
        dist,
    });
    FIELDS.with(|f| {
        f.borrow_mut().insert(surface_id.to_string(), field);
    });
    Ok(())
}

/// 删除一张 surface 的距离场（宿主清理用；不调用也无正确性影响）。
pub fn dissolve_forget(surface_id: &str) {
    FIELDS.with(|f| {
        f.borrow_mut().remove(surface_id);
    });
}

/// 3-4 chamfer 距离变换（参考 index.html:1278-1313 的两遍扫描，逐像素顺序
/// 一致；单位 px*3：正交 3、对角 4）。
fn chamfer_3_4(dist: &mut [i16], w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut d = dist[i];
            if x > 0 && dist[i - 1] + 3 < d {
                d = dist[i - 1] + 3;
            }
            if y > 0 {
                if dist[i - w] + 3 < d {
                    d = dist[i - w] + 3;
                }
                if x > 0 && dist[i - w - 1] + 4 < d {
                    d = dist[i - w - 1] + 4;
                }
                if x < w - 1 && dist[i - w + 1] + 4 < d {
                    d = dist[i - w + 1] + 4;
                }
            }
            dist[i] = d;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut d = dist[i];
            if x < w - 1 && dist[i + 1] + 3 < d {
                d = dist[i + 1] + 3;
            }
            if y < h - 1 {
                if dist[i + w] + 3 < d {
                    d = dist[i + w] + 3;
                }
                if x < w - 1 && dist[i + w + 1] + 4 < d {
                    d = dist[i + w + 1] + 4;
                }
                if x > 0 && dist[i + w - 1] + 4 < d {
                    d = dist[i + w - 1] + 4;
                }
            }
            dist[i] = d;
        }
    }
}

/// 确定性像素哈希（参考 index.html:1316-1321 `h01`）。返回 [0,1)。
fn h01(a: u32, b: u32, c: u32) -> f64 {
    let mut h = (a.wrapping_add(374_761)).wrapping_mul(0x9E37_79B1)
        ^ (b.wrapping_add(668_265)).wrapping_mul(0x85EB_CA6B)
        ^ (c.wrapping_add(951_274)).wrapping_mul(0xC2B2_AE35);
    h = (h ^ (h >> 16)).wrapping_mul(0x27D4_EB2F);
    (h ^ (h >> 15)) as f64 / 4294967296.0
}

/// 距离带点亮概率表（参考 index.html:1323-1331）：帧 → [d<1,<4,<8,<16,<24,bg]。
const DIS_TAB: [(f64, [f64; 6]); 7] = [
    (412.0, [0.97, 0.88, 0.74, 0.34, 0.06, 0.010]),
    (414.0, [0.97, 0.88, 0.84, 0.56, 0.19, 0.033]),
    (425.0, [0.94, 0.83, 0.80, 0.54, 0.21, 0.033]),
    (435.0, [0.87, 0.77, 0.74, 0.51, 0.23, 0.036]),
    (441.0, [0.83, 0.735, 0.70, 0.49, 0.24, 0.036]),
    (442.0, [0.76, 0.66, 0.61, 0.41, 0.19, 0.036]),
    (443.0, [0.60, 0.47, 0.37, 0.23, 0.10, 0.030]),
];

fn band_p(f: f64) -> [f64; 6] {
    if f <= DIS_TAB[0].0 {
        return DIS_TAB[0].1;
    }
    for i in 0..DIS_TAB.len() - 1 {
        if f <= DIS_TAB[i + 1].0 {
            let (fa, va) = DIS_TAB[i];
            let (fb, vb) = DIS_TAB[i + 1];
            let t = (f - fa) / (fb - fa);
            let mut out = [0.0f64; 6];
            for j in 0..6 {
                out[j] = va[j] + (vb[j] - va[j]) * t;
            }
            return out;
        }
    }
    DIS_TAB[DIS_TAB.len() - 1].1
}

/// glow 带包络（参考 index.html:1344-1349）：f416 出现、f420-436 全开、f440 消亡。
fn streak_env(f: f64) -> f64 {
    if f < 416.0 || f > 439.0 {
        return 0.0;
    }
    if f < 420.0 {
        return 0.42 + 0.145 * (f - 416.0);
    }
    if f <= 436.0 {
        return 1.0;
    }
    if f == 437.0 {
        return 0.58;
    }
    if f == 438.0 {
        return 0.33;
    }
    0.125
}

/// 一次渲染的产物：直接可绘制的 RGBA + 绘制位置。
pub struct RenderedDissolve {
    pub frame: u32,
    /// putImageData 的画布坐标（参考 `cvx.putImageData(img, DIS.X0, DIS.Y0-200)`）。
    pub dx: f64,
    pub dy: f64,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

/// 帧窗口语义（参考 `drawDissolve` 入口）：`t < 13.699 || t > 15.2` 或
/// `f > 455` 时不绘制。参数化以保持 op 通用，XML 传参考原值。
#[allow(clippy::too_many_arguments)]
pub fn dissolve_render(
    surface_id: &str,
    t: f64,
    fps: f64,
    win_start: f64,
    win_end: f64,
    max_frame: u32,
    dx: f64,
    dy: f64,
) -> Result<Option<RenderedDissolve>> {
    if t < win_start || t > win_end {
        return Ok(None);
    }
    let f = (t * fps + 1e-4).floor();
    if !(0.0..=max_frame as f64).contains(&f) {
        return Ok(None);
    }
    let field = FIELDS.with(|f| f.borrow().get(surface_id).cloned());
    let Some(field) = field else {
        bail!("dissolve `{surface_id}`: field not built (surface.buildDissolve first)");
    };
    let rgba = render_frame_pixels(&field, f as u32);
    Ok(Some(RenderedDissolve {
        frame: f as u32,
        dx,
        dy,
        width: field.w,
        height: field.h,
        rgba: rgba.into(),
    }))
}

/// 参考帧循环体（index.html:1456-1520）的逐位转写。`field` 的坐标语义：
/// 屏幕 px = x0 + 局部 x（scanline 相位与 glow 带都按屏幕坐标）。
fn render_frame_pixels(field: &DissolveField, f: u32) -> Vec<u8> {
    let w = field.w as usize;
    let h = field.h as usize;
    let p_b = band_p(f as f64);
    let ep = f / 7;
    let fr = (f % 7) as f64 / 7.0;
    let scramble = f >= 412;
    let dust = f >= 444;
    let dust_base = if dust {
        if f == 444 {
            0.40
        } else {
            0.32 * (455i64.saturating_sub(f as i64)).max(0) as f64 / 10.0
        }
    } else {
        0.0
    };
    let s_env = streak_env(f as f64);
    // streak x-extent retracts ~60px/side while dying（index.html:1464-1465）
    let (sx_l, sx_r) = if f >= 437 {
        (
            240.0 + (f - 436) as f64 * 18.0,
            1686.0 - (f - 436) as f64 * 18.0,
        )
    } else {
        (240.0, 1686.0)
    };

    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let py = field.y0 + y as u32;
        let rb = py % 3; // scanline banding, period 3px
        let dys = py as f64 - 540.0;
        let g_y = if s_env > 0.0 && dys > -46.0 && dys < 46.0 {
            (-(dys * dys) / 441.6).exp()
        } else {
            0.0
        };
        for x in 0..w {
            let i = y * w + x;
            let pxx = (field.x0 + x as u32) as i64;
            let a = field.mask[i] as f64;
            let d = field.dist[i] as f64 / 3.0;
            let mut v = 0.0f64;
            if !scramble {
                if a > 10.0 {
                    v = 214.0 * a / 255.0; // crisp text (pre-effect)
                }
            } else {
                let cx = (pxx / 3) as u32; // 3x1 px noise cells (micro-streaks)
                let nz = h01(cx, py, ep) * (1.0 - fr) + h01(cx, py, ep + 1) * fr;
                let p;
                if dust {
                    p = if d < 24.0 {
                        dust_base * (1.0 - 0.28 * d / 24.0)
                    } else {
                        0.03 * (455i64.saturating_sub(f as i64)).max(0) as f64 / 10.0
                    };
                } else {
                    let bi = if d < 1.0 {
                        0usize
                    } else if d < 4.0 {
                        1
                    } else if d < 8.0 {
                        2
                    } else if d < 16.0 {
                        3
                    } else if d < 24.0 {
                        4
                    } else {
                        5
                    };
                let mut pb = p_b[bi];
                if bi == 5 && (pxx < 402 || pxx > 1548) {
                    pb = 0.0; // sprinkle only inside box
                }
                // scanline banding: gentle on the core, strong on the fringe
                pb *= if bi < 2 {
                    match rb {
                        0 => 1.1,
                        1 => 1.0,
                        _ => 0.8,
                    }
                } else {
                    match rb {
                        0 => 1.4,
                        1 => 1.05,
                        _ => 0.58,
                    }
                };
                pb = pb.min(0.985);
                p = pb;
            }
                if nz < p {
                    let nb = h01(cx * 3 + 911, py + 377, ep);
                    v = if dust {
                        150.0 + 80.0 * nb
                    } else if d <= 1.6 {
                        255.0 // cores clip white
                    } else {
                        128.0 + 96.0 * nb
                    };
                } else if !dust && d <= 10.0 {
                    v = 44.0 + 12.0 * nz; // pinholes lifted by bloom
                }
            }
            // additive grainy glow band at the text midline (static, y=540)
            if g_y > 0.0 {
                let xf = pxx as f64;
                if xf > sx_l - 8.0 && xf < sx_r + 8.0 {
                    let mut xp = 1.0f64;
                    if xf < sx_l + 60.0 {
                        xp = ((xf - sx_l + 8.0) / 68.0).max(0.0);
                    } else if xf > sx_r - 36.0 {
                        xp = ((sx_r + 8.0 - xf) / 44.0).max(0.0);
                    }
                    if xp > 0.0 {
                        let cs = (pxx / 3) as u32 + 4096;
                        let ng = h01(cs, py, ep) * (1.0 - fr) + h01(cs, py, ep + 1) * fr;
                        let gm = if ng < 0.91 {
                            0.85 + 1.15 * ng
                        } else {
                            3.2 + 2.2 * (ng - 0.91) / 0.09
                        };
                        v += 20.0 * s_env * g_y * xp * gm;
                        if v > 255.0 {
                            v = 255.0;
                        }
                    }
                }
            }
            let o = i * 4;
            if v >= 40.0 {
                let g = v as u8;
                rgba[o] = g;
                rgba[o + 1] = g;
                rgba[o + 2] = g;
                rgba[o + 3] = 255;
            } else if v > 0.8 {
                rgba[o] = 40;
                rgba[o + 1] = 40;
                rgba[o + 2] = 40;
                rgba[o + 3] = (v * 6.375) as u8;
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FNV-1a 64（与锚点脚本 /tmp/r7_anchor.py 一致的对拍校验和）。
    fn fnv1a64(buf: &[u8]) -> u64 {
        let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
        for &b in buf {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100_0000_01B3);
        }
        hash
    }

    #[test]
    fn h01_matches_oracle_anchors() {
        // python: tools/k3dissolve.py h01（32 位 uint64 通道模拟 Math.imul）
        let cases: [(u32, u32, u32, f64); 7] = [
            (0, 0, 0, 0.92743052844889462),
            (1, 2, 3, 0.7796348906122148),
            (77, 144, 8, 0.5401346527505666),
            (563, 430, 58, 0.35521309939213097),
            (867, 1007, 63, 0.74730445514433086),
            (1690, 651, 64, 0.4569370171520859),
            (5759, 1028, 60, 0.56664541969075799),
        ];
        for (a, b, c, want) in cases {
            assert_eq!(h01(a, b, c), want, "h01({a},{b},{c})");
        }
    }

    #[test]
    fn band_p_matches_oracle_anchors() {
        assert_eq!(band_p(411.0), DIS_TAB[0].1);
        let p413 = band_p(413.0);
        for (got, want) in p413
            .iter()
            .zip([0.97, 0.88, 0.79, 0.45000000000000007, 0.125, 0.0215])
        {
            assert_eq!(*got, want);
        }
        assert_eq!(band_p(425.0), DIS_TAB[2].1);
        let p430 = band_p(430.0);
        for (got, want) in p430.iter().zip([0.905, 0.8, 0.77, 0.525, 0.22, 0.0345]) {
            assert_eq!(*got, want);
        }
        // 443 之后（含 dust 段）钉在最后一行
        assert_eq!(band_p(444.0), DIS_TAB[6].1);
        assert_eq!(band_p(455.0), DIS_TAB[6].1);
    }

    #[test]
    fn streak_env_matches_oracle_anchors() {
        let cases = [
            (415.0, 0.0),
            (416.0, 0.42),
            (417.0, 0.565),
            (420.0, 1.0),
            (436.0, 1.0),
            (437.0, 0.58),
            (438.0, 0.33),
            (439.0, 0.125),
            (440.0, 0.0),
            (441.0, 0.0),
        ];
        for (f, want) in cases {
            assert_eq!(streak_env(f), want, "streak_env({f})");
        }
    }

    /// 合成 mask（与 /tmp/r7_anchor.py 完全一致的整数构造）。
    fn synth_mask(w: u32, h: u32) -> Vec<u8> {
        let (w, h) = (w as i64, h as i64);
        let mut mask = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let mut a = 0u8;
                // rect core
                if (30..120).contains(&y) && (100..180).contains(&x) {
                    a = 255;
                }
                // sub-threshold skirt（背景）
                if (10..50).contains(&y) && (1000..1040).contains(&x) {
                    a = 118;
                }
                // disk + ring
                let d2 = (x - 700) * (x - 700) + (y - 111) * (y - 111);
                if d2 <= 60 * 60 {
                    a = 255;
                }
                if d2 > 61 * 61 && d2 <= 64 * 64 {
                    a = 140;
                }
                // diagonal stripes
                if (x + y) % 50 < 6 && y >= 150 {
                    a = 180;
                }
                mask[(y * w + x) as usize] = a;
            }
        }
        mask
    }

    fn synth_field() -> DissolveField {
        let (w, h) = (1460u32, 222u32);
        let mask = synth_mask(w, h);
        let n = (w * h) as usize;
        let mut dist = vec![3000i16; n];
        for i in 0..n {
            if mask[i] > 120 {
                dist[i] = 0;
            }
        }
        chamfer_3_4(&mut dist, w as usize, h as usize);
        DissolveField {
            x0: 232,
            y0: 430,
            w,
            h,
            mask,
            dist,
        }
    }

    #[test]
    fn chamfer_matches_oracle_anchors() {
        let field = synth_field();
        // dist 值以 px*3 为单位（/tmp/r7_anchor.py 输出）
        let cases = [
            (0usize, 0usize, 330i16),
            (120, 60, 0),
            (175, 119, 0),
            (700, 111, 0),
            (760, 111, 0),
            (664, 111, 0),
            (200, 170, 31),
            (1459, 221, 51),
            (1160, 30, 365),
            (519, 165, 32),
        ];
        for (x, y, want) in cases {
            assert_eq!(field.dist[y * field.w as usize + x], want, "dist[{y},{x}]");
        }
    }

    /// 帧渲染与离线 oracle（tools/k3dissolve.py render_frame）逐位对拍：
    /// 锚点 = oracle 在同一合成 mask 上输出的 FNV-1a64（alpha==0 像素 RGB
    /// 归零后哈希，即 JS 写语义）。
    #[test]
    fn render_frame_matches_oracle_bitwise() {
        let field = synth_field();
        let cases = [
            (411u32, 0x64b6_059e_b67e_cc6fu64),
            (413, 0x20cd_8eeb_04c8_1867),
            (420, 0x9543_1288_9161_c56f),
            (427, 0xb832_8fd0_0561_7f94),
            (444, 0x0274_2788_3bbd_3bd9),
            (448, 0x9c56_5dee_9469_5e99),
            (455, 0xa74d_42f4_9abf_faa5),
        ];
        for (f, want) in cases {
            let mut rgba = render_frame_pixels(&field, f);
            // oracle 归一化：alpha==0 → RGB=0（本实现本身就不写这些字节，
            // 但 vec 全零初始化已保证；此处显式归一保持与锚点定义一致）
            for px in rgba.chunks_exact_mut(4) {
                if px[3] == 0 {
                    px[0] = 0;
                    px[1] = 0;
                    px[2] = 0;
                }
            }
            assert_eq!(fnv1a64(&rgba), want, "frame {f} hash mismatch");
        }
    }

    /// 采样像素逐字节对拍（oracle 输出的代表点，覆盖 crisp / scramble /
    /// dust / glow / 背景分支）。
    #[test]
    fn render_frame_sample_pixels_match_oracle() {
        let field = synth_field();
        // (f, x, y, rgba) — python 锚点打印为 px[ly,lx]，此处已换算为 (x,y)
        let cases = [
            (411u32, 120usize, 60usize, [214u8, 214, 214, 255]),
            (411, 0, 0, [0, 0, 0, 0]),
            (413, 120, 60, [55, 55, 55, 255]),
            (413, 175, 119, [255, 255, 255, 255]),
            (413, 200, 170, [174, 174, 174, 255]),
            (413, 1459, 221, [180, 180, 180, 255]),
            (413, 519, 165, [193, 193, 193, 255]),
            (420, 200, 170, [135, 135, 135, 255]),
            (420, 1459, 221, [153, 153, 153, 255]),
            (427, 1160, 30, [204, 204, 204, 255]),
            (427, 1459, 221, [136, 136, 136, 255]),
            (444, 120, 60, [190, 190, 190, 255]),
            (448, 519, 165, [165, 165, 165, 255]),
            (455, 120, 60, [0, 0, 0, 0]),
        ];
        let w = field.w as usize;
        for (f, x, y, want) in cases {
            let rgba = render_frame_pixels(&field, f);
            let o = (y * w + x) * 4;
            assert_eq!(&rgba[o..o + 4], &want[..], "frame {f} px[y={y},x={x}]");
        }
    }

    /// 端到端：surface fillText → build → render。crisp 首帧（f<412）应出现
    /// 214 灰文字；窗口外不绘制。
    #[test]
    fn build_and_render_end_to_end_over_surface() {
        fn inter_font_db() -> fontdb::Database {
            let mut db = fontdb::Database::new();
            db.load_font_data(include_bytes!("../../../../assets/Inter-Regular.ttf").to_vec());
            db
        }
        super::super::scope_font_db(&inter_font_db(), || {
            super::super::surface::surface_create("t-dissolve", 1920, 1080).unwrap();
            super::super::surface::surface_fill_text(
                "t-dissolve",
                "H",
                300.0,
                500.0,
                "Inter",
                400,
                100.0,
                0.0,
                1.0,
                [1.0, 1.0, 1.0, 1.0],
            )
            .unwrap();
            dissolve_build("t-dissolve", 232.0, 430.0, 1460.0, 222.0).unwrap();

            // 窗口外：None
            for t in [13.0, 15.3] {
                assert!(dissolve_render("t-dissolve", t, 30.0, 13.699, 15.2, 455, 232.0, 230.0)
                    .unwrap()
                    .is_none());
            }
            // t=13.7 → f=411 crisp：H 的 ink 核心应为 214 灰、alpha 255
            let out = dissolve_render("t-dissolve", 13.7, 30.0, 13.699, 15.2, 455, 232.0, 230.0)
                .unwrap()
                .expect("f411 renders");
            assert_eq!(out.frame, 411);
            assert_eq!(out.width, 1460);
            assert_eq!(out.height, 222);
            assert_eq!((out.dx, out.dy), (232.0, 230.0));
            let lit = out
                .rgba
                .chunks_exact(4)
                .filter(|px| px[3] == 255 && px[0] == 214)
                .count();
            assert!(lit > 500, "expected crisp ink pixels, got {lit}");

            super::super::surface::surface_delete("t-dissolve");
            dissolve_forget("t-dissolve");
        });
    }
}
