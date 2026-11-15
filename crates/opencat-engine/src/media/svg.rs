//! SVG 位图资产光栅化。

use opencat_core::svg_asset::parse_img;

/// SVG 字节 → 预乘 RGBA8888（stride = w*4）。
///
/// `target` 是绘制目标尺寸；None 用 intrinsic 尺寸。
pub fn rasterize_svg(bytes: &[u8], target: Option<(u32, u32)>) -> Option<(Vec<u8>, u32, u32)> {
    let tree = parse_img(bytes)?;
    let size = tree.size();
    let (tw, th) = (size.width(), size.height());
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
