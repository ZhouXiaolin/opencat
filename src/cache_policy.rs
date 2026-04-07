use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::Path,
};

use crate::{
    assets::AssetsMap,
    display::list::{DisplayCommand, DisplayItem, DisplayList, TextDisplayItem},
    element::style::ComputedVisualStyle,
    layout::{
        LayoutPassStats,
        tree::{LayoutNode, LayoutPaintKind},
    },
    style::{ComputedTextStyle, Transform},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BitmapSourceKind {
    StaticImage,
    Video,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CacheInvalidationScope {
    Clean,
    Raster,
    Composite,
    Layout,
    Structure,
    TimeVariant,
}

impl CacheInvalidationScope {
    pub(crate) fn allows_picture_reuse(self) -> bool {
        matches!(self, Self::Clean)
    }

    pub(crate) fn prefers_subtree_cache(self) -> bool {
        matches!(self, Self::Composite)
    }
}

pub(crate) fn bitmap_source_kind(path: &Path) -> BitmapSourceKind {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4" | "mov" | "m4v" | "webm" | "mkv" | "avi") => BitmapSourceKind::Video,
        _ => BitmapSourceKind::StaticImage,
    }
}

pub(crate) fn display_list_contains_video(list: &DisplayList, assets: &AssetsMap) -> bool {
    list.commands.iter().any(|command| match command {
        DisplayCommand::Draw {
            item: DisplayItem::Bitmap(bitmap),
        } => assets
            .path(&bitmap.asset_id)
            .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
            .unwrap_or(false),
        DisplayCommand::Draw {
            item: DisplayItem::Canvas(canvas),
        } => canvas.commands.iter().any(|command| {
            matches!(command, crate::script::CanvasCommand::DrawImage { asset_id, .. }
                if assets
                    .path(&crate::assets::AssetId(asset_id.clone()))
                    .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
                    .unwrap_or(false))
        }),
        _ => false,
    })
}

pub(crate) fn scene_cache_scope(
    layout_pass: &LayoutPassStats,
    contains_video: bool,
) -> CacheInvalidationScope {
    if contains_video {
        CacheInvalidationScope::TimeVariant
    } else if layout_pass.structure_rebuild {
        CacheInvalidationScope::Structure
    } else if layout_pass.layout_dirty_nodes > 0 {
        CacheInvalidationScope::Layout
    } else if layout_pass.raster_dirty_nodes > 0 {
        CacheInvalidationScope::Raster
    } else if layout_pass.composite_dirty_nodes > 0 {
        CacheInvalidationScope::Composite
    } else {
        CacheInvalidationScope::Clean
    }
}

pub(crate) fn text_picture_cache_key(text: &TextDisplayItem) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.text.hash(&mut hasher);
    hash_text_style(&text.style, &mut hasher);
    text.allow_wrap.hash(&mut hasher);
    text.bounds.width.to_bits().hash(&mut hasher);
    text.bounds.height.to_bits().hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn subtree_picture_cache_key(layout: &LayoutNode, assets: &AssetsMap) -> Option<u64> {
    subtree_picture_cache_key_inner(layout, assets)
}

fn subtree_picture_cache_key_inner(layout: &LayoutNode, assets: &AssetsMap) -> Option<u64> {
    if layout_node_uses_video(layout, assets) {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    hash_f32(layout.rect.width, &mut hasher);
    hash_f32(layout.rect.height, &mut hasher);
    hash_raster_style(&layout.paint.visual, &mut hasher);
    hash_layout_paint_kind(&layout.paint.kind, &mut hasher);
    layout.children.len().hash(&mut hasher);

    for child in &layout.children {
        hash_f32(child.rect.x, &mut hasher);
        hash_f32(child.rect.y, &mut hasher);
        hash_composite_style(&child.paint.visual, &mut hasher);
        let child_key = subtree_picture_cache_key_inner(child, assets)?;
        child_key.hash(&mut hasher);
    }

    Some(hasher.finish())
}

fn layout_node_uses_video(layout: &LayoutNode, assets: &AssetsMap) -> bool {
    matches!(&layout.paint.kind, LayoutPaintKind::Bitmap(bitmap)
        if assets
            .path(&bitmap.asset_id)
            .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
            .unwrap_or(false))
        || matches!(&layout.paint.kind, LayoutPaintKind::Canvas(canvas)
            if canvas.commands.iter().any(|command| {
                matches!(command, crate::script::CanvasCommand::DrawImage { asset_id, .. }
                    if assets
                        .path(&crate::assets::AssetId(asset_id.clone()))
                        .map(|path| bitmap_source_kind(path) == BitmapSourceKind::Video)
                        .unwrap_or(false))
            }))
        || layout
            .children
            .iter()
            .any(|child| layout_node_uses_video(child, assets))
}

fn hash_layout_paint_kind(kind: &LayoutPaintKind, state: &mut impl Hasher) {
    match kind {
        LayoutPaintKind::Div => {
            0_u8.hash(state);
        }
        LayoutPaintKind::Text(text) => {
            1_u8.hash(state);
            text.text.hash(state);
            hash_text_style(&text.style, state);
            text.allow_wrap.hash(state);
        }
        LayoutPaintKind::Bitmap(bitmap) => {
            2_u8.hash(state);
            bitmap.asset_id.hash(state);
            bitmap.width.hash(state);
            bitmap.height.hash(state);
            bitmap.object_fit.hash(state);
        }
        LayoutPaintKind::Canvas(canvas) => {
            3_u8.hash(state);
            canvas.commands.len().hash(state);
            for command in &canvas.commands {
                hash_canvas_command(command, state);
            }
        }
        LayoutPaintKind::Lucide(lucide) => {
            4_u8.hash(state);
            lucide.icon.hash(state);
            lucide.foreground.hash(state);
        }
    }
}

fn hash_raster_style(style: &ComputedVisualStyle, state: &mut impl Hasher) {
    style.background.hash(state);
    hash_f32(style.border_radius, state);
    style.border_width.map(f32::to_bits).hash(state);
    style.border_color.hash(state);
    style.blur_sigma.map(f32::to_bits).hash(state);
    style.object_fit.hash(state);
    style.shadow.hash(state);
}

fn hash_composite_style(style: &ComputedVisualStyle, state: &mut impl Hasher) {
    hash_f32(style.opacity, state);
    hash_transforms(&style.transforms, state);
}

fn hash_text_style(style: &ComputedTextStyle, state: &mut impl Hasher) {
    style.color.hash(state);
    style.font_weight.hash(state);
    style.text_align.hash(state);
    hash_f32(style.text_px, state);
    hash_f32(style.letter_spacing, state);
    hash_f32(style.line_height, state);
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

fn hash_canvas_command(command: &crate::script::CanvasCommand, state: &mut impl Hasher) {
    match command {
        crate::script::CanvasCommand::Save => {
            0_u8.hash(state);
        }
        crate::script::CanvasCommand::Restore => {
            1_u8.hash(state);
        }
        crate::script::CanvasCommand::SetFillStyle { color } => {
            2_u8.hash(state);
            color.hash(state);
        }
        crate::script::CanvasCommand::SetStrokeStyle { color } => {
            3_u8.hash(state);
            color.hash(state);
        }
        crate::script::CanvasCommand::SetLineWidth { width } => {
            4_u8.hash(state);
            hash_f32(*width, state);
        }
        crate::script::CanvasCommand::SetLineCap { cap } => {
            5_u8.hash(state);
            cap.hash(state);
        }
        crate::script::CanvasCommand::SetLineJoin { join } => {
            6_u8.hash(state);
            join.hash(state);
        }
        crate::script::CanvasCommand::SetGlobalAlpha { alpha } => {
            7_u8.hash(state);
            hash_f32(*alpha, state);
        }
        crate::script::CanvasCommand::Translate { x, y } => {
            8_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::script::CanvasCommand::Scale { x, y } => {
            9_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::script::CanvasCommand::Rotate { degrees } => {
            10_u8.hash(state);
            hash_f32(*degrees, state);
        }
        crate::script::CanvasCommand::ClipRect {
            x,
            y,
            width,
            height,
        } => {
            11_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
        }
        crate::script::CanvasCommand::Clear { color } => {
            12_u8.hash(state);
            color.hash(state);
        }
        crate::script::CanvasCommand::FillRect {
            x,
            y,
            width,
            height,
            color,
        } => {
            13_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            color.hash(state);
        }
        crate::script::CanvasCommand::FillRRect {
            x,
            y,
            width,
            height,
            radius,
        } => {
            14_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            hash_f32(*radius, state);
        }
        crate::script::CanvasCommand::StrokeRect {
            x,
            y,
            width,
            height,
            color,
            stroke_width,
        } => {
            15_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            color.hash(state);
            hash_f32(*stroke_width, state);
        }
        crate::script::CanvasCommand::StrokeRRect {
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
        crate::script::CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
            17_u8.hash(state);
            hash_f32(*x0, state);
            hash_f32(*y0, state);
            hash_f32(*x1, state);
            hash_f32(*y1, state);
        }
        crate::script::CanvasCommand::FillCircle { cx, cy, radius } => {
            18_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*radius, state);
        }
        crate::script::CanvasCommand::StrokeCircle { cx, cy, radius } => {
            19_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*radius, state);
        }
        crate::script::CanvasCommand::BeginPath => {
            20_u8.hash(state);
        }
        crate::script::CanvasCommand::MoveTo { x, y } => {
            21_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::script::CanvasCommand::LineTo { x, y } => {
            22_u8.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::script::CanvasCommand::QuadTo { cx, cy, x, y } => {
            23_u8.hash(state);
            hash_f32(*cx, state);
            hash_f32(*cy, state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::script::CanvasCommand::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => {
            24_u8.hash(state);
            hash_f32(*c1x, state);
            hash_f32(*c1y, state);
            hash_f32(*c2x, state);
            hash_f32(*c2y, state);
            hash_f32(*x, state);
            hash_f32(*y, state);
        }
        crate::script::CanvasCommand::ClosePath => {
            25_u8.hash(state);
        }
        crate::script::CanvasCommand::FillPath => {
            26_u8.hash(state);
        }
        crate::script::CanvasCommand::StrokePath => {
            27_u8.hash(state);
        }
        crate::script::CanvasCommand::DrawImage {
            asset_id,
            x,
            y,
            width,
            height,
            object_fit,
        } => {
            28_u8.hash(state);
            asset_id.hash(state);
            hash_f32(*x, state);
            hash_f32(*y, state);
            hash_f32(*width, state);
            hash_f32(*height, state);
            object_fit.hash(state);
        }
    }
}
