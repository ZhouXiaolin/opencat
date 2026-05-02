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
        self.0.truncate.hash(state);
        F32Hash(self.0.bounds.width).hash(state);
        F32Hash(self.0.bounds.height).hash(state);
        self.0.drop_shadow.hash(state);
        F32Hash(self.0.visual_expand_x).hash(state);
        F32Hash(self.0.visual_expand_y).hash(state);
        self.0.text_unit_overrides.is_some().hash(state);
        if let Some(batch) = &self.0.text_unit_overrides {
            std::mem::discriminant(&batch.granularity).hash(state);
            for unit in &batch.overrides {
                unit.opacity.map(f32::to_bits).hash(state);
                unit.translate_x.map(f32::to_bits).hash(state);
                unit.translate_y.map(f32::to_bits).hash(state);
                unit.scale.map(f32::to_bits).hash(state);
                unit.rotation_deg.map(f32::to_bits).hash(state);
                unit.color.hash(state);
            }
        }
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
            DisplayItem::Timeline(timeline) => {
                5_u8.hash(state);
                F32Hash(timeline.bounds.width).hash(state);
                F32Hash(timeline.bounds.height).hash(state);
                RectPaintFp(&timeline.paint).hash(state);
                if let Some(transition) = timeline.transition.as_ref() {
                    F32Hash(transition.progress).hash(state);
                    hash_transition_kind(&transition.kind, state);
                }
            }
            DisplayItem::Text(text) => {
                1_u8.hash(state);
                TextFp(text).hash(state);
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
            DisplayItem::SvgPath(svg) => {
                4_u8.hash(state);
                for data in &svg.path_data {
                    data.hash(state);
                }
                svg.view_box.map(F32Hash).hash(state);
                SvgPathPaintFp(&svg.paint).hash(state);
                F32Hash(svg.bounds.width).hash(state);
                F32Hash(svg.bounds.height).hash(state);
            }
        }
    }
}

fn hash_transition_kind<H: Hasher>(kind: &crate::scene::transition::TransitionKind, state: &mut H) {
    match kind {
        crate::scene::transition::TransitionKind::Slide(direction) => {
            0_u8.hash(state);
            std::mem::discriminant(direction).hash(state);
        }
        crate::scene::transition::TransitionKind::LightLeak(params) => {
            1_u8.hash(state);
            F32Hash(params.seed).hash(state);
            F32Hash(params.hue_shift).hash(state);
            F32Hash(params.mask_scale).hash(state);
        }
        crate::scene::transition::TransitionKind::Gl(effect) => {
            2_u8.hash(state);
            effect.name.hash(state);
        }
        crate::scene::transition::TransitionKind::Fade => 3_u8.hash(state),
        crate::scene::transition::TransitionKind::Wipe(direction) => {
            4_u8.hash(state);
            std::mem::discriminant(direction).hash(state);
        }
        crate::scene::transition::TransitionKind::ClockWipe => 5_u8.hash(state),
        crate::scene::transition::TransitionKind::Iris => 6_u8.hash(state),
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
        style.line_through.hash(state);
    }
}

struct RectPaintFp<'a>(&'a crate::display::list::RectPaintStyle);

impl Hash for RectPaintFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let paint = self.0;
        paint.background.hash(state);
        paint.border_radius.hash(state);
        paint.border_width.map(F32Hash).hash(state);
        paint.border_top_width.map(F32Hash).hash(state);
        paint.border_right_width.map(F32Hash).hash(state);
        paint.border_bottom_width.map(F32Hash).hash(state);
        paint.border_left_width.map(F32Hash).hash(state);
        paint.border_color.hash(state);
        paint.border_style.hash(state);
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
        paint.border_top_width.map(F32Hash).hash(state);
        paint.border_right_width.map(F32Hash).hash(state);
        paint.border_bottom_width.map(F32Hash).hash(state);
        paint.border_left_width.map(F32Hash).hash(state);
        paint.border_color.hash(state);
        paint.border_style.hash(state);
        paint.blur_sigma.map(F32Hash).hash(state);
        paint.box_shadow.hash(state);
        paint.inset_shadow.hash(state);
        paint.drop_shadow.hash(state);
    }
}

struct SvgPathPaintFp<'a>(&'a crate::display::list::SvgPathPaintStyle);

impl Hash for SvgPathPaintFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let paint = self.0;
        paint.fill.hash(state);
        paint.stroke_width.map(F32Hash).hash(state);
        paint.stroke_color.hash(state);
        paint.drop_shadow.hash(state);
        paint.stroke_dasharray.map(F32Hash).hash(state);
        paint.stroke_dashoffset.map(F32Hash).hash(state);
    }
}

pub(super) fn item_is_time_variant(item: &DisplayItem, assets: &AssetsMap) -> bool {
    match item {
        DisplayItem::Timeline(_) => true,
        DisplayItem::Bitmap(bitmap) => bitmap_is_video(bitmap, assets),
        // DrawScript 的命令序列本身就是纯数据;若脚本输出稳定,hash 稳定即可跨帧复用。
        // 若脚本每帧产出不同 commands(读取 time_secs 等),hash 每帧变 → ItemPictureCache
        // 自然 miss,行为正确。无需静态分析脚本内容。
        DisplayItem::DrawScript(_) => false,
        DisplayItem::Text(text) => text.text_unit_overrides.is_some(),
        DisplayItem::Rect(_) | DisplayItem::SvgPath(_) => false,
    }
}

pub(super) fn bitmap_is_video(bitmap: &BitmapDisplayItem, assets: &AssetsMap) -> bool {
    assets
        .path(&bitmap.asset_id)
        .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::item_is_time_variant;
    use crate::{
        display::list::{DisplayItem, DisplayRect, RectPaintStyle, TimelineDisplayItem},
        resource::assets::AssetsMap,
    };

    fn bounds() -> DisplayRect {
        DisplayRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    #[test]
    fn timeline_display_item_is_always_time_variant() {
        let assets = AssetsMap::new();
        let item = DisplayItem::Timeline(TimelineDisplayItem {
            bounds: bounds(),
            paint: RectPaintStyle {
                background: None,
                border_radius: crate::style::BorderRadius::default(),
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
            transition: None,
        });

        assert!(
            item_is_time_variant(&item, &assets),
            "timeline node itself should stay on the live path"
        );
    }
}
