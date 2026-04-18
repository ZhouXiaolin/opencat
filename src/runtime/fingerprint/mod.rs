//! Paint、subtree snapshot 与 composite 三个独立维度的指纹。
//!
//! - [`subtree_paint_fingerprint`]：纯 paint 指纹。仅由"画什么"决定，不含任何 composite。
//! - [`subtree_snapshot_fingerprint`]：subtree picture 缓存键。
//!   不含当前节点自己的 composite，但递归包含所有后代 composite，因为后代会被烘焙进当前节点 picture。
//! - [`composite_signature`]：每帧比对用的合成参数摘要（transform/opacity/blur），
//!   **不进入缓存键**。
//! - [`classify_paint`]：判定单个 DisplayItem 的 paint variance。
//!
//! 这个模块是纯函数、无副作用、无状态、不依赖 profile。

mod display_item;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::{
    display::{list::DisplayItem, tree::DisplayNode},
    resource::assets::AssetsMap,
};

use display_item::{ClipFp, DisplayItemFp, F32Hash, TextFp, item_is_time_variant};

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
    pub fn from_node(node: &DisplayNode) -> Self {
        let mut transforms_hasher = DefaultHasher::new();
        node.transform.transforms.hash(&mut transforms_hasher);
        Self {
            translation_x_bits: node.transform.translation_x.to_bits(),
            translation_y_bits: node.transform.translation_y.to_bits(),
            transforms_hash: transforms_hasher.finish(),
            opacity_bits: node.opacity.to_bits(),
            backdrop_blur_bits: node.backdrop_blur_sigma.map(|v| v.to_bits()),
        }
    }
}

/// 判定单个 DisplayItem 的 paint variance。
pub fn classify_paint(item: &DisplayItem, assets: &AssetsMap) -> PaintVariance {
    if item_is_time_variant(item, assets) {
        PaintVariance::TimeVariant
    } else {
        PaintVariance::Stable
    }
}

/// 计算文字项的 paint fingerprint。
pub fn text_paint_fingerprint(text: &crate::display::list::TextDisplayItem) -> u64 {
    calculate_hash(&TextFp(text))
}

/// 计算单个 DisplayItem 的 paint fingerprint。
pub fn item_paint_fingerprint(item: &DisplayItem, assets: &AssetsMap) -> Option<u64> {
    if item_is_time_variant(item, assets) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    DisplayItemFp(item).hash(&mut hasher);
    Some(hasher.finish())
}

/// 计算子树 paint fingerprint。
pub fn subtree_paint_fingerprint(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    if node_contains_time_variant(node, assets) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    hash_subtree_paint(node, &mut hasher);
    Some(hasher.finish())
}

/// 计算 subtree snapshot fingerprint。
pub fn subtree_snapshot_fingerprint(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    if node_contains_time_variant(node, assets) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    hash_subtree_snapshot(node, &mut hasher);
    Some(hasher.finish())
}

/// 计算视频场景的静态骨架指纹。
pub fn scene_static_skeleton_fingerprint(node: &DisplayNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_scene_static_skeleton(node, &mut hasher);
    hasher.finish()
}

fn node_contains_time_variant(node: &DisplayNode, assets: &AssetsMap) -> bool {
    item_is_time_variant(&node.item, assets)
        || node
            .children
            .iter()
            .any(|child| node_contains_time_variant(child, assets))
}

fn hash_subtree_paint(node: &DisplayNode, hasher: &mut DefaultHasher) {
    F32Hash(node.transform.bounds.width).hash(hasher);
    F32Hash(node.transform.bounds.height).hash(hasher);
    DisplayItemFp(&node.item).hash(hasher);
    ClipFp(node.clip.as_ref()).hash(hasher);
    node.children.len().hash(hasher);
    for child in &node.children {
        hash_subtree_paint(child, hasher);
    }
}

fn hash_subtree_snapshot(node: &DisplayNode, hasher: &mut DefaultHasher) {
    F32Hash(node.transform.bounds.width).hash(hasher);
    F32Hash(node.transform.bounds.height).hash(hasher);
    DisplayItemFp(&node.item).hash(hasher);
    ClipFp(node.clip.as_ref()).hash(hasher);
    node.children.len().hash(hasher);

    for child in &node.children {
        F32Hash(child.transform.translation_x).hash(hasher);
        F32Hash(child.transform.translation_y).hash(hasher);
        F32Hash(child.opacity).hash(hasher);
        child.backdrop_blur_sigma.map(F32Hash).hash(hasher);
        child.transform.transforms.hash(hasher);
        hash_subtree_snapshot(child, hasher);
    }
}

const TIME_VARIANT_SENTINEL: u64 = 0x5449_4d45_5641_5254;

