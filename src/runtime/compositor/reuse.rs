use crate::{
    display::list::DisplayItem,
    runtime::annotation::{AnnotatedDisplayTree, AnnotatedNodeHandle},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StableNodeReuse {
    /// 足够便宜的叶子，直接重画比 record picture 更划算。
    DirectLeaf,
    /// 稳定的 Bitmap / Lucide 叶子，由 `ItemPictureCache` 跨帧复用。
    ItemPictureLeaf,
    /// 稳定的 Text 叶子，由 `TextSnapshotCache` 跨帧复用。
    TextSnapshotLeaf,
    /// 稳定的非叶子子树，由 `SubtreeSnapshotCache` 跨帧复用。
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
        DisplayItem::Timeline(_) => StableNodeReuse::DirectLeaf,
        DisplayItem::Text(text) => {
            if text.text_unit_overrides.is_some() {
                StableNodeReuse::DirectLeaf
            } else {
                StableNodeReuse::TextSnapshotLeaf
            }
        }
        DisplayItem::Bitmap(_) | DisplayItem::SvgPath(_) => StableNodeReuse::ItemPictureLeaf,
        DisplayItem::DrawScript(_) => StableNodeReuse::DirectLeaf,
    }
}

pub(crate) fn analyze_live_node_item_execution(
    display_tree: &AnnotatedDisplayTree,
    handle: AnnotatedNodeHandle,
) -> LiveNodeItemExecution {
    match &display_tree.node(handle).item {
        DisplayItem::DrawScript(_) => {
            // Stable DrawScript 有 paint fingerprint → 交给 draw_display_item 走 ItemPictureCache。
            // TimeVariant 保留 FrameLocalPicture 兜底,避免给 cache 塞永远 miss 的短命 key。
            match display_tree.analysis(handle).paint_variance {
                crate::runtime::fingerprint::PaintVariance::Stable => LiveNodeItemExecution::Direct,
                crate::runtime::fingerprint::PaintVariance::TimeVariant => {
                    LiveNodeItemExecution::FrameLocalPicture
                }
            }
        }
        DisplayItem::Timeline(_) => LiveNodeItemExecution::Direct,
        DisplayItem::Rect(_)
        | DisplayItem::Text(_)
        | DisplayItem::Bitmap(_)
        | DisplayItem::SvgPath(_) => LiveNodeItemExecution::Direct,
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
            annotation::{
                AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey,
            },
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
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint {
                    primary: 2,
                    secondary: 2,
                }),
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
                paint_variance: PaintVariance::Stable,
                subtree_contains_time_variant: false,
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint {
                    primary: 2,
                    secondary: 2,
                }),
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
                    text_unit_overrides: None,
                    visual_expand_x: 0.0,
                    visual_expand_y: 0.0,
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
        use crate::scene::script::{TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch};

        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_variance: PaintVariance::Stable,
                subtree_contains_time_variant: false,
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint {
                    primary: 2,
                    secondary: 2,
                }),
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
                    text_unit_overrides: Some(TextUnitOverrideBatch {
                        granularity: TextUnitGranularity::Grapheme,
                        overrides: vec![TextUnitOverride {
                            translate_y: Some(-12.0),
                            ..Default::default()
                        }],
                    }),
                    visual_expand_x: 0.0,
                    visual_expand_y: 12.0,
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
        use crate::resource::assets::AssetsMap;
        use crate::style::ObjectFit;

        let mut assets = AssetsMap::new();
        let asset_id = assets.register_dimensions(std::path::Path::new("/tmp/x.png"), 10, 10);

        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_variance: PaintVariance::Stable,
                subtree_contains_time_variant: false,
                paint_fingerprint: Some(1),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint {
                    primary: 3,
                    secondary: 3,
                }),
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
                    object_fit: ObjectFit::Fill,
                    paint: BitmapPaintStyle {
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

        let _ = &assets;
    }

    #[test]
    fn draw_script_stable_prefers_direct_execution() {
        let mut analysis = DisplayAnalysisTable::default();
        analysis.insert(
            AnnotatedNodeHandle(0),
            DisplayNodeAnalysis {
                paint_variance: PaintVariance::Stable,
                subtree_contains_time_variant: false,
                paint_fingerprint: Some(7),
                snapshot_fingerprint: Some(SubtreeSnapshotFingerprint {
                    primary: 8,
                    secondary: 8,
                }),
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
            LiveNodeItemExecution::Direct,
            "Stable DrawScript 应走 Direct 路径,由 draw_display_item 接 ItemPictureCache"
        );
    }

    #[test]
    fn draw_script_time_variant_falls_back_to_frame_local_picture() {
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
            LiveNodeItemExecution::FrameLocalPicture,
            "TimeVariant DrawScript 走 FrameLocalPicture 兜底,不污染 ItemPictureCache"
        );
    }
}
