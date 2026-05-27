use std::hash::{Hash, Hasher};

use crate::{
    display::list::{DisplayClip, DisplayItem, TextDisplayItem},
    display::tree::{DisplayNode, HiddenChildDisplayNode},
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
                RectPaintFp(&rect.paint).hash(state);
            }
            DisplayItem::Timeline(timeline) => {
                5_u8.hash(state);
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
                bitmap.paint_epoch.hash(state);
                bitmap.object_fit.hash(state);
                BitmapPaintFp(&bitmap.paint).hash(state);
            }
            DisplayItem::DrawScript(script) => {
                3_u8.hash(state);
                script.commands.hash(state);
                script.drop_shadow.hash(state);
                script.hidden_subtree.len().hash(state);
                for child in &script.hidden_subtree {
                    HiddenChildFp(child).hash(state);
                }
            }
            DisplayItem::SvgPath(svg) => {
                4_u8.hash(state);
                for data in &svg.path_data {
                    data.hash(state);
                }
                svg.view_box.map(F32Hash).hash(state);
                SvgPathPaintFp(&svg.paint).hash(state);
            }
        }
    }
}

struct HiddenChildFp<'a>(&'a HiddenChildDisplayNode);

impl Hash for HiddenChildFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.owner_id.hash(state);
        DisplayNodeFp(&self.0.node).hash(state);
    }
}

struct DisplayNodeFp<'a>(&'a DisplayNode);

impl Hash for DisplayNodeFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let node = self.0;
        node.element_id.hash(state);
        node.layout_output_fingerprint.record_size.hash(state);
        F32Hash(node.transform.translation_x).hash(state);
        F32Hash(node.transform.translation_y).hash(state);
        node.transform.transforms.hash(state);
        F32Hash(node.opacity).hash(state);
        node.backdrop_blur_sigma.map(F32Hash).hash(state);
        ClipFp(node.clip.as_ref()).hash(state);
        DisplayItemFp(&node.item).hash(state);
        node.draw_slot.is_some().hash(state);
        if let Some(slot) = &node.draw_slot {
            let item = DisplayItem::DrawScript(slot.clone());
            DisplayItemFp(&item).hash(state);
        }
        node.children.len().hash(state);
        for child in &node.children {
            DisplayNodeFp(child).hash(state);
        }
    }
}

fn hash_transition_kind<H: Hasher>(kind: &crate::parse::transition::TransitionKind, state: &mut H) {
    match kind {
        crate::parse::transition::TransitionKind::Slide(direction) => {
            0_u8.hash(state);
            std::mem::discriminant(direction).hash(state);
        }
        crate::parse::transition::TransitionKind::LightLeak(params) => {
            1_u8.hash(state);
            F32Hash(params.seed).hash(state);
            F32Hash(params.hue_shift).hash(state);
            F32Hash(params.mask_scale).hash(state);
        }
        crate::parse::transition::TransitionKind::Gl(effect) => {
            2_u8.hash(state);
            effect.name.hash(state);
        }
        crate::parse::transition::TransitionKind::Fade => 3_u8.hash(state),
        crate::parse::transition::TransitionKind::Wipe(direction) => {
            4_u8.hash(state);
            std::mem::discriminant(direction).hash(state);
        }
        crate::parse::transition::TransitionKind::ClockWipe => 5_u8.hash(state),
        crate::parse::transition::TransitionKind::Iris => 6_u8.hash(state),
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