fn hash_scene_static_skeleton(node: &DisplayNode, hasher: &mut DefaultHasher) {
    if node.paint_variance == PaintVariance::TimeVariant || node.composite_dirty {
        hash_time_variant_sentinel(node, hasher);
        return;
    }

    F32Hash(node.transform.bounds.width).hash(hasher);
    F32Hash(node.transform.bounds.height).hash(hasher);
    DisplayItemFp(&node.item).hash(hasher);
    ClipFp(node.clip.as_ref()).hash(hasher);
    node.children.len().hash(hasher);

    for child in &node.children {
        if child.subtree_contains_dynamic {
            hash_time_variant_sentinel(child, hasher);
        } else {
            F32Hash(child.transform.translation_x).hash(hasher);
            F32Hash(child.transform.translation_y).hash(hasher);
            F32Hash(child.opacity).hash(hasher);
            child.backdrop_blur_sigma.map(F32Hash).hash(hasher);
            child.transform.transforms.hash(hasher);
            hash_scene_static_skeleton(child, hasher);
        }
    }
}

fn hash_time_variant_sentinel(node: &DisplayNode, hasher: &mut DefaultHasher) {
    TIME_VARIANT_SENTINEL.hash(hasher);
    F32Hash(node.transform.bounds.width).hash(hasher);
    F32Hash(node.transform.bounds.height).hash(hasher);
    ClipFp(node.clip.as_ref()).hash(hasher);
    F32Hash(node.transform.translation_x).hash(hasher);
    F32Hash(node.transform.translation_y).hash(hasher);
    F32Hash(node.opacity).hash(hasher);
    node.backdrop_blur_sigma.map(F32Hash).hash(hasher);
    node.transform.transforms.hash(hasher);
}

