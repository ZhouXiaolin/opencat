use crate::analyze::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle};
use crate::display::list::DisplayItem;
use crate::layout::LayoutPassStats;

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

#[cfg(test)]
mod ordered_scene_tests {
    use super::{OrderedSceneOp, OrderedSceneProgram};
    use crate::{
        analyze::{
            DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
            DisplayNodeInvalidation,
            annotation::{
                AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey,
            },
            fingerprint::SubtreeSnapshotFingerprint,
        },
        display::list::{
            DisplayItem, DisplayRect, DisplayTransform, DrawScriptDisplayItem, RectDisplayItem,
            RectPaintStyle, TimelineDisplayItem, TimelineTransitionDisplay,
        },
        parse::transition::TransitionKind,
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
            input_fingerprints: Default::default(),
            layout_output_fingerprint: Default::default(),
            recorded_subtree_fingerprint: Default::default(),
            transform: DisplayTransform {
                translation_x: 0.0,
                translation_y: 0.0,
                bounds: rect_bounds(),
                transforms: Vec::new(),
            },
            opacity: 1.0,
            css_filter: Default::default(),
            backdrop_blur_sigma: None,
            paint_clip: None,
            clip: None,
            item: DisplayItem::Rect(RectDisplayItem {
                bounds: rect_bounds(),
                paint: RectPaintStyle {
                    background: Vec::new(),
                    border_radius: BorderRadius::default(),
                    border_width: None,
                    border_top_width: None,
                    border_right_width: None,
                    border_bottom_width: None,
                    border_left_width: None,
                    border_color: None,
                    border_style: None,
                    box_shadow: Vec::new(),
                    inset_shadow: Vec::new(),
                    drop_shadow: Vec::new(),
                    backdrop_blur_sigma: None,
                },
            }),
            children,
            draw_slot: None,
            hidden_subtree: Vec::new(),
        }
    }

    #[test]
    fn stable_leaf_rect_stays_live_even_when_apply_changed() {
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
                        paint_fingerprint: Some(11),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(22)),
                    },
                );
                table
            },
            invalidation: {
                let mut table = DisplayInvalidationTable::with_len(1);
                table.insert(
                    root,
                    DisplayNodeInvalidation {
                        apply_changed: true,
                    },
                );
                table
            },
            analyze_reuse: vec![],
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                children: Vec::new(),
            }
        );
    }

    #[test]
    fn stable_non_leaf_subtree_builds_as_reused_subtree() {
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
                        paint_fingerprint: Some(11),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(22)),
                    },
                );
                table.insert(
                    child,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(33),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(44)),
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(2),
            analyze_reuse: vec![],
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(program.root, OrderedSceneOp::ReusedSubtree { handle: root });
    }

    #[test]
    fn missing_snapshot_scene_stays_live_in_order() {
        let root = AnnotatedNodeHandle(0);
        let static_child = AnnotatedNodeHandle(1);
        let dynamic_child = AnnotatedNodeHandle(2);

        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                rect_node(vec![static_child, dynamic_child]),
                rect_node(Vec::new()),
                AnnotatedDisplayNode {
                    input_fingerprints: Default::default(),
                    layout_output_fingerprint: Default::default(),
                    recorded_subtree_fingerprint: Default::default(),
                    transform: DisplayTransform {
                        translation_x: 0.0,
                        translation_y: 0.0,
                        bounds: rect_bounds(),
                        transforms: Vec::new(),
                    },
                    opacity: 1.0,
                    css_filter: Default::default(),
                    backdrop_blur_sigma: None,
                    paint_clip: None,
                    clip: None,
                    item: DisplayItem::DrawScript(DrawScriptDisplayItem {
                        bounds: rect_bounds(),
                        commands: Vec::new(),
                        drop_shadow: Vec::new(),
                        hidden_subtree: Vec::new(),
                    }),
                    children: Vec::new(),
                    draw_slot: None,
                    hidden_subtree: Vec::new(),
                },
            ],
            keys: vec![RenderNodeKey(1), RenderNodeKey(2), RenderNodeKey(3)],
            layer_bounds: vec![rect_bounds(), rect_bounds(), rect_bounds()],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table.insert(
                    static_child,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(12),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(13)),
                    },
                );
                table.insert(
                    dynamic_child,
                    DisplayNodeAnalysis {
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(3),
            analyze_reuse: vec![],
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                children: vec![
                    OrderedSceneOp::LiveSubtree {
                        handle: static_child,
                        children: Vec::new(),
                    },
                    OrderedSceneOp::LiveSubtree {
                        handle: dynamic_child,
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
                    input_fingerprints: Default::default(),
                    layout_output_fingerprint: Default::default(),
                    recorded_subtree_fingerprint: Default::default(),
                    transform: DisplayTransform {
                        translation_x: 0.0,
                        translation_y: 0.0,
                        bounds: rect_bounds(),
                        transforms: Vec::new(),
                    },
                    opacity: 1.0,
                    css_filter: Default::default(),
                    backdrop_blur_sigma: None,
                    paint_clip: None,
                    clip: None,
                    item: DisplayItem::DrawScript(DrawScriptDisplayItem {
                        bounds: rect_bounds(),
                        commands: Vec::new(),
                        drop_shadow: Vec::new(),
                        hidden_subtree: Vec::new(),
                    }),
                    children: Vec::new(),
                    draw_slot: None,
                    hidden_subtree: Vec::new(),
                },
            ],
            keys: vec![
                RenderNodeKey(1),
                RenderNodeKey(2),
                RenderNodeKey(3),
                RenderNodeKey(4),
            ],
            layer_bounds: vec![rect_bounds(), rect_bounds(), rect_bounds(), rect_bounds()],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table.insert(
                    stable_child,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(12),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(13)),
                    },
                );
                table.insert(
                    stable_grandchild,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(14),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(15)),
                    },
                );
                table.insert(
                    dynamic_child,
                    DisplayNodeAnalysis {
                        paint_fingerprint: None,
                        snapshot_fingerprint: None,
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(4),
            analyze_reuse: vec![],
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                children: vec![
                    OrderedSceneOp::ReusedSubtree {
                        handle: stable_child,
                    },
                    OrderedSceneOp::LiveSubtree {
                        handle: dynamic_child,
                        children: Vec::new(),
                    },
                ],
            }
        );
    }

    #[test]
    fn subtree_build_keeps_root_live_but_reuses_stable_children() {
        let root = AnnotatedNodeHandle(0);
        let stable_child = AnnotatedNodeHandle(1);
        let stable_grandchild = AnnotatedNodeHandle(2);

        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                rect_node(vec![stable_child]),
                rect_node(vec![stable_grandchild]),
                rect_node(Vec::new()),
            ],
            keys: vec![RenderNodeKey(1), RenderNodeKey(2), RenderNodeKey(3)],
            layer_bounds: vec![
                DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                },
                DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 64.0,
                    height: 64.0,
                },
                DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 64.0,
                    height: 64.0,
                },
            ],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(11),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(12)),
                    },
                );
                table.insert(
                    stable_child,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(21),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(22)),
                    },
                );
                table.insert(
                    stable_grandchild,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(31),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(32)),
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(3),
            analyze_reuse: vec![],
        };

        let subtree = OrderedSceneProgram::build_subtree(&tree, root);
        assert_eq!(subtree.handle, root);
        assert_eq!(
            subtree.children,
            vec![OrderedSceneOp::ReusedSubtree {
                handle: stable_child
            }],
        );
    }

    #[test]
    fn large_parent_prefers_child_level_caching() {
        let root = AnnotatedNodeHandle(0);
        let child = AnnotatedNodeHandle(1);
        let grandchild = AnnotatedNodeHandle(2);
        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                rect_node(vec![child]),
                rect_node(vec![grandchild]),
                rect_node(Vec::new()),
            ],
            keys: vec![RenderNodeKey(1), RenderNodeKey(2), RenderNodeKey(3)],
            layer_bounds: vec![
                DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                },
                DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 64.0,
                    height: 64.0,
                },
                DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 64.0,
                    height: 64.0,
                },
            ],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(11),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(12)),
                    },
                );
                table.insert(
                    child,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(21),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(22)),
                    },
                );
                table.insert(
                    grandchild,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(31),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(32)),
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(3),
            analyze_reuse: vec![],
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                children: vec![OrderedSceneOp::ReusedSubtree { handle: child }],
            }
        );
    }

    #[test]
    fn active_transition_timeline_stays_live_for_compositing() {
        let root = AnnotatedNodeHandle(0);
        let from = AnnotatedNodeHandle(1);
        let to = AnnotatedNodeHandle(2);
        let tree = AnnotatedDisplayTree {
            root,
            nodes: vec![
                AnnotatedDisplayNode {
                    input_fingerprints: Default::default(),
                    layout_output_fingerprint: Default::default(),
                    recorded_subtree_fingerprint: Default::default(),
                    transform: DisplayTransform {
                        translation_x: 0.0,
                        translation_y: 0.0,
                        bounds: rect_bounds(),
                        transforms: Vec::new(),
                    },
                    opacity: 1.0,
                    css_filter: Default::default(),
                    backdrop_blur_sigma: None,
                    paint_clip: None,
                    clip: None,
                    item: DisplayItem::Timeline(TimelineDisplayItem {
                        bounds: rect_bounds(),
                        paint: RectPaintStyle {
                            background: Vec::new(),
                            border_radius: BorderRadius::default(),
                            border_width: None,
                            border_top_width: None,
                            border_right_width: None,
                            border_bottom_width: None,
                            border_left_width: None,
                            border_color: None,
                            border_style: None,
                            box_shadow: Vec::new(),
                            inset_shadow: Vec::new(),
                            drop_shadow: Vec::new(),
                            backdrop_blur_sigma: None,
                        },
                        transition: Some(TimelineTransitionDisplay {
                            progress: 0.5,
                            kind: TransitionKind::Fade,
                        }),
                    }),
                    children: vec![from, to],
                    draw_slot: None,
                    hidden_subtree: Vec::new(),
                },
                rect_node(Vec::new()),
                rect_node(Vec::new()),
            ],
            keys: vec![RenderNodeKey(1), RenderNodeKey(2), RenderNodeKey(3)],
            layer_bounds: vec![rect_bounds(), rect_bounds(), rect_bounds()],
            analysis: {
                let mut table = DisplayAnalysisTable::default();
                table.insert(
                    root,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(11),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(12)),
                    },
                );
                table.insert(
                    from,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(21),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(22)),
                    },
                );
                table.insert(
                    to,
                    DisplayNodeAnalysis {
                        paint_fingerprint: Some(31),
                        snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(32)),
                    },
                );
                table
            },
            invalidation: DisplayInvalidationTable::with_len(3),
            analyze_reuse: vec![],
        };

        let program = OrderedSceneProgram::build(&tree);
        assert_eq!(
            program.root,
            OrderedSceneOp::LiveSubtree {
                handle: root,
                children: vec![
                    OrderedSceneOp::LiveSubtree {
                        handle: from,
                        children: Vec::new(),
                    },
                    OrderedSceneOp::LiveSubtree {
                        handle: to,
                        children: Vec::new(),
                    },
                ],
            }
        );
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneRenderPlan {
    pub allows_scene_snapshot_cache: bool,
    pub scene_snapshot_blocked_by_structure: bool,
    pub scene_snapshot_blocked_by_layout: bool,
    pub scene_snapshot_blocked_by_raster: bool,
    pub scene_snapshot_blocked_by_apply_change: bool,
}

