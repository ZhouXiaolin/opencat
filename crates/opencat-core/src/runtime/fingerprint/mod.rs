//! Paint、subtree snapshot 与 composite 三个独立维度的指纹。
//!
//! - [`annotated_subtree_paint_fingerprint`]：纯 paint 指纹。仅由"画什么"决定，不含任何 composite。
//! - [`annotated_subtree_snapshot_fingerprint`]：subtree picture 缓存键。
//!   不含当前节点自己的 composite，但递归包含所有后代 composite，因为后代会被烘焙进当前节点 picture。
//! - [`composite_signature`]：每帧比对用的合成参数摘要（transform/opacity/blur），
//!   **不进入缓存键**。
//! - [`classify_paint`]：判定单个 DisplayItem 的 paint variance。
//!
//! 这个模块是纯函数、无副作用、无状态、不依赖 profile。

mod display_item;

use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;

fn new_primary_hasher() -> FxHasher {
    FxHasher::default()
}

fn new_secondary_hasher() -> ahash::AHasher {
    ahash::AHasher::default()
}

use crate::{
    display::list::DisplayItem,
    runtime::{
        analysis::{DisplayAnalysisTable, DisplayInvalidationTable},
        annotation::{AnnotatedDisplayNode, DrawCompositeSemantics, RecordedNodeSemantics},
    },
};

use display_item::{ClipFp, DisplayItemFp, F32Hash, item_is_time_variant};

/// subtree picture cache 的双 hash fingerprint。
///
/// `primary` 用于 `HashMap` 查表；`secondary` 在命中后做二次验证，
/// 规避 64-bit hash 碰撞。两个 hasher 必须 feed 完全相同的字节流，
/// 但底层 hash 算法独立。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubtreeSnapshotFingerprint {
    pub primary: u64,
    pub secondary: u64,
}

/// 每个节点的 paint variance 分类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintVariance {
    /// 画面内容跨帧稳定。
    Stable,
    /// 画面内容每帧都可能变。
    TimeVariant,
}

/// 合成参数摘要：transform、opacity、blur。
///
/// 这里只放 draw-time 的摆放/合成语义：
/// - translation / transforms
/// - opacity
/// - backdrop blur
///
/// 像 `clip` 这类会改变录制内容的语义仍然留在 snapshot / skeleton 指纹里，
/// 不放进 composite dirty 判定。
///
/// **不进入缓存键**。每帧对同一节点比对，用来判断是否需要重新合成。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompositeSig {
    pub translation_x_bits: u32,
    pub translation_y_bits: u32,
    pub transforms_hash: u64,
    pub opacity_bits: u32,
    pub backdrop_blur_bits: Option<u32>,
}

impl CompositeSig {
    pub fn from_annotated_node(node: &AnnotatedDisplayNode) -> Self {
        Self::from_draw_composite(&node.draw_composite_semantics())
    }

    fn from_draw_composite(draw: &DrawCompositeSemantics<'_>) -> Self {
        let mut transforms_hasher = new_primary_hasher();
        draw.transform.transforms.hash(&mut transforms_hasher);
        Self {
            translation_x_bits: draw.transform.translation_x.to_bits(),
            translation_y_bits: draw.transform.translation_y.to_bits(),
            transforms_hash: transforms_hasher.finish(),
            opacity_bits: draw.opacity.to_bits(),
            backdrop_blur_bits: draw.backdrop_blur_sigma.map(|v| v.to_bits()),
        }
    }
}

/// 判定单个 DisplayItem 的 paint variance。
pub fn classify_paint(item: &DisplayItem) -> PaintVariance {
    if item_is_time_variant(item) {
        PaintVariance::TimeVariant
    } else {
        PaintVariance::Stable
    }
}

/// 计算单个 DisplayItem 的 paint fingerprint(作为 `ItemPictureCache` key)。
///
/// 语义:
/// - TimeVariant 项(包含视频 Bitmap) → None(不进 cache)
///   视频帧数据由下层 `video_frame_cache` 按 pts 做复用,Picture 层无需再缓存。
/// - Stable 项 → Some(基于 `DisplayItemFp` 全量 hash)
pub fn item_paint_fingerprint(item: &DisplayItem) -> Option<u64> {
    if item_is_time_variant(item) {
        return None;
    }
    let mut hasher = new_primary_hasher();
    DisplayItemFp(item).hash(&mut hasher);
    Some(hasher.finish())
}

