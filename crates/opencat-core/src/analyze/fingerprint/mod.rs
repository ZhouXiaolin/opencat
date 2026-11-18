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
    display::{
        list::{DisplayClip, DisplayItem},
        tree::{DisplayNode, DisplayRecordedSubtreeFingerprint, DisplayTree, HiddenChildDisplayNode},
    },
    layout::tree::LayoutOutputFingerprint,
};

use crate::fp::FpF32;
use display_item::DisplayItemFp;

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
            FpF32(clip.bounds.x).hash(&mut hasher);
            FpF32(clip.bounds.y).hash(&mut hasher);
            FpF32(clip.bounds.width).hash(&mut hasher);
            FpF32(clip.bounds.height).hash(&mut hasher);
            clip.border_radius.hash(&mut hasher);
        }
        Self(hasher.finish())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisplayRecordedFingerprint(pub u64);

impl DisplayRecordedFingerprint {
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
    node.mask_image.hash(&mut hasher);
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

/// 自底向上填充整棵显示树的子树 paint / snapshot 指纹（写回
/// `DisplayNode.paint_fingerprint` / `snapshot_fingerprint`）。
pub fn compute_display_tree_fingerprints(tree: &mut DisplayTree) {
    compute_node_fingerprints(&mut tree.root);
}

fn compute_node_fingerprints(node: &mut DisplayNode) {
    let mut child_paints = Vec::with_capacity(node.children.len());
    let mut child_snapshots = Vec::with_capacity(node.children.len());
    for child in &mut node.children {
        compute_node_fingerprints(child);
        debug_assert!(
            child.paint_fingerprint.is_some()
                && child.snapshot_fingerprint.is_some(),
            "invariant: child subtree must carry both fingerprints after recursion"
        );
        child_paints.push(child.paint_fingerprint);
        child_snapshots.push(child.snapshot_fingerprint);
    }

    node.paint_fingerprint = subtree_paint_fingerprint(node, &child_paints);
    node.snapshot_fingerprint = subtree_snapshot_fingerprint(node, &child_snapshots);
}

/// 子树 paint 指纹：自身 recorded paint + 各子节点 paint 指纹（声明序）。
fn subtree_paint_fingerprint(node: &DisplayNode, child_paints: &[Option<u64>]) -> Option<u64> {
    let mut hasher = new_hasher();
    hash_node_recorded_paint(node, &mut hasher);
    node.children.len().hash(&mut hasher);
    for child_fp in child_paints {
        child_fp.unwrap_or(0).hash(&mut hasher);
    }
    Some(hasher.finish())
}

/// 子树 picture 缓存键指纹：仅 hash paint 内容（节点自身 paint + 子节点
/// snapshot_fingerprint 递归），不含任何 composite（transform/opacity/blur）
/// ——composite 在 replay 时由 render dispatch 动态 apply。
fn subtree_snapshot_fingerprint(
    node: &DisplayNode,
    child_snapshots: &[Option<SubtreeSnapshotFingerprint>],
) -> Option<SubtreeSnapshotFingerprint> {
    let mut hasher = new_hasher();
    hash_node_recorded_paint(node, &mut hasher);
    node.children.len().hash(&mut hasher);
    for child_fp in child_snapshots {
        child_fp
            .unwrap_or(SubtreeSnapshotFingerprint(0))
            .0
            .hash(&mut hasher);
    }
    Some(SubtreeSnapshotFingerprint(hasher.finish()))
}

fn hash_node_recorded_paint<H: Hasher>(node: &DisplayNode, hasher: &mut H) {
    node.input_fingerprints.paint_input_subtree.hash(hasher);
    DisplayRecordedFingerprint::from_display_node(node).hash(hasher);
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
    FpF32(node.transform.translation_x).hash(hasher);
    FpF32(node.transform.translation_y).hash(hasher);
    FpF32(node.transform.origin.x).hash(hasher);
    FpF32(node.transform.origin.y).hash(hasher);
    FpF32(node.transform.origin.x_px).hash(hasher);
    FpF32(node.transform.origin.y_px).hash(hasher);
    FpF32(node.opacity).hash(hasher);
    node.css_filter_image.hash(hasher);
    node.backdrop_blur_sigma.map(FpF32).hash(hasher);
    node.backdrop_filter_image.hash(hasher);
    node.transform.transforms.hash(hasher);
    node.transform.backface_hidden.hash(hasher);
    node.transform
        .rotation_m4
        .map(|m| m.0.map(FpF32))
        .hash(hasher);
}