impl SceneRenderPlan {
    pub fn from_layout_pass(layout_pass: &LayoutPassStats, apply_changed_nodes: usize) -> Self {
        let scene_snapshot_blocked_by_structure = layout_pass.structure_rebuild;
        let scene_snapshot_blocked_by_layout = layout_pass.layout_dirty_nodes > 0;
        let scene_snapshot_blocked_by_raster = layout_pass.raster_dirty_nodes > 0;
        let scene_snapshot_blocked_by_apply_change = apply_changed_nodes > 0;
        let allows_scene_snapshot_cache = !(scene_snapshot_blocked_by_structure
            || scene_snapshot_blocked_by_layout
            || scene_snapshot_blocked_by_raster
            || scene_snapshot_blocked_by_apply_change);

        Self {
            allows_scene_snapshot_cache,
            scene_snapshot_blocked_by_structure,
            scene_snapshot_blocked_by_layout,
            scene_snapshot_blocked_by_raster,
            scene_snapshot_blocked_by_apply_change,
        }
    }
}

pub fn plan_for_scene(
    layout_pass: &LayoutPassStats,
    apply_changed_nodes: usize,
) -> SceneRenderPlan {
    SceneRenderPlan::from_layout_pass(layout_pass, apply_changed_nodes)
}

#[cfg(test)]
mod plan_tests {
    use super::SceneRenderPlan;
    use crate::layout::LayoutPassStats;

