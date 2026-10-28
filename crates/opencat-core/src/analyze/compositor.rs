use crate::analyze::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle};
use crate::display::list::DisplayItem;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderedSceneProgram {
    pub root: OrderedSceneOp,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OrderedSubtreeProgram {
    pub handle: AnnotatedNodeHandle,
    pub children: Vec<OrderedSceneOp>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum OrderedSceneOp {
    LiveSubtree {
        handle: AnnotatedNodeHandle,
        children: Vec<OrderedSceneOp>,
    },
    ReusedSubtree {
        handle: AnnotatedNodeHandle,
    },
}

impl OrderedSceneOp {
    pub fn handle(&self) -> AnnotatedNodeHandle {
        match self {
            OrderedSceneOp::LiveSubtree { handle, .. } => *handle,
            OrderedSceneOp::ReusedSubtree { handle } => *handle,
        }
    }
}

impl OrderedSceneProgram {
    pub fn build(display_tree: &AnnotatedDisplayTree) -> Self {
        Self {
            root: build_scene_op(display_tree, display_tree.root),
        }
    }

    pub fn build_subtree(
        display_tree: &AnnotatedDisplayTree,
        handle: AnnotatedNodeHandle,
    ) -> OrderedSubtreeProgram {
        OrderedSubtreeProgram {
            handle,
            children: display_tree
                .children(handle)
                .iter()
                .map(|&child_handle| build_scene_op(display_tree, child_handle))
                .collect(),
        }
    }
}

const SUBTREE_GRANULARITY_RATIO_THRESHOLD: f32 = 16.0;

fn layer_area(bounds: crate::display::list::DisplayRect) -> f32 {
    bounds.width.max(1.0) * bounds.height.max(1.0)
}

fn should_cache_subtree_at_parent_granularity(
    display_tree: &AnnotatedDisplayTree,
    handle: AnnotatedNodeHandle,
) -> bool {
    let children = display_tree.children(handle);
    if children.is_empty() {
        return false;
    }

    let parent_area = layer_area(display_tree.layer_bounds(handle));
    let max_child_area = children
        .iter()
        .map(|&child| layer_area(display_tree.layer_bounds(child)))
        .fold(0.0_f32, f32::max);

    max_child_area <= 0.0 || (parent_area / max_child_area) <= SUBTREE_GRANULARITY_RATIO_THRESHOLD
}

fn build_scene_op(
    display_tree: &AnnotatedDisplayTree,
    handle: AnnotatedNodeHandle,
) -> OrderedSceneOp {
    let analysis = display_tree.analysis(handle);
    if analysis.snapshot_fingerprint.is_some()
        && matches!(
            analyze_stable_node_reuse(display_tree, handle),
            StableNodeReuse::SubtreeSnapshot
        )
        && should_cache_subtree_at_parent_granularity(display_tree, handle)
    {
        return OrderedSceneOp::ReusedSubtree { handle };
    }

    let children = display_tree
        .children(handle)
        .iter()
        .map(|&child_handle| build_scene_op(display_tree, child_handle))
        .collect();

    OrderedSceneOp::LiveSubtree { handle, children }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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

pub fn analyze_stable_node_reuse(
    display_tree: &AnnotatedDisplayTree,
    handle: AnnotatedNodeHandle,
) -> StableNodeReuse {
    let node = display_tree.node(handle);
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
        DisplayItem::Bitmap(_) | DisplayItem::Lottie(_) | DisplayItem::SvgPath(_)
        | DisplayItem::Svg(_) => StableNodeReuse::ItemPictureLeaf,
        DisplayItem::DrawScript(_) => StableNodeReuse::DirectLeaf,
    }
}


