//! 平台无关的 SVG 路径包围盒计算 trait。
//!
//! Core 内部不依赖任何具体的 2D 库，仅描述行为。
//! 默认实现 [`DefaultPathBounds`] 返回固定 view_box，
//! Skia 等真实实现由 host 侧注入。

use anyhow::Result;

pub trait PathBoundsComputer: Send + Sync {
    /// 计算给定 SVG path data 的并集包围盒，返回 `[min_x, min_y, width, height]`。
    fn compute_view_box(&self, path_data: &[String]) -> Result<[f32; 4]>;
}

/// 无外部依赖的占位实现：始终返回 `[0, 0, 100, 100]`。
pub struct DefaultPathBounds;

impl PathBoundsComputer for DefaultPathBounds {
    fn compute_view_box(&self, _path_data: &[String]) -> Result<[f32; 4]> {
        Ok([0.0, 0.0, 100.0, 100.0])
    }
}
