use crate::{element::style::ComputedVisualStyle, style::ComputedTextStyle};

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
}

#[derive(Clone, Debug)]
pub enum LayoutPaintKind {
    Div,
    Text(LayoutTextPaint),
}

#[derive(Clone, Debug)]
pub struct LayoutTextPaint {
    pub text: String,
    pub style: ComputedTextStyle,
}
