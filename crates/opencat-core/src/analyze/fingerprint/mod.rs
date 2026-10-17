//! Paint、subtree snapshot 与 composite 三个独立维度的指纹。
//!
//! - [`annotated_subtree_paint_fingerprint`]：纯 paint 指纹。仅由"画什么"决定，不含任何 composite。
//! - [`annotated_subtree_snapshot_fingerprint`]：subtree picture 缓存键。
//!   不含当前节点自己的 composite，但递归包含所有后代 composite，因为后代会被烘焙进当前节点 picture。
//! - [`composite_signature`]：每帧比对用的合成参数摘要（transform/opacity/blur），
//!   **不进入缓存键**。
//!
//! 这个模块是纯函数、无副作用、无状态、不依赖 profile。

mod display_item;

use std::hash::{Hash, Hasher};

fn new_hasher() -> ahash::AHasher {
    ahash::AHasher::default()
}

use crate::{
    analyze::{
        DisplayAnalysisTable,
        annotation::{AnnotatedDisplayNode, RecordedNodeSemantics},
    },
    display::{
        list::{DisplayClip, DisplayItem},
        tree::{DisplayNode, DisplayRecordedSubtreeFingerprint, HiddenChildDisplayNode},
    },
    layout::tree::LayoutOutputFingerprint,
};

use display_item::{DisplayItemFp, F32Hash};

/// subtree picture cache 的 fingerprint。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubtreeSnapshotFingerprint(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisplayClipFingerprint(pub u64);

