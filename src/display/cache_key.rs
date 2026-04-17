use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::{
    display::{
        list::{DisplayClip, DisplayItem, TextDisplayItem},
        tree::DisplayNode,
    },
    resource::{
        assets::{AssetId, AssetsMap},
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
    },
    scene::script::CanvasCommand,
    style::ComputedTextStyle,
};

pub(crate) fn text_snapshot_cache_key(text: &TextDisplayItem) -> u64 {
    calculate_hash(&TextSnapshotFingerprint(text))
}

pub(crate) fn subtree_snapshot_cache_key(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    subtree_snapshot_cache_key_inner(node, assets)
}

fn subtree_snapshot_cache_key_inner(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    if display_node_uses_video(node, assets) {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    F32Hash(node.transform.bounds.width).hash(&mut hasher);
    F32Hash(node.transform.bounds.height).hash(&mut hasher);
    DisplayItemFingerprint(&node.item).hash(&mut hasher);
    ClipFingerprint(node.clip.as_ref()).hash(&mut hasher);
    node.children.len().hash(&mut hasher);

    for child in &node.children {
        F32Hash(child.transform.translation_x).hash(&mut hasher);
        F32Hash(child.transform.translation_y).hash(&mut hasher);
        F32Hash(child.opacity).hash(&mut hasher);
        child.backdrop_blur_sigma.map(F32Hash).hash(&mut hasher);
        child.transform.transforms.hash(&mut hasher);
        let child_key = subtree_snapshot_cache_key_inner(child, assets)?;
        child_key.hash(&mut hasher);
    }

    Some(hasher.finish())
}

fn display_node_uses_video(node: &DisplayNode, assets: &AssetsMap) -> bool {
    display_item_uses_video(&node.item, assets)
        || node
            .children
            .iter()
            .any(|child| display_node_uses_video(child, assets))
}

fn display_item_uses_video(item: &DisplayItem, assets: &AssetsMap) -> bool {
    match item {
        DisplayItem::Bitmap(bitmap) => assets
            .path(&bitmap.asset_id)
            .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
            .unwrap_or(false),
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

struct TextSnapshotFingerprint<'a>(&'a TextDisplayItem);

impl Hash for TextSnapshotFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.text.hash(state);
        TextStyleFingerprint(&self.0.style).hash(state);
        self.0.allow_wrap.hash(state);
        F32Hash(self.0.bounds.width).hash(state);
        F32Hash(self.0.bounds.height).hash(state);
    }
}

struct DisplayItemFingerprint<'a>(&'a DisplayItem);

impl Hash for DisplayItemFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.0 {
            DisplayItem::Rect(rect) => {
                0_u8.hash(state);
                RectPaintFingerprint(&rect.paint).hash(state);
            }
            DisplayItem::Text(text) => {
                1_u8.hash(state);
                text.text.hash(state);
                TextStyleFingerprint(&text.style).hash(state);
                text.allow_wrap.hash(state);
                text.drop_shadow.hash(state);
            }
            DisplayItem::Bitmap(bitmap) => {
                2_u8.hash(state);
                bitmap.asset_id.hash(state);
                bitmap.width.hash(state);
                bitmap.height.hash(state);
                bitmap.video_timing.hash(state);
                bitmap.object_fit.hash(state);
                BitmapPaintFingerprint(&bitmap.paint).hash(state);
            }
            DisplayItem::DrawScript(script) => {
                3_u8.hash(state);
                script.commands.hash(state);
                script.drop_shadow.hash(state);
            }
            DisplayItem::Lucide(lucide) => {
                4_u8.hash(state);
                lucide.icon.hash(state);
                LucidePaintFingerprint(&lucide.paint).hash(state);
            }
        }
    }
}

struct ClipFingerprint<'a>(Option<&'a DisplayClip>);

impl Hash for ClipFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.is_some().hash(state);
        if let Some(clip) = self.0 {
            F32Hash(clip.bounds.width).hash(state);
            F32Hash(clip.bounds.height).hash(state);
            clip.border_radius.hash(state);
        }
    }
}

struct TextStyleFingerprint<'a>(&'a ComputedTextStyle);

impl Hash for TextStyleFingerprint<'_> {
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

struct RectPaintFingerprint<'a>(&'a crate::display::list::RectPaintStyle);

impl Hash for RectPaintFingerprint<'_> {
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

struct BitmapPaintFingerprint<'a>(&'a crate::display::list::BitmapPaintStyle);

impl Hash for BitmapPaintFingerprint<'_> {
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

struct LucidePaintFingerprint<'a>(&'a crate::display::list::LucidePaintStyle);

impl Hash for LucidePaintFingerprint<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let paint = self.0;
        paint.foreground.hash(state);
        paint.background.hash(state);
        paint.border_width.map(F32Hash).hash(state);
        paint.border_color.hash(state);
        paint.drop_shadow.hash(state);
    }
}
