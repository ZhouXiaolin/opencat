use crate::core::resource::{asset_catalog::AssetId, types::VideoFrameTiming};
use crate::core::scene::script::{CanvasCommand, TextUnitOverrideBatch};
use crate::core::scene::transition::TransitionKind;
use crate::core::style::ComputedTextStyle;

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
    Timeline(ElementTimeline),
    Text(ElementText),
    Bitmap(ElementBitmap),
    Canvas(ElementCanvas),
    SvgPath(ElementSvgPath),
}

#[derive(Clone, Debug, Default)]
pub struct ElementDiv;

#[derive(Clone, Debug)]
pub struct ElementTimeline {
    pub transition: Option<ElementTimelineTransition>,
}

#[derive(Clone, Debug)]
pub struct ElementTimelineTransition {
    pub progress: f32,
    pub kind: TransitionKind,
}

#[derive(Clone, Debug)]
pub struct ElementText {
    pub text: String,
    pub text_style: ComputedTextStyle,
    pub text_unit_overrides: Option<TextUnitOverrideBatch>,
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
pub struct ElementSvgPath {
    pub path_data: Vec<String>,
    pub view_box: [f32; 4],
    pub intrinsic_size: Option<(f32, f32)>,
}

impl ElementNode {}
