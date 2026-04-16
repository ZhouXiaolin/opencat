use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::{
    display::{
        list::{DisplayItem, TextDisplayItem},
        tree::DisplayNode,
    },
    resource::{
        assets::{AssetId, AssetsMap},
        bitmap_source::{BitmapSourceKind, bitmap_source_kind},
    },
    scene::script::CanvasCommand,
    style::{ComputedTextStyle, Transform},
};

pub(crate) fn text_snapshot_cache_key(text: &TextDisplayItem) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.text.hash(&mut hasher);
    hash_text_style(&text.style, &mut hasher);
    text.allow_wrap.hash(&mut hasher);
    text.bounds.width.to_bits().hash(&mut hasher);
    text.bounds.height.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn subtree_snapshot_cache_key(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    subtree_snapshot_cache_key_inner(node, assets)
}

fn subtree_snapshot_cache_key_inner(node: &DisplayNode, assets: &AssetsMap) -> Option<u64> {
    if display_node_uses_video(node, assets) {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    hash_f32(node.transform.bounds.width, &mut hasher);
    hash_f32(node.transform.bounds.height, &mut hasher);
    hash_display_item(&node.item, &mut hasher);
    hash_clip(node.clip.as_ref(), &mut hasher);
    node.children.len().hash(&mut hasher);

    for child in &node.children {
        hash_f32(child.transform.translation_x, &mut hasher);
        hash_f32(child.transform.translation_y, &mut hasher);
        hash_f32(child.opacity, &mut hasher);
        hash_transforms(&child.transform.transforms, &mut hasher);
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

fn hash_display_item(item: &DisplayItem, state: &mut impl Hasher) {
    match item {
        DisplayItem::Rect(rect) => {
            0_u8.hash(state);
            rect.paint.background.hash(state);
            hash_f32(rect.paint.border_radius, state);
            rect.paint.border_width.map(f32::to_bits).hash(state);
            rect.paint.border_color.hash(state);
            rect.paint.blur_sigma.map(f32::to_bits).hash(state);
            rect.paint.box_shadow.hash(state);
            rect.paint.inset_shadow.hash(state);
            rect.paint.drop_shadow.hash(state);
        }
        DisplayItem::Text(text) => {
            1_u8.hash(state);
            text.text.hash(state);
            hash_text_style(&text.style, state);
            text.allow_wrap.hash(state);
            text.drop_shadow.hash(state);
        }
        DisplayItem::Bitmap(bitmap) => {
            2_u8.hash(state);
            bitmap.asset_id.hash(state);
            bitmap.width.hash(state);
            bitmap.height.hash(state);
            bitmap.video_timing.is_some().hash(state);
            if let Some(video_timing) = bitmap.video_timing {
                video_timing.media_offset_secs.to_bits().hash(state);
                video_timing.playback_rate.to_bits().hash(state);
                video_timing.looping.hash(state);
            }
            bitmap.object_fit.hash(state);
            bitmap.paint.background.hash(state);
            hash_f32(bitmap.paint.border_radius, state);
            bitmap.paint.border_width.map(f32::to_bits).hash(state);
            bitmap.paint.border_color.hash(state);
            bitmap.paint.blur_sigma.map(f32::to_bits).hash(state);
            bitmap.paint.box_shadow.hash(state);
            bitmap.paint.inset_shadow.hash(state);
            bitmap.paint.drop_shadow.hash(state);
        }
        DisplayItem::DrawScript(script) => {
            3_u8.hash(state);
            script.commands.len().hash(state);
            script.drop_shadow.hash(state);
            for command in &script.commands {
                hash_draw_script_command(command, state);
            }
        }
        DisplayItem::Lucide(lucide) => {
            4_u8.hash(state);
            lucide.icon.hash(state);
            lucide.paint.foreground.hash(state);
            lucide.paint.background.hash(state);
            lucide.paint.border_width.map(f32::to_bits).hash(state);
            lucide.paint.border_color.hash(state);
            lucide.paint.drop_shadow.hash(state);
        }
    }
}

fn hash_clip(clip: Option<&crate::display::list::DisplayClip>, state: &mut impl Hasher) {
    clip.is_some().hash(state);
    if let Some(clip) = clip {
        hash_f32(clip.bounds.width, state);
        hash_f32(clip.bounds.height, state);
        hash_f32(clip.border_radius, state);
    }
}

fn hash_text_style(style: &ComputedTextStyle, state: &mut impl Hasher) {
    style.color.hash(state);
    style.font_weight.hash(state);
    style.text_align.hash(state);
    hash_f32(style.text_px, state);
    hash_f32(style.letter_spacing, state);
    hash_f32(style.line_height, state);
    style.line_height_px.map(f32::to_bits).hash(state);
    style.text_transform.hash(state);
}

fn hash_transforms(transforms: &[Transform], state: &mut impl Hasher) {
    transforms.len().hash(state);
    for transform in transforms {
        match *transform {
            Transform::TranslateX(x) => {
                0_u8.hash(state);
                hash_f32(x, state);
            }
            Transform::TranslateY(y) => {
                1_u8.hash(state);
                hash_f32(y, state);
            }
            Transform::Translate(x, y) => {
                2_u8.hash(state);
                hash_f32(x, state);
                hash_f32(y, state);
            }
            Transform::Scale(value) => {
                3_u8.hash(state);
                hash_f32(value, state);
            }
            Transform::ScaleX(value) => {
                4_u8.hash(state);
                hash_f32(value, state);
            }
            Transform::ScaleY(value) => {
                5_u8.hash(state);
                hash_f32(value, state);
            }
            Transform::RotateDeg(value) => {
                6_u8.hash(state);
                hash_f32(value, state);
            }
            Transform::SkewXDeg(value) => {
                7_u8.hash(state);
                hash_f32(value, state);
            }
            Transform::SkewYDeg(value) => {
                8_u8.hash(state);
                hash_f32(value, state);
            }
            Transform::SkewDeg(x, y) => {
                9_u8.hash(state);
                hash_f32(x, state);
                hash_f32(y, state);
            }
        }
    }
}

fn hash_f32(value: f32, state: &mut impl Hasher) {
    value.to_bits().hash(state);
}

fn hash_draw_script_command(command: &CanvasCommand, state: &mut impl Hasher) {
    match command {
        CanvasCommand::Save => {
            0_u8.hash(state);
        }
        CanvasCommand::SaveLayer { alpha, bounds } => {
            45_u8.hash(state);
            hash_f32(*alpha, state);
            bounds.is_some().hash(state);
            if let Some(bounds) = bounds {
                for value in bounds {
                    hash_f32(*value, state);
                }
            }
        }
        CanvasCommand::Restore => {
            1_u8.hash(state);
        }
        CanvasCommand::RestoreToCount { count } => {
            43_u8.hash(state);
            count.hash(state);
        }
        CanvasCommand::SetFillStyle { color } => {
            2_u8.hash(state);
            color.hash(state);
        }
        CanvasCommand::SetStrokeStyle { color } => {
            3_u8.hash(state);
            color.hash(state);
        }
        CanvasCommand::SetLineWidth { width } => {
            4_u8.hash(state);
            hash_f32(*width, state);
        }
        CanvasCommand::SetLineCap { cap } => {
            5_u8.hash(state);
            cap.hash(state);
        }
        CanvasCommand::SetLineJoin { join } => {
            6_u8.hash(state);
            join.hash(state);
        }
        CanvasCommand::SetLineDash { intervals, phase } => {
            7_u8.hash(state);
            intervals.len().hash(state);
            for interval in intervals {
                hash_f32(*interval, state);
            }
            hash_f32(*phase, state);
        }
        CanvasCommand::ClearLineDash => {
            8_u8.hash(state);
        }
        CanvasCommand::SetGlobalAlpha { alpha } => {
            9_u8.hash(state);
            hash_f32(*alpha, state);
        }
        CanvasCommand::SetAntiAlias { enabled } => {
            44_u8.hash(state);
            enabled.hash(state);
        }
        CanvasCommand::Translate { x, y } => {
            10_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        CanvasCommand::Scale { x, y } => {
            11_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        CanvasCommand::Rotate { degrees } => {
            12_u8.hash(state);
            hash_f32(*degrees, state);
        }
        CanvasCommand::ClipRect {
            x,
            y,
            width,
            height,
            anti_alias,
        } => {
            13_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            anti_alias.hash(state);
        }
        CanvasCommand::Clear { color } => {
            14_u8.hash(state);
            color.hash(state);
        }
        CanvasCommand::DrawPaint { color, anti_alias } => {
            46_u8.hash(state);
            color.hash(state);
            anti_alias.hash(state);
        }
        CanvasCommand::DrawText {
            text,
            x,
            y,
            color,
            anti_alias,
            stroke,
            stroke_width,
            font_size,
            font_scale_x,
            font_skew_x,
            font_subpixel,
            font_edging,
        } => {
            51_u8.hash(state);
            text.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            color.hash(state);
            anti_alias.hash(state);
            stroke.hash(state);
            hash_f32(*stroke_width, state);
            hash_f32(*font_size, state);
            hash_f32(*font_scale_x, state);
            hash_f32(*font_skew_x, state);
            font_subpixel.hash(state);
            font_edging.hash(state);
        }
        CanvasCommand::FillRect {
            x,
            y,
            width,
            height,
            color,
        } => {
            15_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            color.hash(state);
        }
        CanvasCommand::FillRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            16_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*radius, state);
        }
        CanvasCommand::StrokeRect {
            x,
            y,
            width,
            height,
            color,
            stroke_width,
        } => {
            17_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            color.hash(state);
            hash_f32(*stroke_width, state);
        }
        CanvasCommand::StrokeRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            18_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*radius, state);
        }
        CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
            19_u8.hash(state);
            hash_f32(*x0, state);
            hash_f32(*y0, state);
            hash_f32(*x1, state);
            hash_f32(*y1, state);
        }
        CanvasCommand::FillCircle { cx, cy, radius } => {
            20_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*radius, state);
        }
        CanvasCommand::StrokeCircle { cx, cy, radius } => {
            21_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*radius, state);
        }
        CanvasCommand::BeginPath => {
            22_u8.hash(state);
        }
        CanvasCommand::MoveTo { x, y } => {
            23_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        CanvasCommand::LineTo { x, y } => {
            24_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        CanvasCommand::QuadTo { cx, cy, x, y } => {
            25_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        CanvasCommand::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => {
            26_u8.hash(state);
            hash_f32(*c1x, state);
            hash_f32(*c1y, state);
            hash_f32(*c2x, state);
            hash_f32(*c2y, state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        CanvasCommand::ClosePath => {
            27_u8.hash(state);
        }
        CanvasCommand::AddRectPath {
            x,
            y,
            width,
            height,
        } => {
            47_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
        }
        CanvasCommand::AddRRectPath {
            x,
            y,
            width,
            height,
            radius,
        } => {
            48_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*radius, state);
        }
        CanvasCommand::AddOvalPath {
            x,
            y,
            width,
            height,
        } => {
            49_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
        }
        CanvasCommand::AddArcPath {
            x,
            y,
            width,
            height,
            start_angle,
            sweep_angle,
        } => {
            50_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*start_angle, state);
            hash_f32(*sweep_angle, state);
        }
        CanvasCommand::FillPath => {
            28_u8.hash(state);
        }
        CanvasCommand::StrokePath => {
            29_u8.hash(state);
        }
        CanvasCommand::DrawImage {
            asset_id,
            x,
            y,
            width,
            height,
            src_rect,
            alpha,
            anti_alias,
            object_fit,
        } => {
            30_u8.hash(state);
            asset_id.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            src_rect.is_some().hash(state);
            if let Some(src_rect) = src_rect {
                for value in src_rect {
                    hash_f32(*value, state);
                }
            }
            hash_f32(*alpha, state);
            anti_alias.hash(state);
            object_fit.hash(state);
        }
        CanvasCommand::DrawArc {
            cx,
            cy,
            rx,
            ry,
            start_angle,
            sweep_angle,
            use_center,
        } => {
            31_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*rx, state);
            hash_f32(*ry, state);
            hash_f32(*start_angle, state);
            hash_f32(*sweep_angle, state);
            use_center.hash(state);
        }
        CanvasCommand::StrokeArc {
            cx,
            cy,
            rx,
            ry,
            start_angle,
            sweep_angle,
        } => {
            32_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*rx, state);
            hash_f32(*ry, state);
            hash_f32(*start_angle, state);
            hash_f32(*sweep_angle, state);
        }
        CanvasCommand::FillOval { cx, cy, rx, ry } => {
            33_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*rx, state);
            hash_f32(*ry, state);
        }
        CanvasCommand::StrokeOval { cx, cy, rx, ry } => {
            34_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*rx, state);
            hash_f32(*ry, state);
        }
        CanvasCommand::ClipPath { anti_alias } => {
            35_u8.hash(state);
            anti_alias.hash(state);
        }
        CanvasCommand::ClipRRect {
            x,
            y,
            width,
            height,
            radius,
            anti_alias,
        } => {
            36_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*radius, state);
            anti_alias.hash(state);
        }
        CanvasCommand::DrawPoints { mode, points } => {
            37_u8.hash(state);
            mode.hash(state);
            for p in points {
                hash_f32(*p, state);
            }
        }
        CanvasCommand::FillDRRect {
            outer_x,
            outer_y,
            outer_width,
            outer_height,
            outer_radius,
            inner_x,
            inner_y,
            inner_width,
            inner_height,
            inner_radius,
        } => {
            38_u8.hash(state);
            hash_f32(*outer_x, state);
            hash_f32(*outer_y, state);
            hash_f32(*outer_width, state);
            hash_f32(*outer_height, state);
            hash_f32(*outer_radius, state);
            hash_f32(*inner_x, state);
            hash_f32(*inner_y, state);
            hash_f32(*inner_width, state);
            hash_f32(*inner_height, state);
            hash_f32(*inner_radius, state);
        }
        CanvasCommand::StrokeDRRect {
            outer_x,
            outer_y,
            outer_width,
            outer_height,
            outer_radius,
            inner_x,
            inner_y,
            inner_width,
            inner_height,
            inner_radius,
        } => {
            39_u8.hash(state);
            hash_f32(*outer_x, state);
            hash_f32(*outer_y, state);
            hash_f32(*outer_width, state);
            hash_f32(*outer_height, state);
            hash_f32(*outer_radius, state);
            hash_f32(*inner_x, state);
            hash_f32(*inner_y, state);
            hash_f32(*inner_width, state);
            hash_f32(*inner_height, state);
            hash_f32(*inner_radius, state);
        }
        CanvasCommand::Skew { sx, sy } => {
            40_u8.hash(state);
            hash_f32(*sx, state);
            hash_f32(*sy, state);
        }
        CanvasCommand::DrawImageSimple {
            asset_id,
            x,
            y,
            alpha,
            anti_alias,
        } => {
            41_u8.hash(state);
            asset_id.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*alpha, state);
            anti_alias.hash(state);
        }
        CanvasCommand::Concat { matrix } => {
            42_u8.hash(state);
            for v in matrix {
                hash_f32(*v, state);
            }
        }
    }
}
