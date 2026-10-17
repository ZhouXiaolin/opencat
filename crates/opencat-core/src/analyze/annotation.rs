use crate::{
    analyze::{DisplayAnalysisTable, DisplayNodeAnalysis, fingerprint},
    display::{
        list::{DisplayClip, DisplayItem, DisplayRect, DisplayTransform, DrawScriptDisplayItem},
        tree::{
            DisplayNode, DisplayRecordedSubtreeFingerprint, DisplayTree, HiddenChildDisplayNode,
        },
    },
    layout::tree::LayoutOutputFingerprint,
    semantic::fingerprint::ElementInputFingerprints,
    style::CssFilter,
    style::BorderRadius,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderNodeKey(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub struct AnnotatedNodeHandle(pub usize);

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayTree {
    pub root: AnnotatedNodeHandle,
    pub nodes: Vec<AnnotatedDisplayNode>,
    pub keys: Vec<RenderNodeKey>,
    pub layer_bounds: Vec<DisplayRect>,
    pub analysis: DisplayAnalysisTable,
}

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayNode {
    pub input_fingerprints: ElementInputFingerprints,
    pub layout_output_fingerprint: LayoutOutputFingerprint,
    pub recorded_subtree_fingerprint: DisplayRecordedSubtreeFingerprint,
    pub transform: DisplayTransform,
    pub opacity: f32,
    pub css_filter: CssFilter,
    pub backdrop_blur_sigma: Option<f32>,
    pub backdrop_filter: CssFilter,
    pub paint_clip: Option<DisplayClip>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<AnnotatedNodeHandle>,
    pub draw_slot: Option<DrawScriptDisplayItem>,
    pub hidden_subtree: Vec<HiddenChildDisplayNode>,
}

#[derive(Clone, Copy, Debug)]
pub struct RecordedNodeSemantics<'a> {
    pub layout_output_fingerprint: LayoutOutputFingerprint,
    pub item: &'a DisplayItem,
    pub paint_clip: Option<&'a DisplayClip>,
    pub clip: Option<&'a DisplayClip>,
}

#[derive(Clone, Debug)]
pub struct DrawCompositeSemantics<'a> {
    pub transform: &'a DisplayTransform,
    pub opacity: f32,
    pub css_filter: &'a CssFilter,
    pub backdrop_blur_sigma: Option<f32>,
    /// Non-blur ops of CSS `backdrop-filter` (blur rides `backdrop_blur_sigma`).
    pub backdrop_filter: &'a CssFilter,
    /// The node's own border-box radius — CSS clips a backdrop-filter to the
    /// rounded border box, so the filtered backdrop needs it at draw time.
    pub backdrop_border_radius: BorderRadius,
}

impl AnnotatedDisplayTree {
    pub fn new(
        root: AnnotatedNodeHandle,
        nodes: Vec<AnnotatedDisplayNode>,
        keys: Vec<RenderNodeKey>,
        layer_bounds: Vec<DisplayRect>,
        analysis: DisplayAnalysisTable,
    ) -> Self {
        Self {
            root,
            nodes,
            keys,
            layer_bounds,
            analysis,
        }
    }

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

    pub fn layer_bounds(&self, handle: AnnotatedNodeHandle) -> DisplayRect {
        self.layer_bounds[handle.0]
    }

}

impl AnnotatedDisplayNode {
    pub fn recorded_semantics(&self) -> RecordedNodeSemantics<'_> {
        RecordedNodeSemantics {
            layout_output_fingerprint: self.layout_output_fingerprint,
            item: &self.item,
            paint_clip: self.paint_clip.as_ref(),
            clip: self.clip.as_ref(),
        }
    }

    pub fn draw_composite_semantics(&self) -> DrawCompositeSemantics<'_> {
        let backdrop_border_radius = match &self.item {
            DisplayItem::Rect(rect) => rect.paint.border_radius,
            _ => BorderRadius::default(),
        };
        DrawCompositeSemantics {
            transform: &self.transform,
            opacity: self.opacity,
            css_filter: &self.css_filter,
            backdrop_blur_sigma: self.backdrop_blur_sigma,
            backdrop_filter: &self.backdrop_filter,
            backdrop_border_radius,
        }
    }
}

pub fn annotate_display_tree(display_tree: &DisplayTree) -> AnnotatedDisplayTree {
    let node_count = count_display_nodes(&display_tree.root);
    let mut nodes = Vec::with_capacity(node_count);
    let mut keys = Vec::with_capacity(node_count);
    let mut layer_bounds = Vec::with_capacity(node_count);
    let mut analysis = DisplayAnalysisTable::with_capacity(node_count);
    let root = annotate_display_node(
        &display_tree.root,
        &mut nodes,
        &mut keys,
        &mut layer_bounds,
        &mut analysis,
    );

    AnnotatedDisplayTree::new(root, nodes, keys, layer_bounds, analysis)
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
) -> AnnotatedNodeHandle {
    let mut children = Vec::with_capacity(node.children.len());
    for child in &node.children {
        children.push(annotate_display_node(
            child,
            nodes,
            keys,
            layer_bounds,
            analysis,
        ));
    }

    let render_key = RenderNodeKey(node.element_id.0);
    let handle = AnnotatedNodeHandle(nodes.len());
    let annotated = AnnotatedDisplayNode {
        input_fingerprints: node.input_fingerprints,
        layout_output_fingerprint: node.layout_output_fingerprint,
        recorded_subtree_fingerprint: node.recorded_subtree_fingerprint,
        transform: node.transform.clone(),
        opacity: node.opacity,
        css_filter: node.css_filter.clone(),
        backdrop_blur_sigma: node.backdrop_blur_sigma,
        backdrop_filter: node.backdrop_filter.clone(),
        paint_clip: node.paint_clip.clone(),
        clip: node.clip.clone(),
        item: node.item.clone(),
        children,
        draw_slot: node.draw_slot.clone(),
        hidden_subtree: node.hidden_subtree.clone(),
    };

    let node_analysis = DisplayNodeAnalysis {
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
    // CSS filter 的绘制区域是 filter region：`drop-shadow()` 的阴影落在
    // 元素框之外，合成层的 bounds 必须扩到能容纳它，否则阴影被裁掉。
    for op in &annotated.css_filter.ops {
        if let Some(shadow) = op.shadow {
            let (left, top, right, bottom) = shadow.outsets();
            node_layer_bounds = node_layer_bounds.outset(left, top, right, bottom);
        }
    }

    keys.push(render_key);
    nodes.push(annotated);
    layer_bounds.push(node_layer_bounds);
    analysis.insert(handle, node_analysis);

    handle
}

/// 自底向上填充 fingerprint。
pub fn compute_display_tree_fingerprints(tree: &mut AnnotatedDisplayTree) {
    compute_node_fingerprint(tree.root, tree);
}

fn compute_node_fingerprint(handle: AnnotatedNodeHandle, tree: &mut AnnotatedDisplayTree) {
    let child_count = tree.node(handle).children.len();
    for i in 0..child_count {
        let child = tree.node(handle).children[i];
        compute_node_fingerprint(child, tree);
    }

    let node = tree.node(handle);
    let paint_fp = fingerprint::annotated_subtree_paint_fingerprint(node, &tree.analysis);
    let snapshot_fp = fingerprint::annotated_subtree_snapshot_fingerprint(node, &tree.analysis);
    tree.analysis.insert(
        handle,
        DisplayNodeAnalysis {
            paint_fingerprint: paint_fp,
            snapshot_fingerprint: snapshot_fp,
        },
    );
}