    #[test]
    fn apply_changed_scene_disables_scene_snapshot_cache() {
        let plan = SceneRenderPlan::from_layout_pass(&LayoutPassStats::default(), 2);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: false,
                scene_snapshot_blocked_by_structure: false,
                scene_snapshot_blocked_by_layout: false,
                scene_snapshot_blocked_by_raster: false,
                scene_snapshot_blocked_by_apply_change: true,
            }
        );
    }

    #[test]
    fn clean_scene_reuses_scene_snapshot_cache() {
        let plan = SceneRenderPlan::from_layout_pass(&LayoutPassStats::default(), 0);
        assert_eq!(
            plan,
            SceneRenderPlan {
                allows_scene_snapshot_cache: true,
                scene_snapshot_blocked_by_structure: false,
                scene_snapshot_blocked_by_layout: false,
                scene_snapshot_blocked_by_raster: false,
                scene_snapshot_blocked_by_apply_change: false,
            }
        );
    }

    #[test]
    fn layout_pass_records_scene_snapshot_block_reasons() {
        let layout_pass = LayoutPassStats {
            structure_rebuild: true,
            layout_dirty_nodes: 2,
            raster_dirty_nodes: 3,
            ..Default::default()
        };

        let plan = SceneRenderPlan::from_layout_pass(&layout_pass, 4);

        assert!(!plan.allows_scene_snapshot_cache);
        assert!(plan.scene_snapshot_blocked_by_structure);
        assert!(plan.scene_snapshot_blocked_by_layout);
        assert!(plan.scene_snapshot_blocked_by_raster);
        assert!(plan.scene_snapshot_blocked_by_apply_change);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum StableNodeReuse {
    /// 足够便宜的叶子，直接重画比 record picture 更划算。
    DirectLeaf,
    /// 稳定的 Bitmap / Lucide 叶子，由 `ItemPictureCache` 跨帧复用。
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
        DisplayItem::Bitmap(_) | DisplayItem::Lottie(_) | DisplayItem::SvgPath(_) => {
            StableNodeReuse::ItemPictureLeaf
        }
        DisplayItem::DrawScript(_) => StableNodeReuse::DirectLeaf,
    }
}

