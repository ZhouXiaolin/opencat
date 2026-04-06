use crate::{
    assets::AssetId,
    element::style::ComputedVisualStyle,
    style::{ColorToken, ComputedTextStyle, ObjectFit},
};

#[derive(Clone, Copy, Debug)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub rect: LayoutRect,
    pub paint: LayoutPaint,
    pub children: Vec<LayoutNode>,
}

#[derive(Clone, Debug)]
pub struct LayoutTree {
    pub root: LayoutNode,
}

#[derive(Clone, Debug)]
pub struct LayoutPaint {
    pub visual: ComputedVisualStyle,
    pub kind: LayoutPaintKind,
    pub id: String,
}

#[derive(Clone, Debug)]
pub enum LayoutPaintKind {
    Div,
    Text(LayoutTextPaint),
    Bitmap(LayoutBitmapPaint),
    Lucide(LayoutLucidePaint),
}

#[derive(Clone, Debug)]
pub struct LayoutTextPaint {
    pub text: String,
    pub style: ComputedTextStyle,
    pub allow_wrap: bool,
}

#[derive(Clone, Debug)]
pub struct LayoutBitmapPaint {
    pub asset_id: AssetId,
    pub width: u32,
    pub height: u32,
    pub object_fit: ObjectFit,
}

#[derive(Clone, Debug)]
pub struct LayoutLucidePaint {
    pub icon: String,
    pub foreground: ColorToken,
}
