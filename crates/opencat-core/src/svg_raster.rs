//! SVG 字节 → 位图（`<img src=*.svg>` 语义）。
//!
//! 浏览器把外部 svg 文件当图片光栅成位图；我们的 `<img>` 静态资产链路
//! （probe 尺寸 → 解码成 Skia Image）是位图语义，所以 svg 在这里落成
//! RGBA。`render::svg` 的 Tree→DrawOp 通道是渲染图层语义（inline svg），
//! 两通道互不借用。

/// svg 字节嗅探：utf-8 且以 `<` 开头（`<?xml` / `<svg`）。位图魔数从来
/// 不以 `<` 开头，误报不存在。
fn svg_text(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    text.trim_start().starts_with('<').then_some(text)
}

/// intrinsic 尺寸（width/height/viewBox 归一后的画布尺寸）。非 svg 字节
/// 或解析失败返回 None（调用方落回位图探测）。
pub fn svg_dims(bytes: &[u8]) -> Option<(u32, u32)> {
    let text = svg_text(bytes)?;
    let tree = crate::render::svg::load_tree_markup(text)?;
    let (w, h) = crate::render::svg::tree_size(&tree);
    (w > 0.0 && h > 0.0).then(|| (w.round() as u32, h.round() as u32))
}

/// 光栅成预乘 RGBA8888（stride = w*4）。`target` 是 draw 目标尺寸
/// （shrink-only 由调用方决定）；None 用 intrinsic 尺寸。
pub fn rasterize_svg(bytes: &[u8], target: Option<(u32, u32)>) -> Option<(Vec<u8>, u32, u32)> {
    let text = svg_text(bytes)?;
    let tree = crate::render::svg::load_tree_markup(text)?;
    let (tw, th) = crate::render::svg::tree_size(&tree);
    if tw <= 0.0 || th <= 0.0 {
        return None;
    }
    let (w, h) = target.unwrap_or((tw.round() as u32, th.round() as u32));
    if w == 0 || h == 0 {
        return None;
    }
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let (fw, fh) = (w as f32, h as f32);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(fw / tw, fh / th),
        &mut pixmap.as_mut(),
    );
    Some((pixmap.take().to_vec(), w, h))
}