#[cfg(test)]
mod reuse_tests {
    use super::{StableNodeReuse, analyze_stable_node_reuse};
    use crate::{
        analyze::{
            DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
            DisplayNodeInvalidation,
            annotation::{
                AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey,
            },
            fingerprint::SubtreeSnapshotFingerprint,
        },
        display::list::{
            DisplayItem, DisplayRect, DisplayTransform, RectDisplayItem, RectPaintStyle,
            TextDisplayItem,
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
            input_fingerprints: Default::default(),
            layout_output_fingerprint: Default::default(),
            recorded_subtree_fingerprint: Default::default(),
            transform: DisplayTransform {
                translation_x: 0.0,
                translation_y: 0.0,
                bounds: rect_bounds(),
                transforms: Vec::new(),
            },
            opacity: 1.0,
            css_filter: Default::default(),
            backdrop_blur_sigma: None,
            paint_clip: None,
            clip: None,
            item,
            children,
            draw_slot: None,
            hidden_subtree: Vec::new(),
        }
    }

    fn tree(
        nodes: Vec<AnnotatedDisplayNode>,
        analysis: DisplayAnalysisTable,
    ) -> AnnotatedDisplayTree {
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
                            apply_changed: false,
                        },
                    );
                }
                table
            },
            analyze_reuse: vec![],
        }
    }

    #[test]
    fn rect_leaf_prefers_direct_leaf_reuse() {
        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(2)),
            },
        );
        let display_tree = tree(
            vec![node(
                DisplayItem::Rect(RectDisplayItem {
                    bounds: rect_bounds(),
                    paint: RectPaintStyle {
                        background: Vec::new(),
                        border_radius: BorderRadius::default(),
                        border_width: None,
                        border_top_width: None,
                        border_right_width: None,
                        border_bottom_width: None,
                        border_left_width: None,
                        border_color: None,
                        border_style: None,
                        box_shadow: Vec::new(),
                        inset_shadow: Vec::new(),
                        drop_shadow: Vec::new(),
                        backdrop_blur_sigma: None,
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
    fn text_leaf_prefers_text_snapshot_leaf_reuse() {
        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(2)),
            },
        );
        let display_tree = tree(
            vec![node(
                DisplayItem::Text(TextDisplayItem {
                    bounds: rect_bounds(),
                    text: "Hello".into(),
                    style: ComputedTextStyle::default(),
                    allow_wrap: false,
                    truncate: false,
                    drop_shadow: Vec::new(),
                    text_shadows: Vec::new(),
                    text_unit_overrides: None,
                    visual_expand_x: 0.0,
                    visual_expand_y: 0.0,
                    glyphs: None,
                }),
                Vec::new(),
            )],
            analysis,
        );

        assert_eq!(
            analyze_stable_node_reuse(&display_tree, AnnotatedNodeHandle(0)),
            StableNodeReuse::TextSnapshotLeaf
        );
    }

    #[test]
    fn text_leaf_with_unit_overrides_prefers_direct_leaf_reuse() {
        use crate::script::{TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch};

        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(2)),
            },
        );
        let display_tree = tree(
            vec![node(
                DisplayItem::Text(TextDisplayItem {
                    bounds: rect_bounds(),
                    text: "Hello".into(),
                    style: ComputedTextStyle::default(),
                    allow_wrap: false,
                    truncate: false,
                    drop_shadow: Vec::new(),
                    text_shadows: Vec::new(),
                    text_unit_overrides: Some(TextUnitOverrideBatch {
                        granularity: TextUnitGranularity::Grapheme,
                        overrides: vec![TextUnitOverride {
                            translate_y: Some(-12.0),
                            ..Default::default()
                        }],
                    }),
                    visual_expand_x: 0.0,
                    visual_expand_y: 12.0,
                    glyphs: None,
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
    fn bitmap_leaf_prefers_item_picture_leaf_reuse() {
        use crate::display::list::{BitmapDisplayItem, BitmapPaintStyle};
        use crate::ir::asset_id::AssetId;
        use crate::style::ObjectFit;

        let asset_id = AssetId::new(crate::ir::asset_id::ResourceKind::Image, "/tmp/x.png");

        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint(3)),
            },
        );
        let display_tree = tree(
            vec![node(
                DisplayItem::Bitmap(BitmapDisplayItem {
                    bounds: rect_bounds(),
                    asset_id,
                    width: 10,
                    height: 10,
                    video_timing: None,
                    paint_epoch: 0,
                    object_fit: ObjectFit::Fill,
                    paint: BitmapPaintStyle {
                        background: Vec::new(),
                        border_radius: BorderRadius::default(),
                        border_width: None,
                        border_top_width: None,
                        border_right_width: None,
                        border_bottom_width: None,
                        border_left_width: None,
                        border_color: None,
                        border_style: None,
                        box_shadow: Vec::new(),
                        inset_shadow: Vec::new(),
                        drop_shadow: Vec::new(),
                    },
                }),
                Vec::new(),
            )],
            analysis,
        );

        assert_eq!(
            analyze_stable_node_reuse(&display_tree, AnnotatedNodeHandle(0)),
            StableNodeReuse::ItemPictureLeaf
        );
    }
}
