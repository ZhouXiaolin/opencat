use crate::{
    display::list::DisplayItem,
    runtime::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StableNodeReuse {
    DirectLeaf,
    ItemLeaf,
    SubtreeSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LiveNodeItemExecution {
    Direct,
    FrameLocalPicture,
}

pub(crate) fn analyze_stable_node_reuse(
    display_tree: &AnnotatedDisplayTree,
    handle: AnnotatedNodeHandle,
) -> StableNodeReuse {
    let node = display_tree.node(handle);
    if !node.children.is_empty() {
        return StableNodeReuse::SubtreeSnapshot;
    }

    match &node.item {
        DisplayItem::Rect(_) => StableNodeReuse::DirectLeaf,
        DisplayItem::Text(_) | DisplayItem::Bitmap(_) | DisplayItem::Lucide(_) => {
            StableNodeReuse::ItemLeaf
        }
        DisplayItem::DrawScript(_) => StableNodeReuse::DirectLeaf,
    }
}

pub(crate) fn analyze_live_node_item_execution(
    display_tree: &AnnotatedDisplayTree,
    handle: AnnotatedNodeHandle,
) -> LiveNodeItemExecution {
    match &display_tree.node(handle).item {
        DisplayItem::DrawScript(_) => LiveNodeItemExecution::FrameLocalPicture,
        DisplayItem::Rect(_)
        | DisplayItem::Text(_)
        | DisplayItem::Bitmap(_)
        | DisplayItem::Lucide(_) => LiveNodeItemExecution::Direct,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LiveNodeItemExecution, StableNodeReuse, analyze_live_node_item_execution,
        analyze_stable_node_reuse,
    };
    use crate::{
        display::list::{
            DisplayItem, DisplayRect, DisplayTransform, DrawScriptDisplayItem, RectDisplayItem,
            RectPaintStyle, TextDisplayItem,
        },
        runtime::{
            analysis::{
                DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
                DisplayNodeInvalidation,
            },
            annotation::{AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey},
            fingerprint::{PaintVariance, SubtreeSnapshotFingerprint},
        },
        style::{BorderRadius, ComputedTextStyle},
    };

    fn rect_bounds() -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    fn node(item: DisplayItem, children: Vec<AnnotatedNodeHandle>) -> AnnotatedDisplayNode {
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
            item,
            children,
        }
    }

    fn tree(nodes: Vec<AnnotatedDisplayNode>, analysis: DisplayAnalysisTable) -> AnnotatedDisplayTree {
        let len = nodes.len();
        AnnotatedDisplayTree {
            root: AnnotatedNodeHandle(0),
            nodes,
            keys: (0..len).map(|i| RenderNodeKey(i as u64 + 1)).collect(),
            layer_bounds: vec![rect_bounds(); len],
            analysis,
            invalidation: {
                let mut table = DisplayInvalidationTable::with_len(len);
                for i in 0..len {
                    table.insert(
                        AnnotatedNodeHandle(i),
                        DisplayNodeInvalidation {
                            composite_dirty: false,
                        },
                    );
                }
                table
            },
        }
    }

    #[test]
    fn rect_leaf_prefers_direct_leaf_reuse() {
        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_variance: PaintVariance::Stable,
                subtree_contains_time_variant: false,
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 2, secondary: 2 }),
            },
        );
        let display_tree = tree(
            vec![node(
                DisplayItem::Rect(RectDisplayItem {
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
                Vec::new(),
            )],
            analysis,
        );

        assert_eq!(
            analyze_stable_node_reuse(&display_tree, AnnotatedNodeHandle(0)),
            StableNodeReuse::DirectLeaf
        );
    }

    #[test]
    fn text_leaf_prefers_item_leaf_reuse() {
        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_variance: PaintVariance::Stable,
                subtree_contains_time_variant: false,
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint { primary: 2, secondary: 2 }),
            },
        );
        let display_tree = tree(
            vec![node(
                DisplayItem::Text(TextDisplayItem {
                    bounds: rect_bounds(),
                    text: "Hello".into(),
                    style: ComputedTextStyle::default(),
                    allow_wrap: false,
                    drop_shadow: None,
                }),
                Vec::new(),
            )],
            analysis,
        );

        assert_eq!(
            analyze_stable_node_reuse(&display_tree, AnnotatedNodeHandle(0)),
            StableNodeReuse::ItemLeaf
        );
    }

    #[test]
    fn draw_script_prefers_frame_local_picture_execution() {
        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_variance: PaintVariance::TimeVariant,
                subtree_contains_time_variant: true,
                paint_fingerprint: None,
                snapshot_fingerprint: None,
            },
        );
        let display_tree = tree(
            vec![node(
                DisplayItem::DrawScript(DrawScriptDisplayItem {
                    bounds: rect_bounds(),
                    commands: Vec::new(),
                    drop_shadow: None,
                }),
                Vec::new(),
            )],
            analysis,
        );

        assert_eq!(
            analyze_live_node_item_execution(&display_tree, AnnotatedNodeHandle(0)),
            LiveNodeItemExecution::FrameLocalPicture
        );
    }
}
