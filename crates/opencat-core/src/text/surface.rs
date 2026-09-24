//! 离屏像素 surface + 真字体 2D 文本光栅化（脚本引擎 canvas 能力补全）。
//!
//! 参考实现 `index.html drawDissolve`（HyperFrames k3-promo，1240-1523 行）依赖
//! 三个 DOM canvas 2D 能力：真字体 `fillText`、`measureText`（ink box）、
//! `getImageData` 像素读回。本模块在 core 内以 fontdb + swash 直接实现这三者，
//! 不依赖 Skia（core 无渲染后端）。
//!
//! 语义锚定（canvas 2D / Chrome）：
//! - advance = hmtx 横向步进之和，**无 kerning**（handoff §14.3-4：canvas
//!   measureText 与 hmtx advance 同源）；
//! - `letter_spacing` 在**每个字符之后**追加（含末字符，Chrome canvas
//!   letterSpacing 语义；由 XML 烘焙 squeeze 锚点 `260/nat("JTX.")=0.8264503`
//!   反推 nat=314.5665 vs 本实现 314.597 验证，Δ0.03px）；
//! - `measureText` 返回 ink box：`actualBoundingBoxLeft = 锚点x − ink左缘`
//!   （ink 在锚点右侧时为负）、`actualBoundingBoxAscent = ink顶 − 基线`
//!   （向上为正）。index.html `setBrace`（455-468 行）依赖该约定。
//!
//! surface 注册表为 thread-local（与 `scope_font_db` 同一执行线程），生命周期
//! 跨帧：参考侧离屏栅格只在字体加载后重建一次，XML 端以 same-id 复用。

use std::cell::RefCell;

use anyhow::{Result, bail};
use hashbrown::HashMap;
use swash::scale::{Render, ScaleContext, Source};
use swash::{FontRef, GlyphId};

use super::font_db_for_script;

/// 一张离屏 RGBA 表面（unpremultiplied，行主序，长度 = w*h*4）。
struct Surface {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

thread_local! {
    static SURFACES: RefCell<HashMap<String, Surface>> = RefCell::new(HashMap::new());
}

/// 删除一张 surface（宿主在 pipeline 结束时可显式回收；不调用也无正确性影响）。
pub fn surface_delete(id: &str) {
    SURFACES.with(|s| {
        s.borrow_mut().remove(id);
    });
}

/// 创建（或重置）一张全透明 surface。
pub fn surface_create(id: &str, width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        bail!("surface `{id}`: unsupported dimensions {width}x{height}");
    }
    let len = width as usize * height as usize * 4;
    SURFACES.with(|s| {
        s.borrow_mut().insert(
            id.to_string(),
            Surface {
                width,
                height,
                rgba: vec![0u8; len],
            },
        );
    });
    Ok(())
}

fn with_surface<R>(id: &str, f: impl FnOnce(&mut Surface) -> Result<R>) -> Result<R> {
    SURFACES.with(|s| {
        let mut map = s.borrow_mut();
        let Some(surface) = map.get_mut(id) else {
            bail!("surface `{id}`: not created (ctx.createSurface first)");
        };
        f(surface)
    })
}

/// 查询一张 surface 的 (width, height)。
pub fn surface_dimensions(id: &str) -> Result<(u32, u32)> {
    with_surface(id, |surface| Ok((surface.width, surface.height)))
}

/// 清空一个子矩形为透明黑（canvas clearRect 语义；四边形参数向内裁剪）。
pub fn surface_clear(id: &str, x: f64, y: f64, w: f64, h: f64) -> Result<()> {
    with_surface(id, |surface| {
        let (iw, ih) = (surface.width as i64, surface.height as i64);
        let x0 = (x.floor() as i64).max(0);
        let y0 = (y.floor() as i64).max(0);
        let x1 = ((x + w).ceil() as i64).min(iw);
        let y1 = ((y + h).ceil() as i64).min(ih);
        for yy in y0..y1 {
            let row = yy as usize * surface.width as usize;
            for xx in x0..x1 {
                let o = (row + xx as usize) * 4;
                surface.rgba[o..o + 4].fill(0);
            }
        }
        Ok(())
    })
}

/// 读取一个子矩形的 RGBA 字节（canvas getImageData 语义：返回拷贝）。
pub fn surface_get_rgba(id: &str, x: f64, y: f64, w: f64, h: f64) -> Result<Vec<u8>> {
    with_surface(id, |surface| {
        if w <= 0.0 || h <= 0.0 {
            bail!("surface `{id}`: getImageData with empty rect");
        }
        let iw = surface.width as i64;
        let ih = surface.height as i64;
        let x0 = (x.floor() as i64).max(0);
        let y0 = (y.floor() as i64).max(0);
        let x1 = ((x + w).ceil() as i64).min(iw);
        let y1 = ((y + h).ceil() as i64).min(ih);
        if x1 <= x0 || y1 <= y0 {
            bail!("surface `{id}`: getImageData rect out of bounds");
        }
        let ow = (x1 - x0) as usize;
        let mut out = vec![0u8; ow * (y1 - y0) as usize * 4];
        for (row, yy) in (y0..y1).enumerate() {
            let src = (yy as usize * surface.width as usize + x0 as usize) * 4;
            let dst = row * ow * 4;
            out[dst..dst + ow * 4].copy_from_slice(&surface.rgba[src..src + ow * 4]);
        }
        Ok(out)
    })
}

