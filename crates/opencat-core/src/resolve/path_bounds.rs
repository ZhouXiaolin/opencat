//! Core 唯一、确定性的 SVG path view-box / bounds 语义。
//!
//! 使用 kurbo 解析 SVG path data 并计算并集包围盒；engine / web 不再提供
//! 可替换的 bounds 算法。返回值语义为 `[min_x, min_y, width, height]`。
//!
//! 明确行为：
//! - 无路径（空切片或全部为空字符串）→ [`EMPTY_PATH_VIEW_BOX`]
//! - 非法 path data → `Err`
//! - 多 path → 并集包围盒

use anyhow::{Result, anyhow};

use kurbo::Shape;

/// 无路径时的确定性 fallback view box。
pub const EMPTY_PATH_VIEW_BOX: [f32; 4] = [0.0, 0.0, 100.0, 100.0];

/// 计算给定 SVG path data 的并集包围盒，返回 `[min_x, min_y, width, height]`。
///
/// 空输入返回固定 fallback [`EMPTY_PATH_VIEW_BOX`]。
/// 任一 path 解析失败则整体失败。
pub fn compute_view_box(path_data: &[String]) -> Result<[f32; 4]> {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut has_geometry = false;

    for data in path_data {
        if data.trim().is_empty() {
            continue;
        }
        let path =
            kurbo::BezPath::from_svg(data).map_err(|e| anyhow!("invalid SVG path: {}", e))?;
        let bbox = path.bounding_box();
        if !bbox.x0.is_finite()
            || !bbox.y0.is_finite()
            || !bbox.x1.is_finite()
            || !bbox.y1.is_finite()
        {
            continue;
        }
        min_x = min_x.min(bbox.x0);
        min_y = min_y.min(bbox.y0);
        max_x = max_x.max(bbox.x1);
        max_y = max_y.max(bbox.y1);
        has_geometry = true;
    }

    if !has_geometry {
        return Ok(EMPTY_PATH_VIEW_BOX);
    }

    Ok([
        min_x as f32,
        min_y as f32,
        (max_x - min_x) as f32,
        (max_y - min_y) as f32,
    ])
}


