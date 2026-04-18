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
    /// 当前节点的 subtree snapshot 指纹。
    ///
    /// 语义是“在当前节点自身 composite 之外，这棵子树录成 picture 后会长什么样”。
    /// 因此：
    /// - 不包含当前节点自己的 translation / opacity / transforms
    /// - 递归包含所有后代的 composite 状态，因为它们会被烘焙进当前节点 picture
    /// - `None` 表示子树含 TimeVariant，不可缓存
    pub snapshot_fingerprint: Option<u64>,
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