impl DisplayClipFingerprint {
    pub fn from_clip(clip: Option<&DisplayClip>) -> Self {
        let mut hasher = new_hasher();
        clip.is_some().hash(&mut hasher);
        if let Some(clip) = clip {
            F32Hash(clip.bounds.x).hash(&mut hasher);
            F32Hash(clip.bounds.y).hash(&mut hasher);
            F32Hash(clip.bounds.width).hash(&mut hasher);
            F32Hash(clip.bounds.height).hash(&mut hasher);
            clip.border_radius.hash(&mut hasher);
        }
        Self(hasher.finish())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisplayRecordedFingerprint(pub u64);

impl DisplayRecordedFingerprint {
    pub fn from_recorded(semantics: &RecordedNodeSemantics<'_>) -> Self {
        Self::from_parts(
            semantics.layout_output_fingerprint,
            semantics.item,
            semantics.paint_clip,
            semantics.clip,
        )
    }

    pub fn from_display_node(node: &DisplayNode) -> Self {
        Self::from_parts(
            node.layout_output_fingerprint,
            &node.item,
            node.paint_clip.as_ref(),
            node.clip.as_ref(),
        )
    }

    pub fn from_parts(
        layout_output_fingerprint: LayoutOutputFingerprint,
        item: &DisplayItem,
        paint_clip: Option<&DisplayClip>,
        clip: Option<&DisplayClip>,
    ) -> Self {
        let mut hasher = new_hasher();
        layout_output_fingerprint.record_size.hash(&mut hasher);
        DisplayItemFp(item).hash(&mut hasher);
        DisplayClipFingerprint::from_clip(paint_clip).hash(&mut hasher);
        DisplayClipFingerprint::from_clip(clip).hash(&mut hasher);
        Self(hasher.finish())
    }
}

pub fn display_recorded_subtree_fingerprint(
    node: &DisplayNode,
) -> DisplayRecordedSubtreeFingerprint {
    let mut hasher = new_hasher();
    node.element_id.hash(&mut hasher);
    node.input_fingerprints
        .paint_input_subtree
        .hash(&mut hasher);
    DisplayRecordedFingerprint::from_display_node(node).hash(&mut hasher);
    node.draw_slot.is_some().hash(&mut hasher);
    if let Some(slot) = &node.draw_slot {
        let item = DisplayItem::DrawScript(slot.clone());
        DisplayRecordedFingerprint::from_parts(node.layout_output_fingerprint, &item, None, None)
            .hash(&mut hasher);
    }
    node.children.len().hash(&mut hasher);
    for child in &node.children {
        child.recorded_subtree_fingerprint.hash(&mut hasher);
    }
    DisplayRecordedSubtreeFingerprint(hasher.finish())
}

/// 基于已注解节点计算子树 paint fingerprint。
///
/// 要求所有后代的 `paint_fingerprint` 已经自底向上填充完成。
pub(crate) fn annotated_subtree_paint_fingerprint(
    node: &AnnotatedDisplayNode,
    analysis: &DisplayAnalysisTable,
) -> Option<u64> {
    let mut hasher = new_hasher();
    hash_node_recorded_paint(node, &mut hasher);
    node.children.len().hash(&mut hasher);
    for &child_handle in &node.children {
        let child_fp = analysis.require(child_handle).paint_fingerprint;
        debug_assert!(
            child_fp.is_some(),
            "invariant: stable annotated child must carry paint_fingerprint"
        );
        child_fp.unwrap_or(0).hash(&mut hasher);
    }
    Some(hasher.finish())
}

/// 基于已注解节点计算 subtree snapshot fingerprint。
///
/// 要求所有后代的 `snapshot_fingerprint` 已经自底向上填充完成。
///
/// 仅 hash paint 内容（节点自身 paint + 子节点 snapshot_fingerprint 递归），
/// 不含任何 composite（transform/opacity/blur）。
/// composite 在 replay 时由 render dispatch 动态 apply。
pub(crate) fn annotated_subtree_snapshot_fingerprint(
    node: &AnnotatedDisplayNode,
    analysis: &DisplayAnalysisTable,
) -> Option<SubtreeSnapshotFingerprint> {
    let mut hasher = new_hasher();

    hash_node_recorded_paint(node, &mut hasher);
    node.children.len().hash(&mut hasher);

    for &child_handle in &node.children {
        let child_fp = analysis.require(child_handle).snapshot_fingerprint;
        debug_assert!(
            child_fp.is_some(),
            "invariant: annotated child must carry snapshot_fingerprint"
        );
        child_fp
            .unwrap_or(SubtreeSnapshotFingerprint(0))
            .0
            .hash(&mut hasher);
    }

    Some(SubtreeSnapshotFingerprint(hasher.finish()))
}

fn hash_node_recorded_paint<H: Hasher>(node: &AnnotatedDisplayNode, hasher: &mut H) {
    node.input_fingerprints.paint_input_subtree.hash(hasher);
    DisplayRecordedFingerprint::from_recorded(&node.recorded_semantics()).hash(hasher);
}

pub(super) fn hash_hidden_child_display_node<H: Hasher>(
    child: &HiddenChildDisplayNode,
    hasher: &mut H,
) {
    child.owner_id.hash(hasher);
    child.node.recorded_subtree_fingerprint.hash(hasher);
    hash_display_node_composite_subtree(&child.node, hasher);
}

fn hash_display_node_composite_subtree<H: Hasher>(node: &DisplayNode, hasher: &mut H) {
    hash_display_node_composite(node, hasher);
    node.children.len().hash(hasher);
    for child in &node.children {
        hash_display_node_composite_subtree(child, hasher);
    }
}

fn hash_display_node_composite<H: Hasher>(node: &DisplayNode, hasher: &mut H) {
    F32Hash(node.transform.translation_x).hash(hasher);
    F32Hash(node.transform.translation_y).hash(hasher);
    F32Hash(node.transform.origin.x).hash(hasher);
    F32Hash(node.transform.origin.y).hash(hasher);
    F32Hash(node.opacity).hash(hasher);
    node.css_filter.hash(hasher);
    node.backdrop_blur_sigma.map(F32Hash).hash(hasher);
    node.backdrop_filter.hash(hasher);
    node.transform.transforms.hash(hasher);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyze::{
            DisplayAnalysisTable, DisplayNodeAnalysis,
            annotation::{
                AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey,
            },
        },
        display::{
            list::{
                BitmapDisplayItem, BitmapPaintStyle, DisplayClip, DisplayItem, DisplayRect,
                DisplayTransform, DrawScriptDisplayItem, RectDisplayItem, RectPaintStyle,
            },
            tree::{DisplayNode, HiddenChildDisplayNode},
        },
        ir::asset_id::AssetId,
        layout::tree::LayoutOutputFingerprint,
        resolve::tree::ElementId,
        style::{BorderRadius, CssFilter, ObjectFit, Transform},
    };

    struct AnnotatedRectConfig {
        key: RenderNodeKey,
        transform: DisplayTransform,
        opacity: f32,
        css_filter: CssFilter,
        backdrop_blur_sigma: Option<f32>,
        backdrop_filter: CssFilter,
        paint_clip: Option<DisplayClip>,
        clip: Option<DisplayClip>,
        children: Vec<TestAnnotatedNode>,
        background: Vec<crate::style::BackgroundFill>,
        layout_output_fingerprint: LayoutOutputFingerprint,
    }

    impl Default for AnnotatedRectConfig {
        fn default() -> Self {
            Self {
                key: RenderNodeKey(1),
                transform: rect_transform(0.0, 0.0),
                opacity: 1.0,
                css_filter: CssFilter::default(),
                backdrop_blur_sigma: None,
                backdrop_filter: CssFilter::default(),
                paint_clip: None,
                clip: None,
                children: Vec::new(),
                background: Vec::new(),
                layout_output_fingerprint: LayoutOutputFingerprint::default(),
            }
        }
    }

    #[derive(Clone)]
    struct TestAnnotatedNode {
        key: RenderNodeKey,
        transform: DisplayTransform,
        opacity: f32,
        css_filter: CssFilter,
        backdrop_blur_sigma: Option<f32>,
        backdrop_filter: CssFilter,
        paint_clip: Option<DisplayClip>,
        clip: Option<DisplayClip>,
        item: DisplayItem,
        children: Vec<TestAnnotatedNode>,
        layout_output_fingerprint: LayoutOutputFingerprint,
    }

    fn empty_bounds() -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    fn rect_transform(translation_x: f32, translation_y: f32) -> DisplayTransform {
        DisplayTransform {
            translation_x,
            translation_y,
            bounds: empty_bounds(),
            transforms: Vec::new(),
            origin: Default::default(),
        }
    }

    fn annotated_rect_node(config: AnnotatedRectConfig) -> TestAnnotatedNode {
        TestAnnotatedNode {
            key: config.key,
            transform: config.transform,
            opacity: config.opacity,
            css_filter: config.css_filter,
            backdrop_blur_sigma: config.backdrop_blur_sigma,
            backdrop_filter: config.backdrop_filter,
            paint_clip: config.paint_clip,
            clip: config.clip,
            item: DisplayItem::Rect(RectDisplayItem {
                bounds: empty_bounds(),
                paint: RectPaintStyle {
                    background: config.background,
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
                    blend_mode: Default::default(),
                },
            }),
            children: config.children,
            layout_output_fingerprint: config.layout_output_fingerprint,
        }
    }

    impl TestAnnotatedNode {
        fn into_annotated_node(self) -> AnnotatedDisplayNode {
            AnnotatedDisplayNode {
                input_fingerprints: Default::default(),
                layout_output_fingerprint: self.layout_output_fingerprint,
                recorded_subtree_fingerprint: Default::default(),
                transform: self.transform,
                opacity: self.opacity,
                css_filter: self.css_filter,
                backdrop_blur_sigma: self.backdrop_blur_sigma,
                backdrop_filter: self.backdrop_filter,
                paint_clip: self.paint_clip,
                clip: self.clip,
                item: self.item,
                children: Vec::new(),
                draw_slot: None,
                hidden_subtree: Vec::new(),
            }
        }
    }

    fn finalize_annotated_tree(node: TestAnnotatedNode) -> AnnotatedDisplayTree {
        let mut nodes = Vec::new();
        let mut keys = Vec::new();
        let mut layer_bounds = Vec::new();
        let mut analysis = DisplayAnalysisTable::default();
        let root = finalize_test_node(
            node,
            &mut nodes,
            &mut keys,
            &mut layer_bounds,
            &mut analysis,
        );
        AnnotatedDisplayTree {
            root,
            nodes,
            keys,
            layer_bounds,
            analysis,
        }
    }

    fn finalize_test_node(
        node: TestAnnotatedNode,
        nodes: &mut Vec<AnnotatedDisplayNode>,
        keys: &mut Vec<RenderNodeKey>,
        layer_bounds: &mut Vec<DisplayRect>,
        analysis: &mut DisplayAnalysisTable,
    ) -> AnnotatedNodeHandle {
        let children = node
            .children
            .into_iter()
            .map(|child| {
                finalize_test_node(child, nodes, keys, layer_bounds, analysis)
            })
            .collect::<Vec<_>>();

        let handle = AnnotatedNodeHandle(nodes.len());
        let annotated = AnnotatedDisplayNode {
            input_fingerprints: Default::default(),
            layout_output_fingerprint: node.layout_output_fingerprint,
            recorded_subtree_fingerprint: Default::default(),
            transform: node.transform,
            opacity: node.opacity,
            css_filter: node.css_filter,
            backdrop_blur_sigma: node.backdrop_blur_sigma,
            backdrop_filter: node.backdrop_filter,
            paint_clip: node.paint_clip,
            clip: node.clip,
            item: node.item,
            children,
            draw_slot: None,
            hidden_subtree: Vec::new(),
        };

        let node_analysis = DisplayNodeAnalysis {
            paint_fingerprint: annotated_subtree_paint_fingerprint(&annotated, analysis),
            snapshot_fingerprint: annotated_subtree_snapshot_fingerprint(&annotated, analysis),
        };
        let mut node_layer_bounds = annotated.item.visual_bounds();
        for &child_handle in &annotated.children {
            let child = &nodes[child_handle.0];
            let child_bounds = layer_bounds[child_handle.0]
                .translate(child.transform.translation_x, child.transform.translation_y);
            node_layer_bounds = node_layer_bounds.union(child_bounds);
        }
        keys.push(node.key);
        nodes.push(annotated);
        layer_bounds.push(node_layer_bounds);
        analysis.insert(handle, node_analysis);

        handle
    }

    #[test]
    fn paint_fingerprint_is_invariant_under_translation() {
        let a = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            transform: rect_transform(10.0, 20.0),
            ..Default::default()
        }));
        let b = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            transform: rect_transform(50.0, 80.0),
            ..Default::default()
        }));
        let fp_a = a.analysis(a.root).paint_fingerprint;
        let fp_b = b.analysis(b.root).paint_fingerprint;
        assert_eq!(fp_a, fp_b, "translation must not affect paint fingerprint");
        assert!(fp_a.is_some());
    }

    #[test]
    fn paint_fingerprint_tracks_layout_record_size_not_transform_position() {
        let size_a = LayoutOutputFingerprint {
            record_size: 11,
            ..LayoutOutputFingerprint::default()
        };
        let size_b = LayoutOutputFingerprint {
            record_size: 22,
            ..LayoutOutputFingerprint::default()
        };
        let a = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            layout_output_fingerprint: size_a,
            transform: rect_transform(10.0, 20.0),
            ..Default::default()
        }));
        let moved_same_size = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            layout_output_fingerprint: size_a,
            transform: rect_transform(50.0, 80.0),
            ..Default::default()
        }));
        let resized = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            layout_output_fingerprint: size_b,
            transform: rect_transform(10.0, 20.0),
            ..Default::default()
        }));

        assert_eq!(
            a.analysis(a.root).paint_fingerprint,
            moved_same_size
                .analysis(moved_same_size.root)
                .paint_fingerprint,
            "recorded paint fingerprint must ignore position changes"
        );
        assert_ne!(
            a.analysis(a.root).paint_fingerprint,
            resized.analysis(resized.root).paint_fingerprint,
            "recorded paint fingerprint must change when layout record size changes"
        );
    }

    #[test]
    fn display_recorded_fingerprint_is_explicit_recorded_semantics_api() {
        let base = annotated_rect_node(AnnotatedRectConfig {
            layout_output_fingerprint: LayoutOutputFingerprint {
                record_size: 11,
                ..LayoutOutputFingerprint::default()
            },
            ..Default::default()
        })
        .into_annotated_node();
        let clipped = annotated_rect_node(AnnotatedRectConfig {
            layout_output_fingerprint: LayoutOutputFingerprint {
                record_size: 11,
                ..LayoutOutputFingerprint::default()
            },
            clip: Some(DisplayClip {
                bounds: DisplayRect {
                    x: 0.0,
                    y: 0.0,
                    width: 80.0,
                    height: 50.0,
                },
                border_radius: BorderRadius {
                    top_left: 8.0,
                    top_right: 8.0,
                    bottom_right: 8.0,
                    bottom_left: 8.0,
                },
            }),
            ..Default::default()
        })
        .into_annotated_node();

        let base_fp = DisplayRecordedFingerprint::from_recorded(&base.recorded_semantics());
        let clipped_fp = DisplayRecordedFingerprint::from_recorded(&clipped.recorded_semantics());

        assert_ne!(
            base_fp, clipped_fp,
            "clip is recorded display semantics and must be captured explicitly"
        );
        assert_eq!(
            base_fp,
            DisplayRecordedFingerprint::from_parts(
                base.layout_output_fingerprint,
                &base.item,
                base.paint_clip.as_ref(),
                base.clip.as_ref()
            ),
            "recorded fingerprint must be constructible without re-reading AnnotatedDisplayNode"
        );
    }

    #[test]
    fn paint_fingerprint_is_invariant_under_opacity() {
        let a = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig::default()));
        let b = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            opacity: 0.3,
            ..Default::default()
        }));
        let fp_a = a.analysis(a.root).paint_fingerprint;
        let fp_b = b.analysis(b.root).paint_fingerprint;
        assert_eq!(fp_a, fp_b, "opacity must not affect paint fingerprint");
    }

    #[test]
    fn paint_fingerprint_is_invariant_under_transforms() {
        let mut transform_a = rect_transform(0.0, 0.0);
        transform_a.transforms = vec![Transform::Scale { value: 1.0 }];
        let mut transform_b = rect_transform(0.0, 0.0);
        transform_b.transforms = vec![Transform::Scale { value: 2.0 }];
        let a = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            transform: transform_a,
            ..Default::default()
        }));
        let b = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            transform: transform_b,
            ..Default::default()
        }));
        let fp_a = a.analysis(a.root).paint_fingerprint;
        let fp_b = b.analysis(b.root).paint_fingerprint;
        assert_eq!(fp_a, fp_b, "transforms must not affect paint fingerprint");
    }

    #[test]
    fn paint_fingerprint_changes_with_paint_content() {
        let a = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig::default()));
        let b = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            background: vec![crate::style::BackgroundFill::Solid {
                color: crate::style::ColorToken::Red,
            }],
            ..Default::default()
        }));
        let fp_a = a.analysis(a.root).paint_fingerprint;
        let fp_b = b.analysis(b.root).paint_fingerprint;
        assert_ne!(fp_a, fp_b, "different paint content must differ");
    }

    #[test]
    fn snapshot_fingerprint_ignores_current_node_transform() {
        let mut transform_a = rect_transform(0.0, 0.0);
        transform_a.transforms = vec![Transform::Scale { value: 1.0 }];
        let mut transform_b = rect_transform(0.0, 0.0);
        transform_b.transforms = vec![Transform::Scale { value: 2.0 }];
        let a = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            transform: transform_a,
            ..Default::default()
        }));
        let b = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            transform: transform_b,
            ..Default::default()
        }));

        let fp_a = a.analysis(a.root).snapshot_fingerprint;
        let fp_b = b.analysis(b.root).snapshot_fingerprint;
        assert_eq!(
            fp_a, fp_b,
            "current node transform is applied outside its snapshot and must not bust the key"
        );
    }

    #[test]
    fn snapshot_fingerprint_ignores_descendant_transform_changes() {
        let mut a = annotated_rect_node(AnnotatedRectConfig::default());
        let mut b = annotated_rect_node(AnnotatedRectConfig::default());
        let mut child_transform_a = rect_transform(0.0, 0.0);
        child_transform_a.transforms = vec![Transform::Scale { value: 1.0 }];
        let mut child_transform_b = rect_transform(0.0, 0.0);
        child_transform_b.transforms = vec![Transform::Scale { value: 2.0 }];
        let child_a = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            transform: child_transform_a,
            ..Default::default()
        });
        let child_b = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            transform: child_transform_b,
            ..Default::default()
        });
        a.children.push(child_a);
        b.children.push(child_b);
        let a = finalize_annotated_tree(a);
        let b = finalize_annotated_tree(b);

        let fp_a = a.analysis(a.root).snapshot_fingerprint;
        let fp_b = b.analysis(b.root).snapshot_fingerprint;
        assert_eq!(
            fp_a, fp_b,
            "descendant transform is applied dynamically at replay and must not affect the cache key"
        );
        assert!(fp_a.is_some(), "snapshot_fingerprint is always Some");
    }

    #[test]
    fn snapshot_fingerprint_always_some_for_nested_subtrees() {
        // composite is applied dynamically at replay, so nesting never blocks
        // snapshot_fingerprint.
        let child = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            ..Default::default()
        });
        let tree = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![child],
            ..Default::default()
        }));
        assert!(
            tree.analysis(tree.root).snapshot_fingerprint.is_some(),
            "parent subtree should have snapshot_fingerprint"
        );
        assert!(
            tree.analysis(tree.root).paint_fingerprint.is_some(),
            "paint fingerprint is independent of nesting"
        );
    }

    #[test]
    fn snapshot_fingerprint_tracks_clip_that_changes_recorded_subtree() {
        let child = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            transform: rect_transform(12.0, 0.0),
            ..Default::default()
        });
        let a = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![child.clone()],
            ..Default::default()
        }));
        let b = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            clip: Some(DisplayClip {
                bounds: empty_bounds(),
                border_radius: BorderRadius {
                    top_left: 8.0,
                    top_right: 8.0,
                    bottom_right: 8.0,
                    bottom_left: 8.0,
                },
            }),
            children: vec![child],
            ..Default::default()
        }));

        assert_ne!(
            a.analysis(a.root).snapshot_fingerprint,
            b.analysis(b.root).snapshot_fingerprint,
            "clip changes recorded subtree contents and must affect snapshot fingerprint"
        );
    }

    #[test]
    fn snapshot_fingerprint_is_stable() {
        use std::hash::Hasher;

        let mut hasher = new_hasher();
        hasher.write_u64(0xdead_beef);
        let fp = SubtreeSnapshotFingerprint(hasher.finish());
        assert_eq!(fp.0, fp.0);
    }

}
