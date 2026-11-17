//! 离屏 RGBA surface 与 canvas 文本度量。
//!
//! core 只保存像素缓冲并提供度量；光栅化与绘制由 engine 完成。

use std::cell::RefCell;
use std::sync::Arc;

use anyhow::{Result, bail};
use hashbrown::HashMap;
use swash::scale::{Render, ScaleContext, Source};
use swash::{FontRef, GlyphId};

use super::font_db_for_script;

struct Surface {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

thread_local! {
    static SURFACES: RefCell<HashMap<String, Surface>> = RefCell::new(HashMap::new());
}

/// surface 单边尺寸上限。
pub const MAX_SURFACE_DIMENSION: u32 = 8192;

/// 创建或重置一张全透明 surface。
pub fn surface_create(id: &str, width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > MAX_SURFACE_DIMENSION || height > MAX_SURFACE_DIMENSION
    {
        bail!("surface `{id}`: unsupported dimensions {width}x{height}");
    }
    let rgba = vec![0u8; width as usize * height as usize * 4];
    SURFACES.with(|s| {
        s.borrow_mut().insert(
            id.to_string(),
            Surface {
                width,
                height,
                rgba,
            },
        );
    });
    Ok(())
}

fn with_surface<R>(id: &str, f: impl FnOnce(&mut Surface) -> Result<R>) -> Result<R> {
    SURFACES.with(|s| {
        let mut map = s.borrow_mut();
        let Some(surface) = map.get_mut(id) else {
            bail!("surface `{id}`: not created");
        };
        f(surface)
    })
}

/// 查询一张 surface 的尺寸。
pub fn surface_dimensions(id: &str) -> Result<(u32, u32)> {
    with_surface(id, |surface| Ok((surface.width, surface.height)))
}

/// 尺寸相同时复用缓冲，否则重建。
pub fn surface_create_or_resize(id: &str, width: u32, height: u32) -> Result<()> {
    match surface_dimensions(id) {
        Ok((w, h)) if w == width && h == height => Ok(()),
        _ => surface_create(id, width, height),
    }
}

/// 读取一个子矩形的 RGBA 字节。
pub fn surface_get_rgba(id: &str, x: f64, y: f64, w: f64, h: f64) -> Result<Vec<u8>> {
    with_surface(id, |surface| {
        if w <= 0.0 || h <= 0.0 {
            bail!("surface `{id}`: empty read rect");
        }
        let iw = surface.width as i64;
        let ih = surface.height as i64;
        let x0 = (x.floor() as i64).max(0);
        let y0 = (y.floor() as i64).max(0);
        let x1 = ((x + w).ceil() as i64).min(iw);
        let y1 = ((y + h).ceil() as i64).min(ih);
        if x1 <= x0 || y1 <= y0 {
            bail!("surface `{id}`: read rect out of bounds");
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

/// 整图写回。
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
            bail!("surface `{id}`: write payload length mismatch");
        }
        surface.rgba = rgba;
        Ok(())
    })
}

/// `measureText` 的 ink-box 结果（CSS px，基线向下）。
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct InkMetrics {
    pub width: f64,
    pub ink_left: f64,
    pub ink_right: f64,
    pub ink_ascent: f64,
    pub ink_descent: f64,
    pub font_ascent: f64,
    pub font_descent: f64,
}

type FaceKey = (usize, u32);

thread_local! {
    static FILE_FACE_DATA: RefCell<HashMap<std::path::PathBuf, Arc<[u8]>>> =
        RefCell::new(HashMap::new());
}

fn with_resolved_font<R>(
    family: &str,
    weight: u32,
    f: impl FnOnce(&FontRef<'_>, FaceKey) -> Result<R>,
) -> Result<R> {
    let db = font_db_for_script();
    let mut families: Vec<fontdb::Family<'_>> = Vec::new();
    for part in family.split(',') {
        let name = part.trim().trim_matches(['"', '\'']);
        if name.is_empty() {
            continue;
        }
        families.push(match crate::style::GenericFamily::parse_css(name) {
            Some(crate::style::GenericFamily::Serif) => fontdb::Family::Serif,
            Some(crate::style::GenericFamily::SansSerif) => fontdb::Family::SansSerif,
            Some(crate::style::GenericFamily::Monospace) => fontdb::Family::Monospace,
            None => fontdb::Family::Name(name),
        });
    }
    if families.is_empty() {
        families.push(fontdb::Family::SansSerif);
    }
    let mapped_weight = families.iter().find_map(|f| match f {
        fontdb::Family::Name(name) => {
            Some(crate::text::chrome_query_weight(&db, name, weight as u16))
        }
        _ => None,
    });
    let query = fontdb::Query {
        families: &families,
        weight: fontdb::Weight(mapped_weight.unwrap_or(weight as u16)),
        style: fontdb::Style::Normal,
        stretch: fontdb::Stretch::Normal,
    };
    let face_id = db
        .query(&query)
        .or_else(|| {
            db.query(&fontdb::Query {
                families: &[fontdb::Family::SansSerif],
                ..query
            })
        })
        .or_else(|| {
            db.query(&fontdb::Query {
                families: &[fontdb::Family::Serif],
                ..query
            })
        });
    let Some(face_id) = face_id else {
        bail!("surface: no font face for family `{family}` weight {weight}");
    };
    if let Some((path, index)) = db.face(face_id).and_then(|face| match &face.source {
        fontdb::Source::File(path) => Some((path.clone(), face.index)),
        _ => None,
    }) {
        let data = FILE_FACE_DATA.with(|cache| -> Result<Arc<[u8]>> {
            if let Some(hit) = cache.borrow().get(&path) {
                return Ok(hit.clone());
            }
            let bytes: Arc<[u8]> = Arc::from(std::fs::read(&path)?);
            cache.borrow_mut().insert(path, bytes.clone());
            Ok(bytes)
        })?;
        let Some(font) = FontRef::from_index(&data, index as usize) else {
            bail!("surface: unreadable font face `{face_id:?}`");
        };
        return f(&font, (data.as_ptr() as usize, index));
    }
    let mut out: Option<Result<R>> = None;
    db.with_face_data(face_id, |data, index| -> Result<()> {
        let Some(font) = FontRef::from_index(data, index as usize) else {
            bail!("surface: unreadable font face `{face_id:?}`");
        };
        out = Some(f(&font, (data.as_ptr() as usize, index)));
        Ok(())
    });
    out.unwrap_or_else(|| bail!("surface: font data missing for family `{family}`"))
}

fn glyph_of(font: &FontRef<'_>, ch: char) -> GlyphId {
    font.charmap().map(ch as u32)
}

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

struct GlyphRaster {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

const GLYPH_RASTER_CACHE_CAP: usize = 4096;

thread_local! {
    static GLYPH_RASTERS: RefCell<HashMap<(FaceKey, u64, GlyphId), Option<Arc<GlyphRaster>>>> =
        RefCell::new(HashMap::new());
}

fn glyph_raster(
    face: FaceKey,
    font: &FontRef<'_>,
    size_px: f64,
    gid: GlyphId,
) -> Option<Arc<GlyphRaster>> {
    let key = (face, size_px.to_bits(), gid);
    let cache_enabled = !crate::ablate::ablated(crate::ablate::Ablation::GlyphRasters);
    if cache_enabled && let Some(hit) = GLYPH_RASTERS.with(|c| c.borrow().get(&key).cloned()) {
        return hit;
    }
    let mut context = ScaleContext::new();
    let mut scaler = context
        .builder(*font)
        .size(size_px as f32)
        .hint(false)
        .build();
    let raster = Render::new(&[Source::Outline])
        .render(&mut scaler, gid)
        .and_then(|image| {
            let p = image.placement;
            if p.width == 0 || p.height == 0 {
                return None;
            }
            Some(Arc::new(GlyphRaster {
                left: p.left,
                top: p.top,
                width: p.width,
                height: p.height,
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

/// `canvas.measureText`：kern-free，含 letterSpacing。
pub fn surface_measure_text(
    text: &str,
    family: &str,
    weight: u32,
    size_px: f64,
    letter_spacing: f64,
) -> Result<InkMetrics> {
    with_resolved_font(family, weight, |font, face| {
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
        let mut ink_l = f64::INFINITY;
        let mut ink_r = f64::NEG_INFINITY;
        let mut ink_t = f64::NEG_INFINITY;
        let mut ink_b = f64::INFINITY;
        for (gid, pen_x) in &placed {
            let Some(raster) = glyph_raster(face, font, size_px, *gid) else {
                continue;
            };
            let left = pen_x + raster.left as f64;
            ink_l = ink_l.min(left);
            ink_r = ink_r.max(left + raster.width as f64);
            ink_t = ink_t.max(raster.top as f64);
            ink_b = ink_b.min(raster.top as f64 - raster.height as f64);
        }
        if !ink_l.is_finite() {
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
            ink_left: -ink_l,
            ink_right: ink_r,
            ink_ascent: ink_t,
            ink_descent: -ink_b,
            font_ascent,
            font_descent,
        })
    })
}
