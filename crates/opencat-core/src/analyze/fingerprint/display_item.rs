use std::hash::{Hash, Hasher};

use crate::{
    analyze::fingerprint::hash_hidden_child_display_node,
    display::list::{DisplayItem, TextDisplayItem},
    display::tree::HiddenChildDisplayNode,
    fp::{FpDim, FpF32},
};

pub(super) struct TextFp<'a>(pub(super) &'a TextDisplayItem);

impl Hash for TextFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.text.hash(state);
        // ComputedTextStyle 的 display 文本绘制维度（11 字段，语义层同源但
        // 不含 wrap_text——3b 统一）由 fp_struct! 声明序派生。
        self.0.style.fp_hash_dim(FpDim::TextPaintDisplay, state);
        // `background-clip: text` gradient layers change the glyph pixels, so
        // they must participate in the paint fingerprint (BackgroundFill is
        // Hash only through its stops; a debug repr is sufficient and cheap
        // here because gradient text is rare).
        self.0.text_fill.is_some().hash(state);
        if let Some(layers) = &self.0.text_fill {
            for layer in layers {
                format!("{layer:?}").hash(state);
            }
        }
        self.0
            .text_fill_box
            .map(|r| [r.x, r.y, r.width, r.height].map(FpF32))
            .hash(state);
        self.0.allow_wrap.hash(state);
        self.0.truncate.hash(state);
        // 折行基准宽（取整前盒宽）：同文本不同折行宽 = 不同像素。
        FpF32(self.0.wrap_width).hash(state);
        // 行内片段决定每段字形的落位（同文本不同折行 = 不同像素）。
        if let Some(fragments) = &self.0.fragments {
            for f in fragments.iter() {
                f.start.hash(state);
                f.end.hash(state);
                [f.rect.x, f.rect.y, f.rect.width, f.rect.height]
                    .map(FpF32)
                    .hash(state);
            }
        }
        self.0.drop_shadow.hash(state);
        self.0.text_shadows.hash(state);
        FpF32(self.0.visual_expand_x).hash(state);
        FpF32(self.0.visual_expand_y).hash(state);
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
                // RectPaintStyle 的 impl Hash = Paint 维度（fp_struct! 派生）。
                rect.paint.hash(state);
            }
            DisplayItem::Timeline(timeline) => {
                5_u8.hash(state);
                timeline.paint.hash(state);
                if let Some(transition) = timeline.transition.as_ref() {
                    FpF32(transition.progress).hash(state);
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
                bitmap.paint.hash(state);
            }
            DisplayItem::Lottie(lottie) => {
                6_u8.hash(state);
                lottie.bundle_id.hash(state);
                lottie.width.hash(state);
                lottie.height.hash(state);
                lottie.fps.to_bits().hash(state);
                lottie.in_frame.to_bits().hash(state);
                lottie.out_frame.to_bits().hash(state);
                lottie.timing.hash(state);
                lottie.paint_epoch.hash(state);
                lottie.object_fit.hash(state);
                lottie.paint.hash(state);
            }
            DisplayItem::DrawScript(script) => {
                3_u8.hash(state);
                script.commands.hash(state);
                script
                    .bitmap_scale
                    .map(|(x, y)| (x.to_bits(), y.to_bits()))
                    .hash(state);
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
                svg.view_box.map(FpF32).hash(state);
                svg.paint.hash(state);
            }
            DisplayItem::Svg(item) => {
                6_u8.hash(state);
                item.src.hash(state);
                item.bounds.x.to_bits().hash(state);
                item.bounds.y.to_bits().hash(state);
                item.bounds.width.to_bits().hash(state);
                item.bounds.height.to_bits().hash(state);
                item.preserve_aspect_ratio.hash(state);
            }
        }
    }
}

struct HiddenChildFp<'a>(&'a HiddenChildDisplayNode);

impl Hash for HiddenChildFp<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_hidden_child_display_node(self.0, state);
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
            FpF32(params.seed).hash(state);
            FpF32(params.hue_shift).hash(state);
            FpF32(params.mask_scale).hash(state);
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
        crate::parse::transition::TransitionKind::Cut => 7_u8.hash(state),
    }
}

