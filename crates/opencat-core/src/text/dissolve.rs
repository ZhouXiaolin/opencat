//! dissolve 效果的场数据层 —— k3-promo 场景 H 的 scramble dissolve。
//!
//! 参考实现 `index.html rebuildDissolveField`（HyperFrames k3-promo，
//! 1254-1314 行）。架构原则（handoff §16）：像素缓冲永不跨进 JS、逐像素
//! 循环永不落在 JS —— 文本排版由 JS 以 `surface.fillText` 薄标记记录
//! （真字体光栅化在 [`super::surface`]），mask 阈值 + 3-4 chamfer 距离场
//! 在本模块构建一次、逐帧复用。
//!
//! 逐帧的 scramble 噪声 / bandP 概率表 / scanline banding / dust / y=540
//! glow 带以 **效果 lambda**（`CK.Effect.fromLambda`，见
//! `script::effects_lambda`）表达，CPU 后端经 `bake_rgba` 烘焙的生成图像
//! 采样场数据。JS 侧薄标记：`surface.buildDissolve(x, y, w, h)`（构建）
//! 与 `surface.bakeDissolve(key)`（每帧烘焙）。算法是帧号的纯函数
//! （seek 安全，与参考一致）。
//!
//! 场数据往返的逐位语义：`bake_rgba` 把 dist（px*3，上限 3000 < 65536）
//! 拆进 G/B 两通道，lambda 侧以 `byte(c.g*255+0.5) + byte(c.b*255+0.5)*256`
//! 精确还原；u8/255 与 *255+0.5 截断在 f64 下往返无损。

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

/// 取一次构建的 field（lambda 效果的烘焙通路用；见 `bake_rgba`）。
pub fn dissolve_field(surface_id: &str) -> Option<Arc<DissolveField>> {
    FIELDS.with(|f| f.borrow().get(surface_id).cloned())
}

/// 把 field 打包成帧级生成图像（straight RGBA8），供 lambda 采样：
/// R = mask（0..255），G = dist 低字节，B = dist 高字节（dist 单位 px*3，
/// 上限 3000 < 65536；u8/255 往返在 f64 下精确），A = 255。lambda 侧以
/// `byte(c.r*255+0.5)` 等还原，逐位无损。
pub fn bake_rgba(field: &DissolveField) -> std::sync::Arc<[u8]> {
    let n = field.w as usize * field.h as usize;
    let mut rgba = vec![0u8; n * 4];
    for i in 0..n {
        let d = field.dist[i] as u16;
        rgba[i * 4] = field.mask[i];
        rgba[i * 4 + 1] = (d & 0xFF) as u8;
        rgba[i * 4 + 2] = (d >> 8) as u8;
        rgba[i * 4 + 3] = 255;
    }
    std::sync::Arc::from(rgba)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成 mask（与离线锚点脚本一致的整数构造）。
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
        // dist 值以 px*3 为单位（锚点脚本输出）
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

    /// bake_rgba 往返：R=mask、G/B=dist 低/高字节，u8 精确还原。
    #[test]
    fn bake_rgba_roundtrip() {
        let field = synth_field();
        let rgba = bake_rgba(&field);
        let n = (field.w * field.h) as usize;
        for i in [0usize, 120 * 1460 + 60, 119 * 1460 + 175, n - 1] {
            let m = rgba[i * 4];
            let d = rgba[i * 4 + 1] as u16 | ((rgba[i * 4 + 2] as u16) << 8);
            assert_eq!(m, field.mask[i], "mask[{i}]");
            assert_eq!(d as i16, field.dist[i], "dist[{i}]");
            assert_eq!(rgba[i * 4 + 3], 255);
        }
    }
}