fn calculate_hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        display::{
            list::{
                BitmapDisplayItem, BitmapPaintStyle, DisplayItem, DisplayRect, DisplayTransform,
                RectDisplayItem, RectPaintStyle,
            },
            tree::DisplayNode,
        },
        resource::assets::AssetsMap,
        style::{BorderRadius, ObjectFit, Transform},
    };

    fn empty_bounds() -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    fn rect_node(translation_x: f32, translation_y: f32, opacity: f32) -> DisplayNode {
        DisplayNode {
            transform: DisplayTransform {
                translation_x,
                translation_y,
                bounds: empty_bounds(),
                transforms: Vec::new(),
            },
            opacity,
            backdrop_blur_sigma: None,
            clip: None,
            item: DisplayItem::Rect(RectDisplayItem {
                bounds: empty_bounds(),
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
            children: Vec::new(),
            snapshot_fingerprint: None,
            paint_variance: PaintVariance::Stable,
            composite_dirty: false,
            subtree_contains_time_variant: false,
            subtree_contains_dynamic: false,
        }
    }

    #[test]
    fn paint_fingerprint_is_invariant_under_translation() {
        let assets = AssetsMap::new();
        let a = rect_node(10.0, 20.0, 1.0);
        let b = rect_node(50.0, 80.0, 1.0);
        let fp_a = subtree_paint_fingerprint(&a, &assets);
        let fp_b = subtree_paint_fingerprint(&b, &assets);
        assert_eq!(fp_a, fp_b, "translation must not affect paint fingerprint");
        assert!(fp_a.is_some());
    }

    #[test]
    fn paint_fingerprint_is_invariant_under_opacity() {
        let assets = AssetsMap::new();
        let a = rect_node(0.0, 0.0, 1.0);
        let b = rect_node(0.0, 0.0, 0.3);
        let fp_a = subtree_paint_fingerprint(&a, &assets);
        let fp_b = subtree_paint_fingerprint(&b, &assets);
        assert_eq!(fp_a, fp_b, "opacity must not affect paint fingerprint");
    }

    #[test]
    fn paint_fingerprint_is_invariant_under_transforms() {
        let assets = AssetsMap::new();
        let mut a = rect_node(0.0, 0.0, 1.0);
        let mut b = rect_node(0.0, 0.0, 1.0);
        a.transform.transforms = vec![Transform::Scale(1.0)];
        b.transform.transforms = vec![Transform::Scale(2.0)];
        let fp_a = subtree_paint_fingerprint(&a, &assets);
        let fp_b = subtree_paint_fingerprint(&b, &assets);
        assert_eq!(fp_a, fp_b, "transforms must not affect paint fingerprint");
    }

    #[test]
    fn paint_fingerprint_changes_with_paint_content() {
        let assets = AssetsMap::new();
        let a = rect_node(0.0, 0.0, 1.0);
        let mut b = rect_node(0.0, 0.0, 1.0);
        if let DisplayItem::Rect(ref mut r) = b.item {
            r.paint.background = Some(crate::style::BackgroundFill::Solid(
                crate::style::ColorToken::Red,
            ));
        }
        let fp_a = subtree_paint_fingerprint(&a, &assets);
        let fp_b = subtree_paint_fingerprint(&b, &assets);
        assert_ne!(fp_a, fp_b, "different paint content must differ");
    }

    #[test]
    fn composite_signature_tracks_translation_and_opacity() {
        let a = rect_node(10.0, 20.0, 1.0);
        let b = rect_node(50.0, 80.0, 0.5);
        let sig_a = CompositeSig::from_node(&a);
        let sig_b = CompositeSig::from_node(&b);
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn snapshot_fingerprint_ignores_current_node_transform() {
        let assets = AssetsMap::new();
        let mut a = rect_node(0.0, 0.0, 1.0);
        let mut b = rect_node(0.0, 0.0, 1.0);
        a.transform.transforms = vec![Transform::Scale(1.0)];
        b.transform.transforms = vec![Transform::Scale(2.0)];

        let fp_a = subtree_snapshot_fingerprint(&a, &assets);
        let fp_b = subtree_snapshot_fingerprint(&b, &assets);
        assert_eq!(
            fp_a, fp_b,
            "current node transform is applied outside its snapshot and must not bust the key"
        );
    }

    #[test]
    fn snapshot_fingerprint_tracks_descendant_transform_changes() {
        let assets = AssetsMap::new();
        let mut a = rect_node(0.0, 0.0, 1.0);
        let mut b = rect_node(0.0, 0.0, 1.0);
        let mut child_a = rect_node(0.0, 0.0, 1.0);
        let mut child_b = rect_node(0.0, 0.0, 1.0);
        child_a.transform.transforms = vec![Transform::Scale(1.0)];
        child_b.transform.transforms = vec![Transform::Scale(2.0)];
        a.children.push(child_a);
        b.children.push(child_b);

        let fp_a = subtree_snapshot_fingerprint(&a, &assets);
        let fp_b = subtree_snapshot_fingerprint(&b, &assets);
        assert_ne!(
            fp_a, fp_b,
            "descendant transform is baked into the parent snapshot and must affect the key"
        );
    }

    #[test]
    fn bitmap_time_variant_for_video_asset() {
        let mut assets = AssetsMap::new();
        let video_path = std::path::PathBuf::from("/tmp/fake.mp4");
        let asset_id = assets.register_dimensions(&video_path, 10, 10);
        let bitmap_item = DisplayItem::Bitmap(BitmapDisplayItem {
            bounds: empty_bounds(),
            asset_id,
            width: 10,
            height: 10,
            video_timing: None,
            object_fit: ObjectFit::Fill,
            paint: BitmapPaintStyle {
                background: None,
                border_radius: BorderRadius::default(),
                border_width: None,
                border_color: None,
                blur_sigma: None,
                box_shadow: None,
                inset_shadow: None,
                drop_shadow: None,
            },
        });
        assert_eq!(
            classify_paint(&bitmap_item, &assets),
            PaintVariance::TimeVariant
        );
        assert_eq!(item_paint_fingerprint(&bitmap_item, &assets), None);
    }

    #[test]
    fn scene_static_skeleton_fingerprint_ignores_time_variant_paint_but_tracks_composite() {
        let mut a = rect_node(0.0, 0.0, 1.0);
        let mut b = rect_node(0.0, 0.0, 1.0);
        let mut c = rect_node(0.0, 0.0, 1.0);

        let mut dynamic_a = rect_node(10.0, 20.0, 0.9);
        dynamic_a.paint_variance = PaintVariance::TimeVariant;
        dynamic_a.subtree_contains_time_variant = true;

        let mut dynamic_b = rect_node(10.0, 20.0, 0.9);
        dynamic_b.paint_variance = PaintVariance::TimeVariant;
        dynamic_b.subtree_contains_time_variant = true;
        if let DisplayItem::Rect(ref mut rect) = dynamic_b.item {
            rect.paint.background = Some(crate::style::BackgroundFill::Solid(
                crate::style::ColorToken::Red,
            ));
        }

        let mut dynamic_c = rect_node(30.0, 20.0, 0.9);
        dynamic_c.paint_variance = PaintVariance::TimeVariant;
        dynamic_c.subtree_contains_time_variant = true;

        a.children.push(dynamic_a);
        b.children.push(dynamic_b);
        c.children.push(dynamic_c);

        assert_eq!(
            scene_static_skeleton_fingerprint(&a),
            scene_static_skeleton_fingerprint(&b),
            "dynamic subtree paint must not affect the static skeleton fingerprint"
        );
        assert_ne!(
            scene_static_skeleton_fingerprint(&a),
            scene_static_skeleton_fingerprint(&c),
            "dynamic subtree composite must affect the static skeleton fingerprint"
        );
    }
}
