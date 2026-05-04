use crate::{
    core::display::{
        list::{DisplayClip, DisplayItem, DisplayRect, DisplayTransform},
        tree::{DisplayNode, DisplayTree},
    },
    core::runtime::{
        analysis::{
            DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
            DisplayNodeInvalidation,
        },
        fingerprint::{self, PaintVariance},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderNodeKey(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnnotatedNodeHandle(pub usize);

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayTree {
    pub root: AnnotatedNodeHandle,
    pub nodes: Vec<AnnotatedDisplayNode>,
    pub keys: Vec<RenderNodeKey>,
    pub layer_bounds: Vec<DisplayRect>,
    pub analysis: DisplayAnalysisTable,
    pub invalidation: DisplayInvalidationTable,
}

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayNode {
    pub transform: DisplayTransform,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<AnnotatedNodeHandle>,
}

#[derive(Clone, Copy, Debug)]
pub struct RecordedNodeSemantics<'a> {
    pub bounds: DisplayRect,
    pub item: &'a DisplayItem,
    pub clip: Option<&'a DisplayClip>,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawCompositeSemantics<'a> {
    pub transform: &'a DisplayTransform,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
}

impl AnnotatedDisplayTree {
    pub fn root_node(&self) -> &AnnotatedDisplayNode {
        self.node(self.root)
    }

    pub fn node(&self, handle: AnnotatedNodeHandle) -> &AnnotatedDisplayNode {
        &self.nodes[handle.0]
    }

    pub fn children(&self, handle: AnnotatedNodeHandle) -> &[AnnotatedNodeHandle] {
        &self.node(handle).children
    }

    pub fn analysis(&self, handle: AnnotatedNodeHandle) -> DisplayNodeAnalysis {
        self.analysis.require(handle)
    }

    pub fn key(&self, handle: AnnotatedNodeHandle) -> RenderNodeKey {
        self.keys[handle.0]
    }

    pub fn contains_time_variant(&self) -> bool {
        self.analysis(self.root).subtree_contains_time_variant
    }

    pub fn layer_bounds(&self, handle: AnnotatedNodeHandle) -> DisplayRect {
        self.layer_bounds[handle.0]
    }
}

impl AnnotatedDisplayNode {
    pub fn recorded_semantics(&self) -> RecordedNodeSemantics<'_> {
        RecordedNodeSemantics {
            bounds: self.transform.bounds,
            item: &self.item,
            clip: self.clip.as_ref(),
        }
    }

    pub fn draw_composite_semantics(&self) -> DrawCompositeSemantics<'_> {
        DrawCompositeSemantics {
            transform: &self.transform,
            opacity: self.opacity,
            backdrop_blur_sigma: self.backdrop_blur_sigma,
        }
    }
}

pub(crate) fn annotate_display_tree(
    display_tree: &DisplayTree,
) -> AnnotatedDisplayTree {
    let node_count = count_display_nodes(&display_tree.root);
    let mut nodes = Vec::with_capacity(node_count);
    let mut keys = Vec::with_capacity(node_count);
    let mut layer_bounds = Vec::with_capacity(node_count);
    let mut analysis = DisplayAnalysisTable::with_capacity(node_count);
    let mut invalidation = DisplayInvalidationTable::with_capacity(node_count);
    let root = annotate_display_node(
        &display_tree.root,
        &mut nodes,
        &mut keys,
        &mut layer_bounds,
        &mut analysis,
        &mut invalidation,
    );

    AnnotatedDisplayTree {
        root,
        nodes,
        keys,
        layer_bounds,
        analysis,
        invalidation,
    }
}

fn count_display_nodes(node: &DisplayNode) -> usize {
    1 + node.children.iter().map(count_display_nodes).sum::<usize>()
}

fn annotate_display_node(
    node: &DisplayNode,
    nodes: &mut Vec<AnnotatedDisplayNode>,
    keys: &mut Vec<RenderNodeKey>,
    layer_bounds: &mut Vec<DisplayRect>,
    analysis: &mut DisplayAnalysisTable,
    invalidation: &mut DisplayInvalidationTable,
) -> AnnotatedNodeHandle {
    let mut children = Vec::with_capacity(node.children.len());
    for child in &node.children {
        children.push(annotate_display_node(
            child,
            nodes,
            keys,
            layer_bounds,
            analysis,
            invalidation,
        ));
    }

    let render_key = RenderNodeKey(node.element_id.0);
    let handle = AnnotatedNodeHandle(nodes.len());
    let annotated = AnnotatedDisplayNode {
        transform: node.transform.clone(),
        opacity: node.opacity,
        backdrop_blur_sigma: node.backdrop_blur_sigma,
        clip: node.clip.clone(),
        item: node.item.clone(),
        children,
    };

    let paint_variance = fingerprint::classify_paint(&node.item);
    let subtree_contains_time_variant = matches!(paint_variance, PaintVariance::TimeVariant)
        || annotated
            .children
            .iter()
            .any(|&child_handle| analysis.require(child_handle).subtree_contains_time_variant);

    let node_analysis = DisplayNodeAnalysis {
        paint_variance,
        subtree_contains_time_variant,
        paint_fingerprint: None,
        snapshot_fingerprint: None,
    };

    let mut node_layer_bounds = annotated.item.visual_bounds();
    for &child_handle in &annotated.children {
        let child = &nodes[child_handle.0];
        let child_bounds = layer_bounds[child_handle.0]
            .translate(child.transform.translation_x, child.transform.translation_y);
        node_layer_bounds = node_layer_bounds.union(child_bounds);
    }

    keys.push(render_key);
    nodes.push(annotated);
    layer_bounds.push(node_layer_bounds);
    analysis.insert(handle, node_analysis);
    invalidation.insert(
        handle,
        DisplayNodeInvalidation {
            composite_dirty: false,
        },
    );

    handle
}

/// 在 `mark_display_tree_composite_dirty` 之后调用，自底向上填充 fingerprint。
///
/// annotation 阶段只建结构、分类 paint_variance / subtree_contains_time_variant；
/// fingerprint 计算需要读 invalidation 表（由 mark_dirty 写入），
/// 因此必须排在 mark_dirty 之后才能拿到真实的 `composite_dirty` 值。
pub(crate) fn compute_display_tree_fingerprints(tree: &mut AnnotatedDisplayTree) {
    let node_count = tree.nodes.len();
    for handle_idx in 0..node_count {
        let handle = AnnotatedNodeHandle(handle_idx);
        let analysis = tree.analysis.require(handle);
        if analysis.subtree_contains_time_variant {
            continue;
        }

        let node = &tree.nodes[handle_idx];
        let paint_fp = fingerprint::annotated_subtree_paint_fingerprint(
            node,
            &tree.analysis,
            analysis.subtree_contains_time_variant,
        );
        let snapshot_fp = fingerprint::annotated_subtree_snapshot_fingerprint(
            node,
            &tree.nodes,
            &tree.analysis,
            &tree.invalidation,
            analysis.subtree_contains_time_variant,
        );

        tree.analysis.insert(
            handle,
            DisplayNodeAnalysis {
                paint_fingerprint: paint_fp,
                snapshot_fingerprint: snapshot_fp,
                ..analysis
            },
        );
    }
}
