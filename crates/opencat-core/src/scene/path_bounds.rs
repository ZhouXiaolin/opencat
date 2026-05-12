//! 平台无关的 SVG 路径包围盒计算 trait。
//!
//! Core 内部不依赖任何具体的 2D 库，仅描述行为。
//! 默认实现 [`DefaultPathBounds`] 使用 kurbo 计算实际包围盒。

use anyhow::{Result, anyhow};

use kurbo::Shape;

pub trait PathBoundsComputer: Send + Sync {
    /// 计算给定 SVG path data 的并集包围盒，返回 `[min_x, min_y, width, height]`。
    fn compute_view_box(&self, path_data: &[String]) -> Result<[f32; 4]>;
}

/// 使用 kurbo 计算实际 SVG 路径包围盒。
pub struct DefaultPathBounds;

impl PathBoundsComputer for DefaultPathBounds {
    fn compute_view_box(&self, path_data: &[String]) -> Result<[f32; 4]> {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for data in path_data {
            let path = kurbo::BezPath::from_svg(data)
                .map_err(|e| anyhow!("invalid SVG path: {}", e))?;
            let bbox = path.bounding_box();
            min_x = min_x.min(bbox.x0);
            min_y = min_y.min(bbox.y0);
            max_x = max_x.max(bbox.x1);
            max_y = max_y.max(bbox.y1);
        }

        if min_x > max_x {
            return Ok([0.0, 0.0, 100.0, 100.0]);
        }

        let padding = 0.0f32;
        Ok([
            (min_x as f32) - padding,
            (min_y as f32) - padding,
            ((max_x - min_x) as f32) + padding * 2.0,
            ((max_y - min_y) as f32) + padding * 2.0,
        ])
    }
}
