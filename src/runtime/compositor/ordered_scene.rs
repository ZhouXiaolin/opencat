use crate::runtime::{
    annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle},
    compositor::{
        LiveNodeItemExecution,
        reuse::{StableNodeReuse, analyze_live_node_item_execution, analyze_stable_node_reuse},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OrderedSceneProgram {
    pub root: OrderedSceneOp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OrderedSceneOp {
    LiveSubtree {
        handle: AnnotatedNodeHandle,
        item_execution: LiveNodeItemExecution,
        children: Vec<OrderedSceneOp>,
    },
    CachedSubtree {
        handle: AnnotatedNodeHandle,
    },
}

impl OrderedSceneProgram {
    pub(crate) fn build(display_tree: &AnnotatedDisplayTree) -> Self {
        Self {
            root: build_scene_op(display_tree, display_tree.root),
        }
    }
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
    {
        return OrderedSceneOp::CachedSubtree { handle };
    }

    let children = display_tree
        .children(handle)
        .iter()
        .map(|&child_handle| build_scene_op(display_tree, child_handle))
        .collect();

    OrderedSceneOp::LiveSubtree {
        handle,
        item_execution: analyze_live_node_item_execution(display_tree, handle),
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::{OrderedSceneOp, OrderedSceneProgram};
    use crate::{
        display::list::{
            DisplayItem, DisplayRect, DisplayTransform, DrawScriptDisplayItem, RectDisplayItem,
            RectPaintStyle,
        },
        runtime::{
            analysis::{
                DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
                DisplayNodeInvalidation,
            },
            annotation::{AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey},
            compositor::LiveNodeItemExecution,
            fingerprint::{PaintVariance, SubtreeSnapshotFingerprint},
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

    fn rect_node(children: Vec<AnnotatedNodeHandle>) -> AnnotatedDisplayNode {
        AnnotatedDisplayNode {
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
                    border_color: None,
                    blur_sigma: None,
                    box_shadow: None,
                    inset_shadow: None,
                    drop_shadow: None,
                },
            }),
            children,
        }
    }

    #[test]
    fn stable_leaf_rect_stays_live_even_when_composite_dirty() {
        let root = AnnotatedNodeHandle(0);
        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![rect_node(Vec::new())],
            keys: vec![RenderNodeKey(1)],
            layer_bounds: vec![rect_bounds()],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: false,
                        paint_fingerprint: Some(11),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 22, secondary: 22 }),
                    },
                );
                table
            },
            invalidation: {
                let mut table = DisplayInvalidationTable::with_len(1);
                table.insert(
                    root,
                    DisplayNodeInvalidation {
                        composite_dirty: true,
                    },
                );
                table
            },
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                item_execution: LiveNodeItemExecution::Direct,
                children: Vec::new(),
            }
        );
    }

    #[test]
    fn stable_non_leaf_subtree_builds_as_cached_subtree() {
        let root = AnnotatedNodeHandle(0);
        let child = AnnotatedNodeHandle(1);
        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![rect_node(vec![child]), rect_node(Vec::new())],
            keys: vec![RenderNodeKey(1), RenderNodeKey(2)],
            layer_bounds: vec![rect_bounds(), rect_bounds()],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: false,
                        paint_fingerprint: Some(11),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 22, secondary: 22 }),
                    },
                );
                table.insert(
                    child,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: false,
                        paint_fingerprint: Some(33),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 44, secondary: 44 }),
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(2),
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(program.root, OrderedSceneOp::CachedSubtree { handle: root });
    }

    #[test]
    fn time_variant_scene_stays_live_in_order() {
        let root = AnnotatedNodeHandle(0);
        let static_child = AnnotatedNodeHandle(1);
        let dynamic_child = AnnotatedNodeHandle(2);

        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                rect_node(vec![static_child, dynamic_child]),
                rect_node(Vec::new()),
                AnnotatedDisplayNode {
                    transform: DisplayTransform {
                        translation_x: 0.0,
                        translation_y: 0.0,
                        bounds: rect_bounds(),
                        transforms: Vec::new(),
                    },
                    opacity: 1.0,
                    backdrop_blur_sigma: None,
                    clip: None,
                    item: DisplayItem::DrawScript(DrawScriptDisplayItem {
                        bounds: rect_bounds(),
                        commands: Vec::new(),
                        drop_shadow: None,
                    }),
                    children: Vec::new(),
                },
            ],
            keys: vec![RenderNodeKey(1), RenderNodeKey(2), RenderNodeKey(3)],
            layer_bounds: vec![rect_bounds(), rect_bounds(), rect_bounds()],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table.insert(
                    static_child,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: false,
                        paint_fingerprint: Some(12),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 13, secondary: 13 }),
                    },
                );
                table.insert(
                    dynamic_child,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::TimeVariant,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(3),
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                item_execution: LiveNodeItemExecution::Direct,
                children: vec![
                    OrderedSceneOp::LiveSubtree {
                        handle: static_child,
                        item_execution: LiveNodeItemExecution::Direct,
                        children: Vec::new(),
                    },
                    OrderedSceneOp::LiveSubtree {
                        handle: dynamic_child,
                        item_execution: LiveNodeItemExecution::FrameLocalPicture,
                        children: Vec::new(),
                    },
                ],
            }
        );
    }

    #[test]
    fn live_parent_still_reuses_stable_non_leaf_child() {
        let root = AnnotatedNodeHandle(0);
        let stable_child = AnnotatedNodeHandle(1);
        let stable_grandchild = AnnotatedNodeHandle(2);
        let dynamic_child = AnnotatedNodeHandle(3);

        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                rect_node(vec![stable_child, dynamic_child]),
                rect_node(vec![stable_grandchild]),
                rect_node(Vec::new()),
                AnnotatedDisplayNode {
                    transform: DisplayTransform {
                        translation_x: 0.0,
                        translation_y: 0.0,
                        bounds: rect_bounds(),
                        transforms: Vec::new(),
                    },
                    opacity: 1.0,
                    backdrop_blur_sigma: None,
                    clip: None,
                    item: DisplayItem::DrawScript(DrawScriptDisplayItem {
                        bounds: rect_bounds(),
                        commands: Vec::new(),
                        drop_shadow: None,
                    }),
                    children: Vec::new(),
                },
            ],
            keys: vec![
                RenderNodeKey(1),
                RenderNodeKey(2),
                RenderNodeKey(3),
                RenderNodeKey(4),
            ],
            layer_bounds: vec![
                rect_bounds(),
                rect_bounds(),
                rect_bounds(),
                rect_bounds(),
            ],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table.insert(
                    stable_child,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: false,
                        paint_fingerprint: Some(12),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 13, secondary: 13 }),
                    },
                );
                table.insert(
                    stable_grandchild,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::Stable,
                        subtree_contains_time_variant: false,
                        paint_fingerprint: Some(14),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 15, secondary: 15 }),
                    },
                );
                table.insert(
                    dynamic_child,
                    DisplayNodeAnalysis {
                        paint_variance: PaintVariance::TimeVariant,
                        subtree_contains_time_variant: true,
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(4),
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                item_execution: LiveNodeItemExecution::Direct,
                children: vec![
                    OrderedSceneOp::CachedSubtree {
                        handle: stable_child,
                    },
                    OrderedSceneOp::LiveSubtree {
                        handle: dynamic_child,
                        item_execution: LiveNodeItemExecution::FrameLocalPicture,
                        children: Vec::new(),
                    },
                ],
            }
        );
    }
}
