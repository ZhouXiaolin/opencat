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

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::{
    display::{
        list::{BitmapDisplayItem, DisplayClip, DisplayItem, TextDisplayItem},
        tree::DisplayNode,
    },
    resource::{
        assets::{AssetId, AssetsMap},
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
    },
    scene::script::CanvasCommand,
    style::ComputedTextStyle,
};

// ---------- Public API ----------

/// 每个节点的 paint variance 分类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintVariance {
    /// 画面内容跨帧稳定（除非 paint_fingerprint 变）。
    Stable,
    /// 画面内容每帧都可能变（video、含 video 的 draw script）。
    TimeVariant,
}

/// 合成参数摘要：transform、opacity、blur、save_layer 标志等。
///
/// **不进入缓存键**。每帧对同一节点比对，用来判断是否需要重新合成（但 paint 可复用）。
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
///
/// 视频 bitmap、含 video 引用的 draw script → TimeVariant；其余 → Stable。
pub fn classify_paint(item: &DisplayItem, assets: &AssetsMap) -> PaintVariance {
    if item_is_time_variant(item, assets) {
        PaintVariance::TimeVariant
    } else {
        PaintVariance::Stable
    }
}

/// 计算文字项的 paint fingerprint（仅 text 内容 + 样式 + bounds size）。
pub fn text_paint_fingerprint(text: &TextDisplayItem) -> u64 {
    calculate_hash(&TextFp(text))
}

/// 计算单个 DisplayItem 的 paint fingerprint（**不含** transform/opacity/translation）。
///
/// 对 TimeVariant 项返回 `None`。
pub fn item_paint_fingerprint(item: &DisplayItem, assets: &AssetsMap) -> Option<u64> {
    if item_is_time_variant(item, assets) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    DisplayItemFp(item).hash(&mut hasher);
    Some(hasher.finish())
}

/// 计算子树 paint fingerprint（递归，**不含** transform/opacity）。
///
/// 若任一后代是 TimeVariant，返回 `None`。
pub fn subtree_paint_fingerprint(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    if node_contains_time_variant(node, assets) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    hash_subtree_paint(node, &mut hasher);
    Some(hasher.finish())
}

/// 计算 subtree snapshot fingerprint。
///
/// 这个 key 用于缓存“当前节点整棵子树录成的 picture”。
/// 它故意：
/// - 不包含当前节点自己的 translation / opacity / transforms，因为这些在 picture 外部应用
/// - 递归包含所有后代的 composite 状态，因为后代的 composite 会被烘焙进当前节点 picture
///
/// 若任一后代是 TimeVariant，返回 `None`。
pub fn subtree_snapshot_fingerprint(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    if node_contains_time_variant(node, assets) {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    hash_subtree_snapshot(node, &mut hasher);
    Some(hasher.finish())
}

// ---------- TimeVariant 判定（内部） ----------

fn item_is_time_variant(item: &DisplayItem, assets: &AssetsMap) -> bool {
    match item {
        DisplayItem::Bitmap(bitmap) => bitmap_is_video(bitmap, assets),
        DisplayItem::DrawScript(script) => script.commands.iter().any(|command| {
            matches!(command, CanvasCommand::DrawImage { asset_id, .. }
                if assets
                    .path(&AssetId(asset_id.clone()))
                    .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
                    .unwrap_or(false))
        }),
        DisplayItem::Rect(_) | DisplayItem::Text(_) | DisplayItem::Lucide(_) => false,
    }
}

fn bitmap_is_video(bitmap: &BitmapDisplayItem, assets: &AssetsMap) -> bool {
    assets
        .path(&bitmap.asset_id)
        .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
        .unwrap_or(false)
}

fn node_contains_time_variant(node: &DisplayNode, assets: &AssetsMap) -> bool {
    item_is_time_variant(&node.item, assets)
        || node
            .children
            .iter()
            .any(|child| node_contains_time_variant(child, assets))
}

// ---------- Paint hash 构造（内部） ----------

/// 递归哈希一棵子树的 paint 内容。**不含** child 的 translation/opacity/transforms。
/// 若任一后代是 TimeVariant，调用者已经提前 return None，这里不检查。
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

fn calculate_hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Copy)]
struct F32Hash(f32);

impl Hash for F32Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

struct TextFp<'a>(&'a TextDisplayItem);

