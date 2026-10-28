//! 离屏像素 surface + 真字体 2D 文本光栅化（脚本引擎 canvas 能力补全）。
//!
//! 参考实现 `index.html` 的离屏 2D canvas（HyperFrames k3-promo，1240-1523
//! 行）依赖三个 DOM canvas 2D 能力：真字体 `fillText`、`measureText`
//! （ink box）、`getImageData` 像素读回。本模块在 core 内以 fontdb + swash
//! 直接实现这三者，不依赖 Skia（core 无渲染后端）。surface 同时是脚本的
//! render target（`target.apply` / `target.sweep` 的目标缓冲）。
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
use std::sync::Arc;

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
    /// 内容世代：每次写入（clear / fillText / run / sweep / 整图写回）自增。
    /// 供 SKSL 路径把 surface 自动烘成帧级生成图像时推导确定性 key ——
    /// 世代相同 ⟺ 内容相同，所以同 key 异像素的碰撞不可能发生。
    generation: u64,
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

/// surface 单边尺寸上限（8192 = 常见 GL 纹理上限的保守值）。
pub const MAX_SURFACE_DIMENSION: u32 = 8192;

/// 创建（或重置）一张全透明 surface。
pub fn surface_create(id: &str, width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > MAX_SURFACE_DIMENSION || height > MAX_SURFACE_DIMENSION {
        bail!("surface `{id}`: unsupported dimensions {width}x{height}（单边上限 {MAX_SURFACE_DIMENSION}）");
    }
    let len = width as usize * height as usize * 4;
    SURFACES.with(|s| {
        s.borrow_mut().insert(
            id.to_string(),
            Surface {
                width,
                height,
                rgba: vec![0u8; len],
                generation: 0,
            },
        );
    });
    Ok(())
}

fn with_surface<R>(id: &str, f: impl FnOnce(&mut Surface) -> Result<R>) -> Result<R> {
    SURFACES.with(|s| {
        let mut map = s.borrow_mut();
        let Some(surface) = map.get_mut(id) else {
            bail!("surface `{id}`: not created (ly.surface first)");
        };
        f(surface)
    })
}

/// 查询一张 surface 的 (width, height)。
pub fn surface_dimensions(id: &str) -> Result<(u32, u32)> {
    with_surface(id, |surface| Ok((surface.width, surface.height)))
}

/// Get-or-resize: reuse the buffer when the size already matches so a chart
/// node's paragraph surface survives across frames, and reallocate only when
/// the label geometry changed.
pub fn surface_create_or_resize(id: &str, width: u32, height: u32) -> Result<()> {
    match surface_dimensions(id) {
        Ok((w, h)) if w == width && h == height => Ok(()),
        _ => surface_create(id, width, height),
    }
}

/// 查询一张 surface 的内容世代（每次写入自增）。SKSL 路径把 surface 自动
/// 烘成帧级生成图像时，用它拼确定性 key：世代相同即内容相同。
pub fn surface_generation(id: &str) -> Result<u64> {
    with_surface(id, |surface| Ok(surface.generation))
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
        surface.generation = surface.generation.wrapping_add(1);
        Ok(())
    })
}