/// `measureText` 的 ink-box 结果（CSS 单位 px，基线y向下）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct InkMetrics {
    /// advance 宽度（含 letterSpacing，Chrome canvas `.width`）。
    pub width: f64,
    /// 锚点x 到 ink 左缘的距离，向左为正（canvas `actualBoundingBoxLeft`）。
    pub ink_left: f64,
    /// 锚点x 到 ink 右缘的距离，向右为正（canvas `actualBoundingBoxRight`）。
    pub ink_right: f64,
    /// 基线到 ink 顶缘，向上为正（canvas `actualBoundingBoxAscent`）。
    pub ink_ascent: f64,
    /// 基线到 ink 底缘，向下为正（canvas `actualBoundingBoxDescent`）。
    pub ink_descent: f64,
}

/// 以 scoped 字体库（`scope_font_db`）解析 family+weight → swash `FontRef`。
///
/// 返回 `(FontRef 持有的数据闭包结果, face index)`——借用的数据生命周期由
/// `with_face_data` 闭包约束，调用方在闭包内完成全部 swash 工作。
fn with_resolved_font<R>(
    family: &str,
    weight: u32,
    f: impl FnOnce(&FontRef<'_>) -> Result<R>,
) -> Result<R> {
    let db = font_db_for_script();
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: fontdb::Weight(weight as u16),
        style: fontdb::Style::Normal,
        stretch: fontdb::Stretch::Normal,
    };
    let Some(face_id) = db.query(&query) else {
        bail!("surface: no font face for family `{family}` weight {weight}");
    };
    let mut out: Option<Result<R>> = None;
    db.with_face_data(face_id, |data, index| -> Result<()> {
        let Some(font) = FontRef::from_index(data, index as usize) else {
            bail!("surface: face `{face_id:?}` is not a readable font file");
        };
        out = Some(f(&font));
        Ok(())
    });
    out.unwrap_or_else(|| bail!("surface: font data missing for family `{family}`"))
}

/// 单字符 → glyph id（未命中返回 0 = .notdef，与 canvas 行为一致）。
fn glyph_of(font: &FontRef<'_>, ch: char) -> GlyphId {
    font.charmap().map(ch as u32)
}

/// 逐字符布局：返回每字符 (glyph_id, pen_x)（pen 在本字符绘制前）与总宽。
///
/// advance = hmtx（无 kerning），每字符后追加 `letter_spacing`（含末字符）。
fn layout_text(
    font: &FontRef<'_>,
    text: &str,
    size_px: f64,
    letter_spacing: f64,
) -> (Vec<(GlyphId, f64)>, f64) {
    let metrics = font.glyph_metrics(&[]).scale(size_px as f32);
    let mut pen = 0.0f64;
    let mut placed = Vec::with_capacity(text.chars().count());
    for ch in text.chars() {
        let gid = glyph_of(font, ch);
        placed.push((gid, pen));
        pen += metrics.advance_width(gid) as f64 + letter_spacing;
    }
    (placed, pen)
}

/// `canvas.measureText`：真字体、kern-free、含 letterSpacing（末字符后也追加）。
pub fn surface_measure_text(
    text: &str,
    family: &str,
    weight: u32,
    size_px: f64,
    letter_spacing: f64,
) -> Result<InkMetrics> {
    with_resolved_font(family, weight, |font| {
        if text.is_empty() {
            return Ok(InkMetrics {
                width: 0.0,
                ink_left: 0.0,
                ink_right: 0.0,
                ink_ascent: 0.0,
                ink_descent: 0.0,
            });
        }
        let (placed, width) = layout_text(font, text, size_px, letter_spacing);
        // ink box：对每个字形走一次 outline 栅格，取覆盖区极值
        let mut context = ScaleContext::new();
        let mut scaler = context.builder(*font).size(size_px as f32).hint(false).build();
        let render = Render::new(&[Source::Outline]);
        let mut ink_l = f64::INFINITY;
        let mut ink_r = f64::NEG_INFINITY;
        let mut ink_t = f64::INFINITY;
        let mut ink_b = f64::NEG_INFINITY;
        for (gid, pen_x) in &placed {
            let Some(image) = render.render(&mut scaler, *gid) else {
                continue;
            };
            let p = image.placement;
            if p.width == 0 || p.height == 0 {
                continue;
            }
            let left = pen_x + p.left as f64;
            // swash placement.top：基线向上为正 → y-down 空间的顶部 = 基线 − top
            ink_l = ink_l.min(left);
            ink_r = ink_r.max(left + p.width as f64);
            ink_t = ink_t.min(p.top as f64);
            ink_b = ink_b.max(p.top as f64 - p.height as f64);
        }
        if !ink_l.is_finite() {
            // 空白文本：ink box 全零
            return Ok(InkMetrics {
                width,
                ink_left: 0.0,
                ink_right: 0.0,
                ink_ascent: 0.0,
                ink_descent: 0.0,
            });
        }
        Ok(InkMetrics {
            width,
            // canvas 约定：actualBoundingBoxLeft 向左为正 → = -ink_l（ink_l 相对锚点向右）
            ink_left: -ink_l,
            ink_right: ink_r,
            ink_ascent: ink_t,
            ink_descent: -ink_b,
        })
    })
}

