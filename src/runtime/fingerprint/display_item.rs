use std::hash::{Hash, Hasher};

use crate::{
    display::list::{BitmapDisplayItem, DisplayClip, DisplayItem, TextDisplayItem},
    resource::{
        assets::AssetsMap,
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
    },
    style::ComputedTextStyle,
};

#[derive(Clone, Copy)]
pub(super) struct F32Hash(pub(super) f32);

impl Hash for F32Hash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

pub(super) struct TextFp<'a>(pub(super) &'a TextDisplayItem);

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

pub(super) struct DisplayItemFp<'a>(pub(super) &'a DisplayItem);

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

pub(super) struct ClipFp<'a>(pub(super) Option<&'a DisplayClip>);

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

pub(super) struct BitmapPaintFp<'a>(pub(super) &'a crate::display::list::BitmapPaintStyle);

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

pub(super) fn item_is_time_variant(item: &DisplayItem, assets: &AssetsMap) -> bool {
    match item {
        DisplayItem::Bitmap(bitmap) => bitmap_is_video(bitmap, assets),
        // DrawScript 的命令序列本身就是纯数据;若脚本输出稳定,hash 稳定即可跨帧复用。
        // 若脚本每帧产出不同 commands(读取 time_secs 等),hash 每帧变 → ItemPictureCache
        // 自然 miss,行为正确。无需静态分析脚本内容。
        DisplayItem::DrawScript(_) => false,
        DisplayItem::Rect(_) | DisplayItem::Text(_) | DisplayItem::Lucide(_) => false,
    }
}

pub(super) fn bitmap_is_video(bitmap: &BitmapDisplayItem, assets: &AssetsMap) -> bool {
    assets
        .path(&bitmap.asset_id)
        .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
        .unwrap_or(false)
}
