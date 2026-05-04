//! Host 侧 [`PathBoundsComputer`] 实现：使用 skia 解析 SVG path data 取真实包围盒。
//!
//! 当 `host-backend-skia` feature 关闭时，回退到 core 的 [`DefaultPathBounds`]。

use anyhow::Result;

use crate::core::scene::path_bounds::{DefaultPathBounds, PathBoundsComputer};

#[cfg(feature = "host-backend-skia")]
pub struct SkiaPathBounds;

#[cfg(feature = "host-backend-skia")]
impl PathBoundsComputer for SkiaPathBounds {
    fn compute_view_box(&self, path_data: &[String]) -> Result<[f32; 4]> {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut has_any = false;

        for data in path_data {
            let path = skia_safe::Path::from_svg(data)
                .ok_or_else(|| anyhow::anyhow!("invalid SVG path data"))?;
            let bounds = path.bounds();
            min_x = min_x.min(bounds.left());
            min_y = min_y.min(bounds.top());
            max_x = max_x.max(bounds.right());
            max_y = max_y.max(bounds.bottom());
            has_any = true;
        }

        if !has_any {
            return Ok([0.0, 0.0, 24.0, 24.0]);
        }

        let w = (max_x - min_x).max(1.0);
        let h = (max_y - min_y).max(1.0);
        Ok([min_x, min_y, w, h])
    }
}

/// 返回 host 默认的 path bounds 计算器。
#[cfg(feature = "host-backend-skia")]
pub fn default_host_path_bounds() -> &'static dyn PathBoundsComputer {
    &SkiaPathBounds
}

#[cfg(not(feature = "host-backend-skia"))]
pub fn default_host_path_bounds() -> &'static dyn PathBoundsComputer {
    &DefaultPathBounds
}