/// 基于已注解节点计算子树 paint fingerprint。
///
/// 要求所有后代的 `paint_fingerprint` 已经自底向上填充完成。
pub(crate) fn annotated_subtree_paint_fingerprint(
    node: &AnnotatedDisplayNode,
    analysis: &DisplayAnalysisTable,
    subtree_contains_time_variant: bool,
) -> Option<u64> {
    if subtree_contains_time_variant {
        return None;
    }
    let mut hasher = new_primary_hasher();
    hash_node_recorded_paint(node, &mut hasher);
    node.children.len().hash(&mut hasher);
    for &child_handle in &node.children {
        analysis
            .require(child_handle)
            .paint_fingerprint
            .expect("stable annotated child must carry paint_fingerprint")
            .hash(&mut hasher);
    }
    Some(hasher.finish())
}

/// 基于已注解节点计算 subtree snapshot fingerprint。
///
/// 要求所有后代的 `snapshot_fingerprint` 已经自底向上填充完成。
/// 若子树中存在 `composite_dirty` 的后代，返回 `None`——这些子树每帧 fingerprint 都在抖动，
/// 入 cache 只会污染（一次性 key、consecutive_hits=0、永不再查）。
pub(crate) fn annotated_subtree_snapshot_fingerprint(
    node: &AnnotatedDisplayNode,
    nodes: &[AnnotatedDisplayNode],
    analysis: &DisplayAnalysisTable,
    invalidation: &DisplayInvalidationTable,
    subtree_contains_time_variant: bool,
) -> Option<SubtreeSnapshotFingerprint> {
    if subtree_contains_time_variant {
        return None;
    }

    if subtree_has_dirty_descendant_composite(node, nodes, invalidation) {
        return None;
    }

    let mut primary = new_primary_hasher();
    let mut secondary = new_secondary_hasher();

    hash_node_recorded_paint(node, &mut primary);
    hash_node_recorded_paint(node, &mut secondary);

    node.children.len().hash(&mut primary);
    node.children.len().hash(&mut secondary);

    for &child_handle in &node.children {
        let child = &nodes[child_handle.0];
        hash_node_draw_time_composite(child, &mut primary);
        hash_node_draw_time_composite(child, &mut secondary);

        let child_fp = analysis
            .require(child_handle)
            .snapshot_fingerprint
            .expect("stable annotated child must carry snapshot_fingerprint");
        child_fp.primary.hash(&mut primary);
        child_fp.secondary.hash(&mut secondary);
    }

    Some(SubtreeSnapshotFingerprint {
        primary: primary.finish(),
        secondary: secondary.finish(),
    })
}

/// 子树（**不含** `node` 自身）中是否存在"本帧 composite 跨帧变化"的后代。
///
/// 读 `DisplayInvalidationTable.composite_dirty`——该字段由
/// `mark_display_tree_composite_dirty` 在 pipeline 前段写入，比较前后帧同
/// `RenderNodeKey` 的 `CompositeSig`（translation/transforms/opacity/backdrop_blur）。
///
/// 精准诊断：命中时 hit 几乎不可能为 dirty（key 相同是 composite 稳定的证据）；
/// miss 里 dirty 的部分即"由子 composite 抖动导致的 fingerprint 抖动"，可被
/// "composite-stable only" 新规则救回。非 dirty 的 miss 归因于首次出现或 paint 变化。
pub fn subtree_has_dirty_descendant_composite(
    node: &AnnotatedDisplayNode,
    nodes: &[AnnotatedDisplayNode],
    invalidation: &DisplayInvalidationTable,
) -> bool {
    for &child_handle in &node.children {
        if invalidation
            .get(child_handle)
            .is_some_and(|inv| inv.composite_dirty)
        {
            return true;
        }
        let child = &nodes[child_handle.0];
        if subtree_has_dirty_descendant_composite(child, nodes, invalidation) {
            return true;
        }
    }
    false
}

