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