impl Hash for TextFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.text.hash(state);
        TextStyleFp(&self.0.style).hash(state);
        self.0.allow_wrap.hash(state);
        F32Hash(self.0.bounds.width).hash(state);
        F32Hash(self.0.bounds.height).hash(state);
        self.0.drop_shadow.hash(state);
    }
}

struct DisplayItemFp<'a>(&'a DisplayItem);

impl Hash for DisplayItemFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.0 {
            DisplayItem::Rect(rect) => {
                0_u8.hash(state);
                F32Hash(rect.bounds.width).hash(state);
                F32Hash(rect.bounds.height).hash(state);
                RectPaintFp(&rect.paint).hash(state);
            }
            DisplayItem::Text(text) => {
                1_u8.hash(state);
                text.text.hash(state);
                TextStyleFp(&text.style).hash(state);
                text.allow_wrap.hash(state);
                text.drop_shadow.hash(state);
                F32Hash(text.bounds.width).hash(state);
                F32Hash(text.bounds.height).hash(state);
            }
            DisplayItem::Bitmap(bitmap) => {
                2_u8.hash(state);
                bitmap.asset_id.hash(state);
                bitmap.width.hash(state);
                bitmap.height.hash(state);
                bitmap.video_timing.hash(state);
                bitmap.object_fit.hash(state);
                F32Hash(bitmap.bounds.width).hash(state);
                F32Hash(bitmap.bounds.height).hash(state);
                BitmapPaintFp(&bitmap.paint).hash(state);
            }
            DisplayItem::DrawScript(script) => {
                3_u8.hash(state);
                script.commands.hash(state);
                script.drop_shadow.hash(state);
                F32Hash(script.bounds.width).hash(state);
                F32Hash(script.bounds.height).hash(state);
            }
            DisplayItem::Lucide(lucide) => {
                4_u8.hash(state);
                lucide.icon.hash(state);
                LucidePaintFp(&lucide.paint).hash(state);
                F32Hash(lucide.bounds.width).hash(state);
                F32Hash(lucide.bounds.height).hash(state);
            }
        }
    }
}

struct ClipFp<'a>(Option<&'a DisplayClip>);

impl Hash for ClipFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.is_some().hash(state);
        if let Some(clip) = self.0 {
            F32Hash(clip.bounds.width).hash(state);
            F32Hash(clip.bounds.height).hash(state);
            clip.border_radius.hash(state);
        }
    }
}

struct TextStyleFp<'a>(&'a ComputedTextStyle);

impl Hash for TextStyleFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let style = self.0;
        style.color.hash(state);
        style.font_weight.hash(state);
        style.text_align.hash(state);
        F32Hash(style.text_px).hash(state);
        F32Hash(style.letter_spacing).hash(state);
        F32Hash(style.line_height).hash(state);
        style.line_height_px.map(F32Hash).hash(state);
        style.text_transform.hash(state);
    }
}

struct RectPaintFp<'a>(&'a crate::display::list::RectPaintStyle);

impl Hash for RectPaintFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let paint = self.0;
        paint.background.hash(state);
        paint.border_radius.hash(state);
        paint.border_width.map(F32Hash).hash(state);
        paint.border_color.hash(state);
        paint.blur_sigma.map(F32Hash).hash(state);
        paint.box_shadow.hash(state);
        paint.inset_shadow.hash(state);
        paint.drop_shadow.hash(state);
    }
}

struct BitmapPaintFp<'a>(&'a crate::display::list::BitmapPaintStyle);

impl Hash for BitmapPaintFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let paint = self.0;
        paint.background.hash(state);
        paint.border_radius.hash(state);
        paint.border_width.map(F32Hash).hash(state);
        paint.border_color.hash(state);
        paint.blur_sigma.map(F32Hash).hash(state);
        paint.box_shadow.hash(state);
        paint.inset_shadow.hash(state);
        paint.drop_shadow.hash(state);
    }
}

struct LucidePaintFp<'a>(&'a crate::display::list::LucidePaintStyle);

impl Hash for LucidePaintFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let paint = self.0;
        paint.foreground.hash(state);
        paint.background.hash(state);
        paint.border_width.map(F32Hash).hash(state);
        paint.border_color.hash(state);
        paint.drop_shadow.hash(state);
    }
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
            subtree_contains_time_variant: false,
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
}
