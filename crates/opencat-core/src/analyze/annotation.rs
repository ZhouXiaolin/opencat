use std::collections::HashMap;

use crate::{
    analyze::{
        DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
        DisplayNodeInvalidation, fingerprint,
    },
    display::{
        list::{DisplayClip, DisplayItem, DisplayRect, DisplayTransform, DrawScriptDisplayItem},
        tree::{
            DisplayNode, DisplayRecordedSubtreeFingerprint, DisplayTree, HiddenChildDisplayNode,
        },
    },
    layout::tree::LayoutOutputFingerprint,
    semantic::fingerprint::ElementInputFingerprints,
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
    pub invalidation: DisplayInvalidationTable,
}

#[derive(Clone, Debug)]
pub struct AnnotatedDisplayNode {
    pub input_fingerprints: ElementInputFingerprints,
    pub layout_output_fingerprint: LayoutOutputFingerprint,
    pub recorded_subtree_fingerprint: DisplayRecordedSubtreeFingerprint,
    pub transform: DisplayTransform,
    pub opacity: f32,
    pub backdrop_blur_sigma: Option<f32>,
    pub clip: Option<DisplayClip>,
    pub item: DisplayItem,
    pub children: Vec<AnnotatedNodeHandle>,
    pub draw_slot: Option<DrawScriptDisplayItem>,
    pub hidden_subtree: Vec<HiddenChildDisplayNode>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalyzeFingerprintStats {
    pub merkle_skipped_subtrees: usize,
    pub merkle_skipped_nodes: usize,
    pub recorded_hit_subtrees: usize,
    pub recorded_hit_nodes: usize,
    pub snapshot_eligibility_hit_subtrees: usize,
    pub snapshot_eligibility_hit_nodes: usize,
    pub composite_blocked_subtrees: usize,
    pub composite_blocked_nodes: usize,
}

#[derive(Clone, Copy, Debug)]
struct AnalyzeFingerprintHistoryEntry {
    recorded_subtree_fingerprint: DisplayRecordedSubtreeFingerprint,
    has_dirty_descendant_composite: bool,
    analysis: DisplayNodeAnalysis,
}

#[derive(Default)]
pub struct AnalyzeFingerprintHistory {
    entries: HashMap<RenderNodeKey, AnalyzeFingerprintHistoryEntry>,
}

impl AnalyzeFingerprintHistory {
    fn previous(
        &self,
        structure_rebuild: bool,
    ) -> HashMap<RenderNodeKey, AnalyzeFingerprintHistoryEntry> {
        if structure_rebuild {
            HashMap::new()
        } else {
            self.entries.clone()
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RecordedNodeSemantics<'a> {
    pub layout_output_fingerprint: LayoutOutputFingerprint,
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

    pub fn layer_bounds(&self, handle: AnnotatedNodeHandle) -> DisplayRect {
        self.layer_bounds[handle.0]
    }
}

impl AnnotatedDisplayNode {
    pub fn recorded_semantics(&self) -> RecordedNodeSemantics<'_> {
        RecordedNodeSemantics {
            layout_output_fingerprint: self.layout_output_fingerprint,
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

pub fn annotate_display_tree(display_tree: &DisplayTree) -> AnnotatedDisplayTree {
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
        input_fingerprints: node.input_fingerprints,
        layout_output_fingerprint: node.layout_output_fingerprint,
        recorded_subtree_fingerprint: node.recorded_subtree_fingerprint,
        transform: node.transform.clone(),
        opacity: node.opacity,
        backdrop_blur_sigma: node.backdrop_blur_sigma,
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
/// annotation 阶段只建结构；fingerprint 计算需要读 invalidation 表（由 mark_dirty 写入），
/// 因此必须排在 mark_dirty 之后才能拿到真实的 `composite_dirty` 值。
pub fn compute_display_tree_fingerprints(tree: &mut AnnotatedDisplayTree) {
    let mut history = AnalyzeFingerprintHistory::default();
    compute_display_tree_fingerprints_with_history(tree, &mut history, false);
}

pub fn compute_display_tree_fingerprints_with_history(
    tree: &mut AnnotatedDisplayTree,
    history: &mut AnalyzeFingerprintHistory,
    structure_rebuild: bool,
) -> AnalyzeFingerprintStats {
    let previous = history.previous(structure_rebuild);
    let mut next = HashMap::with_capacity(tree.nodes.len());
    let mut stats = AnalyzeFingerprintStats::default();
    compute_node_fingerprint(tree.root, tree, &previous, &mut next, &mut stats);
    history.entries = next;
    stats
}

fn compute_node_fingerprint(
    handle: AnnotatedNodeHandle,
    tree: &mut AnnotatedDisplayTree,
    previous: &HashMap<RenderNodeKey, AnalyzeFingerprintHistoryEntry>,
    next: &mut HashMap<RenderNodeKey, AnalyzeFingerprintHistoryEntry>,
    stats: &mut AnalyzeFingerprintStats,
) -> usize {
    let key = tree.key(handle);
    let recorded_subtree_fingerprint = tree.node(handle).recorded_subtree_fingerprint;
    let has_dirty_descendant_composite = {
        let node = tree.node(handle);
        fingerprint::subtree_has_dirty_descendant_composite(node, &tree.nodes, &tree.invalidation)
    };

    if let Some(entry) = previous.get(&key)
        && entry.recorded_subtree_fingerprint == recorded_subtree_fingerprint
    {
        if entry.has_dirty_descendant_composite == has_dirty_descendant_composite {
            let skipped_nodes = copy_subtree_analysis_from_history(handle, tree, previous, next);
            stats.recorded_hit_subtrees += 1;
            stats.recorded_hit_nodes += skipped_nodes;
            stats.snapshot_eligibility_hit_subtrees += 1;
            stats.snapshot_eligibility_hit_nodes += skipped_nodes;
            stats.merkle_skipped_subtrees += 1;
            stats.merkle_skipped_nodes += skipped_nodes;
            return skipped_nodes;
        }

        let blocked_nodes = annotated_subtree_node_count(handle, tree);
        stats.recorded_hit_subtrees += 1;
        stats.recorded_hit_nodes += blocked_nodes;
        stats.composite_blocked_subtrees += 1;
        stats.composite_blocked_nodes += blocked_nodes;
    }

    let children = tree.node(handle).children.clone();
    let mut node_count = 1;
    for child in children {
        node_count += compute_node_fingerprint(child, tree, previous, next, stats);
    }

    let node = tree.node(handle);
    let paint_fp = fingerprint::annotated_subtree_paint_fingerprint(node, &tree.analysis);
    let snapshot_fp = fingerprint::annotated_subtree_snapshot_fingerprint(
        node,
        &tree.nodes,
        &tree.analysis,
        &tree.invalidation,
    );
    let analysis = DisplayNodeAnalysis {
        paint_fingerprint: paint_fp,
        snapshot_fingerprint: snapshot_fp,
    };

    tree.analysis.insert(handle, analysis);
    next.insert(
        key,
        AnalyzeFingerprintHistoryEntry {
            recorded_subtree_fingerprint,
            has_dirty_descendant_composite,
            analysis,
        },
    );
    node_count
}

fn annotated_subtree_node_count(handle: AnnotatedNodeHandle, tree: &AnnotatedDisplayTree) -> usize {
    1 + tree
        .node(handle)
        .children
        .iter()
        .copied()
        .map(|child| annotated_subtree_node_count(child, tree))
        .sum::<usize>()
}

fn copy_subtree_analysis_from_history(
    handle: AnnotatedNodeHandle,
    tree: &mut AnnotatedDisplayTree,
    previous: &HashMap<RenderNodeKey, AnalyzeFingerprintHistoryEntry>,
    next: &mut HashMap<RenderNodeKey, AnalyzeFingerprintHistoryEntry>,
) -> usize {
    let key = tree.key(handle);
    if let Some(entry) = previous.get(&key) {
        tree.analysis.insert(handle, entry.analysis);
        next.insert(key, *entry);
    }

    let children = tree.node(handle).children.clone();
    1 + children
        .into_iter()
        .map(|child| copy_subtree_analysis_from_history(child, tree, previous, next))
        .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyze::{DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeInvalidation},
        display::list::{
            DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem, RectPaintStyle,
        },
        style::BorderRadius,
    };

    fn rect_bounds() -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    fn rect_node(
        recorded_subtree_fingerprint: u64,
        children: Vec<AnnotatedNodeHandle>,
    ) -> AnnotatedDisplayNode {
        AnnotatedDisplayNode {
            input_fingerprints: Default::default(),
            layout_output_fingerprint: Default::default(),
            recorded_subtree_fingerprint: DisplayRecordedSubtreeFingerprint(
                recorded_subtree_fingerprint,
            ),
            transform: DisplayTransform {
                translation_x: 0.0,
                translation_y: 0.0,
                bounds: rect_bounds(),
                transforms: Vec::new(),
            },
            opacity: 1.0,
            backdrop_blur_sigma: None,
            clip: None,
            item: DisplayItem::Rect(RectDisplayItem {
                bounds: rect_bounds(),
                paint: RectPaintStyle {
                    background: None,
                    border_radius: BorderRadius::default(),
                    border_width: None,
                    border_top_width: None,
                    border_right_width: None,
                    border_bottom_width: None,
                    border_left_width: None,
                    border_color: None,
                    border_style: None,
                    blur_sigma: None,
                    box_shadow: None,
                    inset_shadow: None,
                    drop_shadow: None,
                    backdrop_blur_sigma: None,
                },
            }),
            children,
            draw_slot: None,
            hidden_subtree: Vec::new(),
        }
    }

    fn two_node_tree(child_composite_dirty: bool) -> AnnotatedDisplayTree {
        let child = AnnotatedNodeHandle(0);
        let root = AnnotatedNodeHandle(1);

        let mut analysis = DisplayAnalysisTable::with_capacity(2);
        analysis.insert(
            child,
            DisplayNodeAnalysis {
                paint_fingerprint: None,
                snapshot_fingerprint: None,
            },
        );
        analysis.insert(
            root,
            DisplayNodeAnalysis {
                paint_fingerprint: None,
                snapshot_fingerprint: None,
            },
        );

        let mut invalidation = DisplayInvalidationTable::with_capacity(2);
        invalidation.insert(
            child,
            DisplayNodeInvalidation {
                composite_dirty: child_composite_dirty,
            },
        );
        invalidation.insert(
            root,
            DisplayNodeInvalidation {
                composite_dirty: false,
            },
        );

        AnnotatedDisplayTree {
            root,
            nodes: vec![rect_node(11, Vec::new()), rect_node(22, vec![child])],
            keys: vec![RenderNodeKey(2), RenderNodeKey(1)],
            layer_bounds: vec![rect_bounds(), rect_bounds()],
            analysis,
            invalidation,
        }
    }

    #[test]
    fn recorded_hit_with_changed_descendant_composite_blocks_parent_skip_only() {
        let mut history = AnalyzeFingerprintHistory::default();
        let mut first = two_node_tree(false);
        compute_display_tree_fingerprints_with_history(&mut first, &mut history, false);

        let mut second = two_node_tree(true);
        let stats =
            compute_display_tree_fingerprints_with_history(&mut second, &mut history, false);

        assert_eq!(stats.recorded_hit_subtrees, 2);
        assert_eq!(stats.recorded_hit_nodes, 3);
        assert_eq!(stats.composite_blocked_subtrees, 1);
        assert_eq!(stats.composite_blocked_nodes, 2);
        assert_eq!(stats.snapshot_eligibility_hit_subtrees, 1);
        assert_eq!(stats.snapshot_eligibility_hit_nodes, 1);
        assert_eq!(stats.merkle_skipped_subtrees, 1);
        assert_eq!(stats.merkle_skipped_nodes, 1);

        assert_ne!(first.analysis(first.root), second.analysis(second.root));
        assert_eq!(
            first.analysis(AnnotatedNodeHandle(0)),
            second.analysis(AnnotatedNodeHandle(0))
        );
    }
}