/// `canvas.fillRect` / `fillRoundRect` / 线性渐变的统一入口：轴对齐圆角
/// 矩形，SrcOver 合成进 surface（覆盖率抗锯齿）。
///
/// `radius` 为圆角半径（0 = 直角）；`axis` 是从 `a` 到 `b` 的渐变轴，`stops`
/// 是 `(offset, [r,g,b,a])` 列表（offset 0..1，不要求均匀；空 = 用最后一个
/// stop 当纯色）。颜色分量 0..1、unpremul。
pub fn surface_fill_shape(
    id: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius: f64,
    axis: ([f64; 2], [f64; 2]),
    stops: &[(f32, [f32; 4])],
) -> Result<()> {
    let colors: Vec<[f32; 4]> = if stops.is_empty() {
        vec![[0.0, 0.0, 0.0, 1.0]]
    } else {
        stops.iter().map(|(_, c)| *c).collect()
    };
    if colors.iter().all(|c| c[3] <= 0.0) || !(w > 0.0) || !(h > 0.0) {
        return Ok(());
    }
    let (a, b) = axis;
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    with_surface(id, |surface| {
        let sw = surface.width as f64;
        let sh = surface.height as f64;
        let x0 = x.max(0.0);
        let y0 = y.max(0.0);
        let x1 = (x + w).min(sw);
        let y1 = (y + h).min(sh);
        if x1 <= x0 || y1 <= y0 {
            return Ok(());
        }
        let r = radius.max(0.0).min((x1 - x0).min(y1 - y0) * 0.5);
        // 圆角矩形的有符号距离（像素中心 → 边界，负 = 内部）；覆盖率 =
        // clamp(0.5 - d, 0, 1) 即 1 像素宽抗锯齿带，与 canvas 2D 一致。
        let (ccx, ccy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        let (hx, hy) = ((x1 - x0) * 0.5 - r, (y1 - y0) * 0.5 - r);
        let coverage = |px: f64, py: f64| -> f64 {
            let qx = (px + 0.5 - ccx).abs() - hx;
            let qy = (py + 0.5 - ccy).abs() - hy;
            let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt()
                + qx.max(qy).min(0.0)
                - r;
            (0.5 - d).clamp(0.0, 1.0)
        };
        // 渐变轴上的位置 → stop 插值。
        let sample = |t: f32| -> [f32; 4] {
            if stops.is_empty() {
                return colors[0];
            }
            let mut lo = &stops[0];
            for s in stops {
                if s.0 <= t {
                    lo = s;
                }
            }
            let mut hi = lo;
            for s in stops {
                if s.0 >= t {
                    hi = s;
                    break;
                }
            }
            let span = hi.0 - lo.0;
            let k = if span > 1e-6 { ((t - lo.0) / span).clamp(0.0, 1.0) } else { 0.0 };
            let mut out = [0.0f32; 4];
            for i in 0..4 {
                out[i] = lo.1[i] + (hi.1[i] - lo.1[i]) * k;
            }
            out
        };
        for py in y0.floor() as usize..(y1.ceil() as usize).min(surface.height as usize) {
            for px in x0.floor() as usize..(x1.ceil() as usize).min(surface.width as usize) {
                let cov = coverage(px as f64, py as f64) as f32;
                if cov <= 0.0 {
                    continue;
                }
                let t = if len2 > 1e-12 {
                    ((((px as f64 + 0.5) - a[0]) * dx + ((py as f64 + 0.5) - a[1]) * dy) / len2)
                        .clamp(0.0, 1.0) as f32
                } else {
                    1.0
                };
                let [fr, fg, fb, fa] = sample(t);
                let sa = cov * fa;
                if sa <= 0.0 {
                    continue;
                }
                let o = (py * surface.width as usize + px) * 4;
                let da = surface.rgba[o + 3] as f32 / 255.0;
                let oa = sa + da * (1.0 - sa);
                if oa <= 0.0 {
                    surface.rgba[o..o + 4].fill(0);
                    continue;
                }
                for (c, sc) in [fr, fg, fb].iter().enumerate() {
                    let dc = surface.rgba[o + c] as f32 / 255.0;
                    let oc = (sc * sa + dc * da * (1.0 - sa)) / oa;
                    surface.rgba[o + c] = (oc.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
                surface.rgba[o + 3] = (oa.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        }
        surface.generation = surface.generation.wrapping_add(1);
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

/// 整图读取（Arc 拷贝），供 render target 采样（lambda 的 surface child /
/// bake 注册生成图像）。
pub fn surface_rgba_arc(id: &str) -> Result<std::sync::Arc<[u8]>> {
    with_surface(id, |surface| {
        Ok(std::sync::Arc::from(surface.rgba.clone()))
    })
}

/// 整图写回（尺寸必须与既有 surface 完全一致）——render target 的
/// `apply` 写入口。
pub fn surface_write_rgba(id: &str, width: u32, height: u32, rgba: Vec<u8>) -> Result<()> {
    with_surface(id, |surface| {
        if surface.width != width || surface.height != height {
            bail!(
                "surface `{id}`: write dims {width}x{height} != surface {}x{}",
                surface.width,
                surface.height
            );
        }
        if rgba.len() != surface.rgba.len() {
            bail!(
                "surface `{id}`: write payload {} bytes != {}",
                rgba.len(),
                surface.rgba.len()
            );
        }
        surface.rgba = rgba;
        surface.generation = surface.generation.wrapping_add(1);
        Ok(())
    })
}

/// 就地访问整图缓冲（render target 的 `sweep` 写入口）。
pub fn surface_with_rgba_mut<R>(
    id: &str,
    f: impl FnOnce(u32, u32, &mut [u8]) -> Result<R>,
) -> Result<R> {
    with_surface(id, |surface| {
        let r = f(surface.width, surface.height, &mut surface.rgba)?;
        surface.generation = surface.generation.wrapping_add(1);
        Ok(r)
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
    /// 基线到字体盒顶缘，向上为正（canvas `fontBoundingBoxAscent`；hhea/OS/2
    /// typo 垂直度量 × size/upem，与文本内容无关）。
    pub font_ascent: f64,
    /// 基线到字体盒底缘，向下为正（canvas `fontBoundingBoxDescent`）。
    pub font_descent: f64,
}

/// 以 scoped 字体库（`scope_font_db`）解析 family+weight → swash `FontRef`。
///
/// 返回 `(FontRef 持有的数据闭包结果, face index)`——借用的数据生命周期由
/// `with_face_data` 闭包约束，调用方在闭包内完成全部 swash 工作。
///
/// `FaceKey` 是字体数据缓冲的 (指针, face index)。它与 `fontdb::Database`
/// 的实例无关：scoped DB 每帧克隆，但字体字节由同一个 Arc 持有，指针稳定。
/// 字形光栅缓存用它做键，跨 DB 克隆仍命中同一 face。
type FaceKey = (usize, u32);

fn with_resolved_font<R>(
    family: &str,
    weight: u32,
    f: impl FnOnce(&FontRef<'_>, FaceKey) -> Result<R>,
) -> Result<R> {
    let db = font_db_for_script();
    // fontdb resolves CSS generic families (`sans-serif`, `monospace`, …) as a
    // `Family::Serif`/`SansSerif`/`Monospace` query variant, while G2 and other
    // third-party specs pass the CSS keyword verbatim as a family *name* — which
    // `Family::Name` never matches. Map the keyword to the generic variant so
    // the query lands on the registered generic family.
    let family_kind = match crate::style::GenericFamily::parse_css(family) {
        Some(crate::style::GenericFamily::Serif) => fontdb::Family::Serif,
        Some(crate::style::GenericFamily::SansSerif) => fontdb::Family::SansSerif,
        Some(crate::style::GenericFamily::Monospace) => fontdb::Family::Monospace,
        None => fontdb::Family::Name(family),
    };
    let query = fontdb::Query {
        families: &[family_kind],
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
        out = Some(f(&font, (data.as_ptr() as usize, index)));
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

/// 单字形轮廓光栅（swash `Source::Outline`、`hint=false`、整数偏移）。
///
/// 同一 (face, size, glyph) 的光栅结果与调用位置无关，也跨帧不变。缓存它把
/// `canvas.fillText` 的每次调用从「新建 ScaleContext + 重栅格化轮廓」降为一次
/// 哈希查找；`ccRender` 这类逐字符画满 12864 格的用法是主要收益方。
struct GlyphRaster {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    data: Arc<[u8]>,
}

/// 缓存容量上限。条目是单个字形的 coverage 位图（通常 < 1 KiB），4096 条
/// 覆盖任何现实字形集；溢出即整表清空（演示渲染的复用窗口远小于此）。
const GLYPH_RASTER_CACHE_CAP: usize = 4096;

thread_local! {
    static GLYPH_RASTERS: RefCell<HashMap<(FaceKey, u64, GlyphId), Option<Arc<GlyphRaster>>>> =
        RefCell::new(HashMap::new());
}

/// 取（或栅格化）一个字形。`None` = 空字形（无覆盖，与 `render` 失败同义）。
fn glyph_raster(
    face: FaceKey,
    font: &FontRef<'_>,
    size_px: f64,
    gid: GlyphId,
) -> Option<Arc<GlyphRaster>> {
    let key = (face, size_px.to_bits(), gid);
    let cache_enabled = !crate::ablate::ablated(crate::ablate::Ablation::GlyphRasters);
    if cache_enabled
        && let Some(hit) = GLYPH_RASTERS.with(|c| c.borrow().get(&key).cloned())
    {
        return hit;
    }
    let mut context = ScaleContext::new();
    let mut scaler = context.builder(*font).size(size_px as f32).hint(false).build();
    let render = Render::new(&[Source::Outline]);
    let raster = render.render(&mut scaler, gid).and_then(|image| {
        let p = image.placement;
        if p.width == 0 || p.height == 0 {
            return None;
        }
        Some(Arc::new(GlyphRaster {
            left: p.left,
            top: p.top,
            width: p.width,
            height: p.height,
            data: Arc::from(image.data.as_slice()),
        }))
    });
    if cache_enabled {
        GLYPH_RASTERS.with(|c| {
            let mut cache = c.borrow_mut();
            if cache.len() >= GLYPH_RASTER_CACHE_CAP {
                cache.clear();
            }
            cache.insert(key, raster.clone());
        });
    }
    raster
}

/// `canvas.measureText`：真字体、kern-free、含 letterSpacing（末字符后也追加）。
pub fn surface_measure_text(
    text: &str,
    family: &str,
    weight: u32,
    size_px: f64,
    letter_spacing: f64,
) -> Result<InkMetrics> {
    with_resolved_font(family, weight, |font, face| {
        // 字体盒（fontBoundingBox）：swash 已按 USE_TYPO_METRICS 优先取 OS/2
        // typo 垂直度量，否则 hhea（Chrome/Skia 同口径）；descent 恒正。
        let fm = font.metrics(&[]);
        let font_scale = size_px / f64::from(fm.units_per_em);
        let font_ascent = f64::from(fm.ascent) * font_scale;
        let font_descent = f64::from(fm.descent) * font_scale;
        if text.is_empty() {
            return Ok(InkMetrics {
                width: 0.0,
                ink_left: 0.0,
                ink_right: 0.0,
                ink_ascent: 0.0,
                ink_descent: 0.0,
                font_ascent,
                font_descent,
            });
        }
        let (placed, width) = layout_text(font, text, size_px, letter_spacing);
        // ink box：对每个字形取 outline 覆盖区的极值（走共享字形光栅缓存）
        let mut ink_l = f64::INFINITY;
        let mut ink_r = f64::NEG_INFINITY;
        // Seeds match the accumulator direction: the top starts at the lowest
        // possible ceiling and the bottom at the highest possible floor.
        let mut ink_t = f64::NEG_INFINITY;
        let mut ink_b = f64::INFINITY;
        for (gid, pen_x) in &placed {
            let Some(raster) = glyph_raster(face, font, size_px, *gid) else {
                continue;
            };
            let left = pen_x + raster.left as f64;
            // swash placement.top：基线向上为正 → y-down 空间的顶部 = 基线 − top
            ink_l = ink_l.min(left);
            ink_r = ink_r.max(left + raster.width as f64);
            // swash placement.top counts UP from the baseline, so the topmost
            // edge of the whole run is the MAX top and the bottom edge (top
            // minus the bitmap height, i.e. a negative number for glyphs that
            // dip below the baseline) is the MIN. Taking the extremes the other
            // way round reports an x-height box for a capitalised string and
            // drops descender ink — both off against Chrome's ink boxes.
            ink_t = ink_t.max(raster.top as f64);
            ink_b = ink_b.min(raster.top as f64 - raster.height as f64);
        }
        if !ink_l.is_finite() {
            // 空白文本：ink box 全零
            return Ok(InkMetrics {
                width,
                ink_left: 0.0,
                ink_right: 0.0,
                ink_ascent: 0.0,
                ink_descent: 0.0,
                font_ascent,
                font_descent,
            });
        }
        Ok(InkMetrics {
            width,
            // canvas 约定：actualBoundingBoxLeft 向左为正 → = -ink_l（ink_l 相对锚点向右）
            ink_left: -ink_l,
            ink_right: ink_r,
            ink_ascent: ink_t,
            ink_descent: -ink_b,
            font_ascent,
            font_descent,
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
        with_resolved_font(family, weight, |font, face| {
            let (placed, _width) = layout_text(font, text, size_px, letter_spacing);
            let sw = surface.width as i64;
            let sh = surface.height as i64;
            let [fr, fg, fb, fa] = fill;
            for (gid, pen_x) in &placed {
                let Some(raster) = glyph_raster(face, font, size_px, *gid) else {
                    continue;
                };
                let coverage: &[u8] = &raster.data;
                let gw = raster.width as usize;
                let left = x + scale_x * (*pen_x + raster.left as f64);
                // swash top 基线向上为正 → 位图顶 = y − top，底 = 顶 + height
                let top = y - raster.top as f64;
                let bw = (scale_x * raster.width as f64).ceil() as i64;
                let bh = raster.height as i64;
                let x0 = left.floor() as i64;
                let y0 = top.floor() as i64;
                for yy in y0.max(0)..(y0 + bh).min(sh) {
                    let srow = (yy - y0) as usize;
                    let drow = yy as usize * surface.width as usize;
                    for xx in x0.max(0)..(x0 + bw).min(sw) {
                        // 水平 squeeze 下按比例重采样（bitmaps 通常 1-2px 宽缩放）
                        let sx = ((xx - x0) as f64 / scale_x) as usize;
                        let s = coverage[srow * gw + sx.min(gw - 1)];
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
            surface.generation = surface.generation.wrapping_add(1);
            Ok(())
        })
    })
}


