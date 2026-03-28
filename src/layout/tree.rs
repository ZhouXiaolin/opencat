use std::sync::Arc;

use crate::{
    element::style::ComputedVisualStyle,
    style::{ComputedTextStyle, ObjectFit},
    transitions::TransitionKind,
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
}

#[derive(Clone, Debug)]
pub enum LayoutPaintKind {
    Div,
    Text(LayoutTextPaint),
    Bitmap(LayoutBitmapPaint),
    Transition(LayoutTransitionPaint),
}

#[derive(Clone, Debug)]
pub struct LayoutTextPaint {
    pub text: String,
    pub style: ComputedTextStyle,
}

#[derive(Clone, Debug)]
pub struct LayoutBitmapPaint {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub object_fit: ObjectFit,
}

#[derive(Clone, Debug)]
pub struct LayoutTransitionPaint {
    pub from: Box<LayoutNode>,
    pub to: Box<LayoutNode>,
    pub progress: f32,
    pub kind: TransitionKind,
}