fn hash_node_recorded_paint<H: Hasher>(node: &AnnotatedDisplayNode, hasher: &mut H) {
    hash_recorded_semantics(&node.recorded_semantics(), hasher);
}

fn hash_node_draw_time_composite<H: Hasher>(node: &AnnotatedDisplayNode, hasher: &mut H) {
    hash_draw_composite_semantics(&node.draw_composite_semantics(), hasher);
}

fn hash_recorded_semantics<H: Hasher>(semantics: &RecordedNodeSemantics<'_>, hasher: &mut H) {
    F32Hash(semantics.bounds.width).hash(hasher);
    F32Hash(semantics.bounds.height).hash(hasher);
    DisplayItemFp(semantics.item).hash(hasher);
    ClipFp(semantics.clip).hash(hasher);
}

fn hash_draw_composite_semantics<H: Hasher>(
    semantics: &DrawCompositeSemantics<'_>,
    hasher: &mut H,
) {
    F32Hash(semantics.transform.translation_x).hash(hasher);
    F32Hash(semantics.transform.translation_y).hash(hasher);
    F32Hash(semantics.opacity).hash(hasher);
    semantics.backdrop_blur_sigma.map(F32Hash).hash(hasher);
    semantics.transform.transforms.hash(hasher);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        display::list::{
            BitmapDisplayItem, BitmapPaintStyle, DisplayClip, DisplayItem, DisplayRect,
            DisplayTransform, DrawScriptDisplayItem, RectDisplayItem, RectPaintStyle,
        },
        resource::asset_id::AssetId,
        runtime::{
            analysis::{
                DisplayAnalysisTable, DisplayInvalidationTable, DisplayNodeAnalysis,
                DisplayNodeInvalidation,
            },
            annotation::{
                AnnotatedDisplayNode, AnnotatedDisplayTree, AnnotatedNodeHandle, RenderNodeKey,
            },
        },
        style::{BorderRadius, ObjectFit, Transform},
    };

    struct AnnotatedRectConfig {
        key: RenderNodeKey,
        transform: DisplayTransform,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        clip: Option<DisplayClip>,
        paint_variance: PaintVariance,
        composite_dirty: bool,
        children: Vec<TestAnnotatedNode>,
        background: Option<crate::style::BackgroundFill>,
    }

    impl Default for AnnotatedRectConfig {
        fn default() -> Self {
            Self {
                key: RenderNodeKey(1),
                transform: rect_transform(0.0, 0.0),
                opacity: 1.0,
                backdrop_blur_sigma: None,
                clip: None,
                paint_variance: PaintVariance::Stable,
                composite_dirty: false,
                children: Vec::new(),
                background: None,
            }
        }
    }

    #[derive(Clone)]
    struct TestAnnotatedNode {
        key: RenderNodeKey,
        transform: DisplayTransform,
        opacity: f32,
        backdrop_blur_sigma: Option<f32>,
        clip: Option<DisplayClip>,
        item: DisplayItem,
        children: Vec<TestAnnotatedNode>,
        paint_variance: PaintVariance,
        composite_dirty: bool,
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
        }
    }

    fn annotated_rect_node(config: AnnotatedRectConfig) -> TestAnnotatedNode {
        TestAnnotatedNode {
            key: config.key,
            transform: config.transform,
            opacity: config.opacity,
            backdrop_blur_sigma: config.backdrop_blur_sigma,
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
                    blur_sigma: None,
                    box_shadow: None,
                    inset_shadow: None,
                    drop_shadow: None,
                },
            }),
            children: config.children,
            paint_variance: config.paint_variance,
            composite_dirty: config.composite_dirty,
        }
    }

    impl TestAnnotatedNode {
        fn into_annotated_node(self) -> AnnotatedDisplayNode {
            AnnotatedDisplayNode {
                transform: self.transform,
                opacity: self.opacity,
                backdrop_blur_sigma: self.backdrop_blur_sigma,
                clip: self.clip,
                item: self.item,
                children: Vec::new(),
            }
        }
    }

    fn finalize_annotated_tree(node: TestAnnotatedNode) -> AnnotatedDisplayTree {
        let mut nodes = Vec::new();
        let mut keys = Vec::new();
        let mut layer_bounds = Vec::new();
        let mut analysis = DisplayAnalysisTable::default();
        let mut invalidation = DisplayInvalidationTable::default();
        let root = finalize_test_node(
            node,
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

    fn finalize_test_node(
        node: TestAnnotatedNode,
        nodes: &mut Vec<AnnotatedDisplayNode>,
        keys: &mut Vec<RenderNodeKey>,
        layer_bounds: &mut Vec<DisplayRect>,
        analysis: &mut DisplayAnalysisTable,
        invalidation: &mut DisplayInvalidationTable,
    ) -> AnnotatedNodeHandle {
        let children = node
            .children
            .into_iter()
            .map(|child| {
                finalize_test_node(child, nodes, keys, layer_bounds, analysis, invalidation)
            })
            .collect::<Vec<_>>();

        let handle = AnnotatedNodeHandle(nodes.len());
        let annotated = AnnotatedDisplayNode {
            transform: node.transform,
            opacity: node.opacity,
            backdrop_blur_sigma: node.backdrop_blur_sigma,
            clip: node.clip,
            item: node.item,
            children,
        };

        let subtree_contains_time_variant =
            matches!(node.paint_variance, PaintVariance::TimeVariant)
                || annotated.children.iter().any(|&child_handle| {
                    analysis.require(child_handle).subtree_contains_time_variant
                });

        let mut node_analysis = DisplayNodeAnalysis {
            paint_variance: node.paint_variance,
            subtree_contains_time_variant,
            paint_fingerprint: None,
            snapshot_fingerprint: None,
        };
        if !subtree_contains_time_variant {
            node_analysis.paint_fingerprint = annotated_subtree_paint_fingerprint(
                &annotated,
                analysis,
                subtree_contains_time_variant,
            );
            node_analysis.snapshot_fingerprint = annotated_subtree_snapshot_fingerprint(
                &annotated,
                nodes,
                analysis,
                invalidation,
                subtree_contains_time_variant,
            );
        }
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
        invalidation.insert(
            handle,
            DisplayNodeInvalidation {
                composite_dirty: node.composite_dirty,
            },
        );

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
    fn descendant_composite_dirty_detection_reads_invalidation_table() {
        // 契约：`subtree_has_dirty_descendant_composite` 递归读 `DisplayInvalidationTable`
        // 的 `composite_dirty` 字段。只返回"实际跨帧变化"的信号，不把"恒定非零 composite"
        // 误判为 dirty。

        // 1. 单节点、无后代 → false
        let leaf = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig::default()));
        assert!(
            !subtree_has_dirty_descendant_composite(
                leaf.root_node(),
                &leaf.nodes,
                &leaf.invalidation,
            ),
            "leaf with no descendants must be non-dirty"
        );

        // 2. 后代虽有非零 translation 但 composite_dirty=false（恒定位移）→ false
        //    与之前的 non-identity 上界版本的关键差别。
        let stable_translating_child = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            transform: rect_transform(100.0, 200.0),
            composite_dirty: false,
            ..Default::default()
        });
        let stable = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![stable_translating_child],
            ..Default::default()
        }));
        assert!(
            !subtree_has_dirty_descendant_composite(
                stable.root_node(),
                &stable.nodes,
                &stable.invalidation,
            ),
            "constant non-zero translation must NOT be marked dirty"
        );

        // 3. 后代 composite_dirty=true → true
        let dirty_child = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            composite_dirty: true,
            ..Default::default()
        });
        let dirty = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![dirty_child],
            ..Default::default()
        }));
        assert!(
            subtree_has_dirty_descendant_composite(
                dirty.root_node(),
                &dirty.nodes,
                &dirty.invalidation,
            ),
            "dirty direct descendant must be detected"
        );

        // 4. 深层 dirty 孙节点 → true（必须递归穿过 clean 的中间节点）
        let dirty_grandchild = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(3),
            composite_dirty: true,
            ..Default::default()
        });
        let clean_middle = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            composite_dirty: false,
            children: vec![dirty_grandchild],
            ..Default::default()
        });
        let deep = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![clean_middle],
            ..Default::default()
        }));
        assert!(
            subtree_has_dirty_descendant_composite(
                deep.root_node(),
                &deep.nodes,
                &deep.invalidation,
            ),
            "dirty grandchild must be detected via recursion through clean middle"
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
        transform_a.transforms = vec![Transform::Scale(1.0)];
        let mut transform_b = rect_transform(0.0, 0.0);
        transform_b.transforms = vec![Transform::Scale(2.0)];
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
            background: Some(crate::style::BackgroundFill::Solid(
                crate::style::ColorToken::Red,
            )),
            ..Default::default()
        }));
        let fp_a = a.analysis(a.root).paint_fingerprint;
        let fp_b = b.analysis(b.root).paint_fingerprint;
        assert_ne!(fp_a, fp_b, "different paint content must differ");
    }

    #[test]
    fn composite_signature_tracks_all_draw_time_composite_semantics() {
        let a = annotated_rect_node(AnnotatedRectConfig {
            transform: rect_transform(10.0, 20.0),
            opacity: 1.0,
            ..Default::default()
        })
        .into_annotated_node();
        let mut transform_b = rect_transform(10.0, 20.0);
        transform_b.transforms = vec![Transform::Scale(1.25)];
        let b = annotated_rect_node(AnnotatedRectConfig {
            transform: transform_b,
            opacity: 0.5,
            backdrop_blur_sigma: Some(6.0),
            ..Default::default()
        })
        .into_annotated_node();
        let sig_a = CompositeSig::from_annotated_node(&a);
        let sig_b = CompositeSig::from_annotated_node(&b);
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn snapshot_fingerprint_ignores_current_node_transform() {
        let mut transform_a = rect_transform(0.0, 0.0);
        transform_a.transforms = vec![Transform::Scale(1.0)];
        let mut transform_b = rect_transform(0.0, 0.0);
        transform_b.transforms = vec![Transform::Scale(2.0)];
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
    fn snapshot_fingerprint_tracks_descendant_transform_changes() {
        let mut a = annotated_rect_node(AnnotatedRectConfig::default());
        let mut b = annotated_rect_node(AnnotatedRectConfig::default());
        let mut child_transform_a = rect_transform(0.0, 0.0);
        child_transform_a.transforms = vec![Transform::Scale(1.0)];
        let mut child_transform_b = rect_transform(0.0, 0.0);
        child_transform_b.transforms = vec![Transform::Scale(2.0)];
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
        assert_ne!(
            fp_a, fp_b,
            "descendant transform is baked into the parent snapshot and must affect the key"
        );
    }

    #[test]
    fn snapshot_fingerprint_returns_none_for_dirty_descendant() {
        // 直接后代 composite_dirty → 父节点 snapshot_fingerprint = None
        let dirty_child = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            composite_dirty: true,
            ..Default::default()
        });
        let tree = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![dirty_child],
            ..Default::default()
        }));
        assert!(
            tree.analysis(tree.root).snapshot_fingerprint.is_none(),
            "parent with dirty descendant must have no snapshot_fingerprint"
        );
        // paint fingerprint 不受 composite_dirty 影响
        assert!(
            tree.analysis(tree.root).paint_fingerprint.is_some(),
            "paint fingerprint is independent of composite dirty"
        );

        // 深层 dirty 孙节点 → 所有祖先 snapshot_fingerprint = None
        let dirty_grandchild = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(3),
            composite_dirty: true,
            ..Default::default()
        });
        let clean_middle = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            composite_dirty: false,
            children: vec![dirty_grandchild],
            ..Default::default()
        });
        let deep = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![clean_middle],
            ..Default::default()
        }));
        assert!(
            deep.analysis(deep.root).snapshot_fingerprint.is_none(),
            "root with dirty grandchild must have no snapshot_fingerprint"
        );

        // clean 树 → snapshot_fingerprint = Some
        let clean_child = annotated_rect_node(AnnotatedRectConfig {
            key: RenderNodeKey(2),
            composite_dirty: false,
            ..Default::default()
        });
        let clean = finalize_annotated_tree(annotated_rect_node(AnnotatedRectConfig {
            children: vec![clean_child],
            ..Default::default()
        }));
        assert!(
            clean.analysis(clean.root).snapshot_fingerprint.is_some(),
            "clean tree must have snapshot_fingerprint"
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
    fn bitmap_time_variant_for_video_asset() {
        let asset_id = AssetId("/tmp/fake.mp4".into());
        let bitmap_item = DisplayItem::Bitmap(BitmapDisplayItem {
            bounds: empty_bounds(),
            asset_id,
            width: 10,
            height: 10,
            video_timing: Some(crate::resource::types::VideoFrameTiming::default()),
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
        });
        assert_eq!(
            classify_paint(&bitmap_item),
            PaintVariance::TimeVariant
        );
        // 视频 Bitmap 不再进 ItemPictureCache —— 解码层的 video_frame_cache 已按
        // pts 复用帧数据,Picture 层冻帧会破坏"视频视为变化"的设计原则。
        assert!(
            item_paint_fingerprint(&bitmap_item).is_none(),
            "video bitmap 不应有 paint fingerprint,避免 ItemPictureCache 冻帧"
        );
    }

    #[test]
    fn draw_script_command_based_stability() {
        let script_item = DisplayItem::DrawScript(DrawScriptDisplayItem {
            bounds: empty_bounds(),
            commands: Vec::new(),
            drop_shadow: None,
        });

        // 命令序列空 → hash 稳定 → Stable
        assert_eq!(classify_paint(&script_item), PaintVariance::Stable);
        assert!(
            item_paint_fingerprint(&script_item).is_some(),
            "Stable DrawScript 必须有 paint fingerprint 作为 ItemPictureCache 的 key"
        );
    }

    #[test]
    fn subtree_snapshot_fingerprint_primary_and_secondary_are_independent() {
        use std::hash::Hasher;

        let mut primary = new_primary_hasher();
        let mut secondary = new_secondary_hasher();
        primary.write_u64(0xdead_beef);
        secondary.write_u64(0xdead_beef);

        let fp = SubtreeSnapshotFingerprint {
            primary: primary.finish(),
            secondary: secondary.finish(),
        };

        assert_ne!(
            fp.primary, fp.secondary,
            "两个独立 hasher 在相同输入下必须产出不同结果"
        );
    }

    #[test]
    fn primary_and_secondary_hashers_stay_independent() {
        use std::hash::Hasher;

        let mut primary = new_primary_hasher();
        let mut secondary = new_secondary_hasher();
        primary.write_u64(0xfeed_face);
        secondary.write_u64(0xfeed_face);

        assert_ne!(primary.finish(), secondary.finish());
    }

    #[test]
    fn video_bitmap_paint_fingerprint_is_none() {
        let asset_id = AssetId("/tmp/fake.mp4".into());

        let item = DisplayItem::Bitmap(BitmapDisplayItem {
            bounds: empty_bounds(),
            asset_id,
            width: 10,
            height: 10,
            video_timing: Some(crate::resource::types::VideoFrameTiming {
                media_offset_secs: 1.234,
                playback_rate: 1.0,
                looping: false,
            }),
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
        });

        assert!(
            item_paint_fingerprint(&item).is_none(),
            "视频 bitmap 绕过 ItemPictureCache,保证每帧重新走 draw_bitmap 解码路径"
        );
    }

    #[test]
    fn video_bitmap_paint_variance_stays_time_variant() {
        let asset_id = AssetId("/tmp/fake.mp4".into());

        let item = DisplayItem::Bitmap(BitmapDisplayItem {
            bounds: empty_bounds(),
            asset_id,
            width: 10,
            height: 10,
            video_timing: Some(crate::resource::types::VideoFrameTiming::default()),
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
        });

        assert_eq!(
            classify_paint(&item),
            PaintVariance::TimeVariant,
            "Video Bitmap 在子树层面仍是 TimeVariant,避免父子树 snapshot 误命中"
        );
    }
}
