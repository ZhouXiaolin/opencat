use crate::display::list::{DisplayClip, DisplayItem, DisplayTransform};
use crate::runtime::fingerprint::PaintVariance;

#[derive(Clone, Debug)]
pub struct DisplayTree {
    pub root: DisplayNode,
}

#[derive(Clone, Debug)]
pub struct DisplayNode {
    pub transform: DisplayTransform,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<DisplayNode>,

    /// 本节点自身 paint 的 variance（不看子树）。
    pub paint_variance: PaintVariance,
    /// 本节点或任一后代是否为 TimeVariant。向上传播标志。
    pub subtree_contains_time_variant: bool,
    /// 整棵子树的 paint 指纹，构建期一次性计算。
    /// `None` 表示子树含 TimeVariant，不可缓存。
    pub paint_fingerprint: Option<u64>,
}

impl DisplayNode {
    pub fn layer_bounds(&self) -> crate::display::list::DisplayRect {
        let mut bounds = self.item.visual_bounds();
        for child in &self.children {
            let child_bounds = child
                .layer_bounds()
                .translate(child.transform.translation_x, child.transform.translation_y);
            bounds = bounds.union(child_bounds);
        }
        bounds
    }
}
