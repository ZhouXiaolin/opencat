use std::sync::Arc;

use crate::style::ComputedTextStyle;
use crate::transitions::TransitionKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementId(pub u64);

#[derive(Clone, Debug)]
pub struct ElementNode {
    pub id: ElementId,
    pub kind: ElementKind,
    pub style: super::style::ComputedStyle,
    pub children: Vec<ElementNode>,
}

#[derive(Clone, Debug)]
pub enum ElementKind {
    Div(ElementDiv),
    Text(ElementText),
    Bitmap(ElementBitmap),
    Transition(TransitionElement),
}

#[derive(Clone, Debug, Default)]
pub struct ElementDiv;

#[derive(Clone, Debug)]
pub struct ElementText {
    pub text: String,
    pub text_style: ComputedTextStyle,
}

#[derive(Clone, Debug)]
pub struct ElementBitmap {
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct TransitionElement {
    pub from: Box<ElementNode>,
    pub to: Box<ElementNode>,
    pub progress: f32,
    pub kind: TransitionKind,
}

impl ElementNode {}
