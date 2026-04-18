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

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::{
    display::list::DisplayItem,
    resource::assets::AssetsMap,
    runtime::{
        analysis::DisplayAnalysisTable,
        annotation::{
            AnnotatedDisplayNode, DrawCompositeSemantics, RecordedNodeSemantics,
        },
    },
};

use display_item::{
    BitmapPaintFp, ClipFp, DisplayItemFp, F32Hash, TextFp, bitmap_is_video, item_is_time_variant,
};

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
        let mut transforms_hasher = DefaultHasher::new();
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

/// 计算单个 DisplayItem 的 paint fingerprint(作为 `ItemPictureCache` key)。
///
/// 语义:
/// - 非 video Bitmap 的 TimeVariant 项 → None(不进 cache)
/// - Video Bitmap → Some(hash 中 pts 按 1/10000 秒量化),暂停段内多帧命中
/// - Stable 项 → Some(基于 `DisplayItemFp` 全量 hash)
///
/// 注:Video Bitmap 返回 Some 与 `classify_paint` 仍返回 TimeVariant **不矛盾**。
/// Classify 用于决定子树级 paint_fingerprint 是否计算(TimeVariant 不计算),
/// 本函数用于单节点 ItemPictureCache 命中决策 —— 两套机制独立。
pub fn item_paint_fingerprint(item: &DisplayItem, assets: &AssetsMap) -> Option<u64> {
    if let DisplayItem::Bitmap(bitmap) = item
        && bitmap_is_video(bitmap, assets)
    {
        return Some(video_bitmap_quantized_fingerprint(bitmap));
    }
    if item_is_time_variant(item, assets) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    DisplayItemFp(item).hash(&mut hasher);
    Some(hasher.finish())
}

fn video_bitmap_quantized_fingerprint(bitmap: &crate::display::list::BitmapDisplayItem) -> u64 {
    use crate::runtime::cache::video_frames::quantize_pts;
    let mut hasher = DefaultHasher::new();
    // Prefix marker:区分 video bitmap fingerprint 与非 video 的 hash 空间,避免碰撞。
    0xF0_u8.hash(&mut hasher);
    bitmap.asset_id.hash(&mut hasher);
    bitmap.width.hash(&mut hasher);
    bitmap.height.hash(&mut hasher);
    bitmap.object_fit.hash(&mut hasher);
    F32Hash(bitmap.bounds.width).hash(&mut hasher);
    F32Hash(bitmap.bounds.height).hash(&mut hasher);
    BitmapPaintFp(&bitmap.paint).hash(&mut hasher);
    if let Some(timing) = &bitmap.video_timing {
        quantize_pts(timing.media_offset_secs).hash(&mut hasher);
        timing.playback_rate.to_bits().hash(&mut hasher);
        timing.looping.hash(&mut hasher);
    }
    hasher.finish()
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
    let mut hasher = DefaultHasher::new();
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
pub(crate) fn annotated_subtree_snapshot_fingerprint(
    node: &AnnotatedDisplayNode,
    nodes: &[AnnotatedDisplayNode],
    analysis: &DisplayAnalysisTable,
    subtree_contains_time_variant: bool,
) -> Option<SubtreeSnapshotFingerprint> {
    if subtree_contains_time_variant {
        return None;
    }

    let mut primary = DefaultHasher::new();
    let mut secondary = ahash::AHasher::default();

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

fn calculate_hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        display::list::{
            BitmapDisplayItem, BitmapPaintStyle, DisplayClip, DisplayItem, DisplayRect,
            DisplayTransform, DrawScriptDisplayItem, RectDisplayItem, RectPaintStyle,
        },
        resource::assets::AssetsMap,
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
                    border_color: None,
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
            .map(|child| finalize_test_node(child, nodes, keys, layer_bounds, analysis, invalidation))
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
                || annotated
                    .children
                    .iter()
                    .any(|&child_handle| analysis.require(child_handle).subtree_contains_time_variant);

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
        // Video Bitmap 现在通过量化 fingerprint 进入 ItemPictureCache,
        // 即使 video_timing 为 None(仍返回 Some,只是不 hash timing 字段)。
        assert!(
            item_paint_fingerprint(&bitmap_item, &assets).is_some(),
            "video bitmap 必须有 paint fingerprint 以支持暂停段 ItemPictureCache 复用"
        );
    }

    #[test]
    fn draw_script_command_based_stability() {
        let assets = AssetsMap::new();
        let script_item = DisplayItem::DrawScript(DrawScriptDisplayItem {
            bounds: empty_bounds(),
            commands: Vec::new(),
            drop_shadow: None,
        });

        // 命令序列空 → hash 稳定 → Stable
        assert_eq!(
            classify_paint(&script_item, &assets),
            PaintVariance::Stable
        );
        assert!(
            item_paint_fingerprint(&script_item, &assets).is_some(),
            "Stable DrawScript 必须有 paint fingerprint 作为 ItemPictureCache 的 key"
        );
    }

    #[test]
    fn subtree_snapshot_fingerprint_primary_and_secondary_are_independent() {
        use std::hash::Hasher;

        let mut primary = std::collections::hash_map::DefaultHasher::new();
        let mut secondary = ahash::AHasher::default();
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
    fn video_bitmap_paint_fingerprint_stable_within_quantized_pts() {
        let mut assets = AssetsMap::new();
        let video_path = std::path::PathBuf::from("/tmp/fake.mp4");
        let asset_id = assets.register_dimensions(&video_path, 10, 10);

        let make_item = |offset: f64| {
            DisplayItem::Bitmap(BitmapDisplayItem {
                bounds: empty_bounds(),
                asset_id: asset_id.clone(),
                width: 10,
                height: 10,
                video_timing: Some(crate::resource::media::VideoFrameTiming {
                    media_offset_secs: offset,
                    playback_rate: 1.0,
                    looping: false,
                }),
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
            })
        };

        let fp_a = item_paint_fingerprint(&make_item(1.234_560_1), &assets);
        let fp_b = item_paint_fingerprint(&make_item(1.234_560_2), &assets);
        let fp_c = item_paint_fingerprint(&make_item(1.235_0), &assets);

        assert!(fp_a.is_some(), "video bitmap 必须有 paint fingerprint");
        assert_eq!(
            fp_a, fp_b,
            "同一量化 pts(1/10000 秒精度)内两次采样必须 fingerprint 相同"
        );
        assert_ne!(
            fp_a, fp_c,
            "跨量化边界的两次采样必须 fingerprint 不同"
        );
    }

    #[test]
    fn video_bitmap_paint_variance_stays_time_variant() {
        let mut assets = AssetsMap::new();
        let video_path = std::path::PathBuf::from("/tmp/fake.mp4");
        let asset_id = assets.register_dimensions(&video_path, 10, 10);

        let item = DisplayItem::Bitmap(BitmapDisplayItem {
            bounds: empty_bounds(),
            asset_id,
            width: 10,
            height: 10,
            video_timing: Some(crate::resource::media::VideoFrameTiming::default()),
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
            classify_paint(&item, &assets),
            PaintVariance::TimeVariant,
            "Video Bitmap 在子树层面仍是 TimeVariant,避免父子树 snapshot 误命中"
        );
    }

}