/// `canvas.fillText`：把文本 coverage 以 SrcOver 合成进 surface。
///
/// - `x`,`y`：笔原点（基线锚点，已含调用方矩阵平移）。
/// - `scale_x`：水平 squeeze（等价 `ctx.scale(s,1)` 后 fillText：步进与字形
///   位图都乘 s；旋转/非均匀垂直缩放不受支持，调用方 facade 会拒绝）。
/// - `fill`：RGBA 0..1（unpremul）。
#[allow(clippy::too_many_arguments)]
pub fn surface_fill_text(
    id: &str,
    text: &str,
    x: f64,
    y: f64,
    family: &str,
    weight: u32,
    size_px: f64,
    letter_spacing: f64,
    scale_x: f64,
    fill: [f32; 4],
) -> Result<()> {
    if scale_x <= 0.0 {
        bail!("surface `{id}`: fillText scale_x must be > 0");
    }
    with_surface(id, |surface| {
        with_resolved_font(family, weight, |font| {
            let (placed, _width) = layout_text(font, text, size_px, letter_spacing);
            let mut context = ScaleContext::new();
            let mut scaler = context.builder(*font).size(size_px as f32).hint(false).build();
            let render = Render::new(&[Source::Outline]);
            let sw = surface.width as i64;
            let sh = surface.height as i64;
            let [fr, fg, fb, fa] = fill;
            for (gid, pen_x) in &placed {
                let Some(image) = render.render(&mut scaler, *gid) else {
                    continue;
                };
                let p = image.placement;
                if p.width == 0 || p.height == 0 {
                    continue;
                }
                let coverage: &[u8] = &image.data;
                let left = x + scale_x * (*pen_x + p.left as f64);
                // swash top 基线向上为正 → 位图顶 = y − top，底 = 顶 + height
                let top = y - p.top as f64;
                let bw = (scale_x * p.width as f64).ceil() as i64;
                let bh = p.height as i64;
                let x0 = left.floor() as i64;
                let y0 = top.floor() as i64;
                for yy in y0.max(0)..(y0 + bh).min(sh) {
                    let srow = (yy - y0) as usize;
                    let drow = yy as usize * surface.width as usize;
                    for xx in x0.max(0)..(x0 + bw).min(sw) {
                        // 水平 squeeze 下按比例重采样（bitmaps 通常 1-2px 宽缩放）
                        let sx = ((xx - x0) as f64 / scale_x) as usize;
                        let s = coverage[srow * p.width as usize + sx.min(p.width as usize - 1)];
                        if s == 0 {
                            continue;
                        }
                        let sa = (s as f32 / 255.0) * fa;
                        let o = (drow + xx as usize) * 4;
                        let da = surface.rgba[o + 3] as f32 / 255.0;
                        let oa = sa + da * (1.0 - sa);
                        if oa <= 0.0 {
                            surface.rgba[o..o + 4].fill(0);
                            continue;
                        }
                        for c in 0..3 {
                            let sc = match c {
                                0 => fr,
                                1 => fg,
                                _ => fb,
                            };
                            let dc = surface.rgba[o + c] as f32 / 255.0;
                            let oc = (sc * sa + dc * da * (1.0 - sa)) / oa;
                            surface.rgba[o + c] = (oc.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                        }
                        surface.rgba[o + 3] = (oa.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            }
            Ok(())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试字体库：Inter Light/Regular/Medium（k3-promo 的运行时字体）。
    fn inter_font_db() -> fontdb::Database {
        let mut db = fontdb::Database::new();
        db.load_font_data(include_bytes!("../../../../assets/Inter-Regular.ttf").to_vec());
        db
    }

    fn measure(text: &str, weight: u32, size: f64, ls: f64) -> InkMetrics {
        super::super::scope_font_db(&inter_font_db(), || {
            surface_measure_text(text, "Inter", weight, size, ls).unwrap()
        })
    }

    /// 锚点：XML 烘焙 squeeze gw1=0.8264503 = 260/nat("JTX.") @148.4536px,-2.5
    /// → nat = 314.5665。fontTools hmtx 预算 = 314.597（Δ0.03px，见模块注释）。
    #[test]
    fn measure_text_matches_baked_gw1_squeeze_anchor() {
        let m = measure("JTX.", 400, 148.4536, -2.5);
        let nat_expected = 260.0 / 0.8264503;
        assert!(
            (m.width - nat_expected).abs() < 0.15,
            "nat(JTX.) = {} should be ≈ {nat_expected} (hmtx 314.597)",
            m.width
        );
        // squeeze 反算必须落在烘焙值 0.8264503 的 ~0.05% 内
        let squeeze = 260.0 / m.width;
        assert!(
            (squeeze - 0.8264503).abs() < 4e-4,
            "squeeze {squeeze} vs baked 0.8264503"
        );
    }

    /// letterSpacing 在每个字符之后追加（含末字符）：4 字符串 -2.5px×4。
    #[test]
    fn letter_spacing_appends_after_every_char_including_last() {
        let no_ls = measure("JTX.", 400, 148.4536, 0.0).width;
        let with_ls = measure("JTX.", 400, 148.4536, -2.5).width;
        assert!(
            (no_ls - with_ls - 10.0).abs() < 1e-6,
            "4 chars × -2.5px should shrink width by 10, got {}",
            no_ls - with_ls
        );
    }

    /// ink metrics：「H」cap 高度 / fs ≈ 0.7275（handoff §5 硬常量 1490/2048）。
    #[test]
    fn ink_ascent_of_h_matches_inter_cap_height() {
        let m = measure("H", 400, 100.0, 0.0);
        let cap_asc = m.ink_ascent / 100.0;
        assert!(
            (cap_asc - 0.7275).abs() < 0.005,
            "Inter capAsc = {cap_asc}, expected ≈ 0.7275"
        );
        assert!((m.ink_descent / 100.0).abs() < 1e-6, "H has no descent ink");
        // actualBoundingBoxLeft：ink 右于锚点 → 负值（canvas 约定）
        assert!(m.ink_left <= 0.0 + 1e-6, "H ink_left should be ≤ 0");
    }

    /// fillText + getImageData：白字写进透明 surface，alpha 覆盖出现。
    #[test]
    fn fill_text_produces_alpha_coverage_readable_back() {
        super::super::scope_font_db(&inter_font_db(), || {
            surface_create("t-raster", 200, 200).unwrap();
            surface_fill_text("t-raster", "H", 50.0, 150.0, "Inter", 400, 100.0, 0.0, 1.0,
                [1.0, 1.0, 1.0, 1.0])
                .unwrap();
            let px = surface_get_rgba("t-raster", 0.0, 0.0, 200.0, 200.0).unwrap();
            let mut max_a = 0u8;
            let mut lit = 0usize;
            for i in 0..(200 * 200) {
                let a = px[i * 4 + 3];
                max_a = max_a.max(a);
                if a > 0 {
                    lit += 1;
                    // 白字：RGB 应等于 alpha（unpremul, 白色 fill）
                    assert_eq!(px[i * 4], 255);
                }
            }
            assert!(max_a >= 250, "core of H should be near-opaque, got {max_a}");
            // 100px 的 H ink 大约 73×52px → 数千个覆盖像素
            assert!(lit > 1000, "expected thousands of lit pixels, got {lit}");
            surface_delete("t-raster");
        });
    }

    /// clearRect 只清子矩形。
    #[test]
    fn clear_rect_scopes_to_subrect() {
        super::super::scope_font_db(&inter_font_db(), || {
            surface_create("t-clear", 40, 40).unwrap();
            surface_fill_text("t-clear", "H", 5.0, 35.0, "Inter", 400, 40.0, 0.0, 1.0,
                [1.0, 1.0, 1.0, 1.0])
                .unwrap();
            surface_clear("t-clear", 0.0, 0.0, 40.0, 20.0).unwrap();
            let px = surface_get_rgba("t-clear", 0.0, 0.0, 40.0, 40.0).unwrap();
            for y in 0..20 {
                for x in 0..40 {
                    assert_eq!(px[(y * 40 + x) * 4 + 3], 0, "top half must be cleared");
                }
            }
            surface_delete("t-clear");
        });
    }

    /// 未知 family 必须显式报错（不静默回退）。
    #[test]
    fn unknown_family_errors() {
        super::super::scope_font_db(&inter_font_db(), || {
            let err = surface_measure_text("A", "NoSuchFamily", 400, 20.0, 0.0).unwrap_err();
            assert!(err.to_string().contains("no font face"));
        });
    }
}
