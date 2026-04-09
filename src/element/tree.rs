use crate::resource::{assets::AssetId, media::VideoFrameTiming};
use crate::scene::script::CanvasCommand;
use crate::style::ComputedTextStyle;

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
    Canvas(ElementCanvas),
    Lucide(ElementLucide),
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
    pub asset_id: AssetId,
    pub width: u32,
    pub height: u32,
    pub video_timing: Option<VideoFrameTiming>,
}

#[derive(Clone, Debug)]
pub struct ElementCanvas {
    pub commands: Vec<CanvasCommand>,
}

#[derive(Clone, Debug)]
pub struct ElementLucide {
    pub icon: String,
}

impl ElementNode {}
