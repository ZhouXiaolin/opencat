use crate::display::list::DisplayItem;
use crate::display::tree::{DisplayNode, DisplayTree};

#[derive(Clone, Debug)]
pub struct OrderedSceneProgram<'a> {
    pub root: OrderedSceneOp<'a>,
}

/// 场景渲染决策树：帧内临时结构，直接借用 [`DisplayNode`]（M4-⑥ 投影
/// 砍层后不再复制节点，也不再携带展平索引）。
#[derive(Clone, Debug)]
pub enum OrderedSceneOp<'a> {
    LiveSubtree {
        node: &'a DisplayNode,
        children: Vec<OrderedSceneOp<'a>>,
    },
    ReusedSubtree {
        node: &'a DisplayNode,
    },
}

impl OrderedSceneOp<'_> {
    pub fn node(&self) -> &DisplayNode {
        match self {
            OrderedSceneOp::LiveSubtree { node, .. } => node,
            OrderedSceneOp::ReusedSubtree { node } => node,
        }
    }
}

impl<'a> OrderedSceneProgram<'a> {
    pub fn build(display_tree: &'a DisplayTree) -> Self {
        Self {
            root: build_scene_op(&display_tree.root),
        }
    }
}

const SUBTREE_GRANULARITY_RATIO_THRESHOLD: f32 = 16.0;

fn layer_area(bounds: crate::display::list::DisplayRect) -> f32 {
    bounds.width.max(1.0) * bounds.height.max(1.0)
}

fn should_cache_subtree_at_parent_granularity(node: &DisplayNode) -> bool {
    if node.children.is_empty() {
        return false;
    }

    let parent_area = layer_area(node.layer_bounds);
    let max_child_area = node
        .children
        .iter()
        .map(|child| layer_area(child.layer_bounds))
        .fold(0.0_f32, f32::max);

    max_child_area <= 0.0 || (parent_area / max_child_area) <= SUBTREE_GRANULARITY_RATIO_THRESHOLD
}

pub fn build_scene_op(node: &DisplayNode) -> OrderedSceneOp<'_> {
    if node.snapshot_fingerprint.is_some()
        && matches!(analyze_stable_node_reuse(node), StableNodeReuse::SubtreeSnapshot)
        && should_cache_subtree_at_parent_granularity(node)
    {
        return OrderedSceneOp::ReusedSubtree { node };
    }

    let children = node.children.iter().map(build_scene_op).collect();

    OrderedSceneOp::LiveSubtree { node, children }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StableNodeReuse {
    /// 足够便宜的叶子，直接重画比 record picture 更划算。
    DirectLeaf,
    /// 稳定的 Bitmap / SVG path 叶子，由 `ItemPictureCache` 跨帧复用。
    ItemPictureLeaf,
    /// 稳定的 Text 叶子，由 `TextSnapshotCache` 跨帧复用。
    TextSnapshotLeaf,
    /// 稳定的非叶子子树，由 `SubtreeSnapshotCache` 跨帧复用。
    SubtreeSnapshot,
}

pub fn analyze_stable_node_reuse(node: &DisplayNode) -> StableNodeReuse {
    if let DisplayItem::Timeline(timeline) = &node.item
        && timeline.transition.is_some()
    {
        return StableNodeReuse::DirectLeaf;
    }

    if !node.children.is_empty() {
        return StableNodeReuse::SubtreeSnapshot;
    }

    match &node.item {
        DisplayItem::Rect(_) => StableNodeReuse::DirectLeaf,
        DisplayItem::Timeline(_) => StableNodeReuse::DirectLeaf,
        DisplayItem::Text(text) => {
            if text.text_unit_overrides.is_some() {
                StableNodeReuse::DirectLeaf
            } else {
                StableNodeReuse::TextSnapshotLeaf
            }
        }
        DisplayItem::Bitmap(_)
        | DisplayItem::Lottie(_)
        | DisplayItem::SvgPath(_)
        | DisplayItem::Svg(_) => StableNodeReuse::ItemPictureLeaf,
        DisplayItem::DrawScript(_) => StableNodeReuse::DirectLeaf,
    }
}
