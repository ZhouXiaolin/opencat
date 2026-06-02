use crate::ir::asset_id::AssetId;
use crate::ir::draw_op::DrawOp;
use crate::parse::transition::TransitionKind;
use crate::resource::types::VideoFrameTiming;
use crate::script::TextUnitOverrideBatch;
use crate::semantic::fingerprint::ElementInputFingerprints;
use crate::style::ComputedTextStyle;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ElementId(pub u64);

#[derive(Clone, Debug, Default)]
pub struct ElementDrawSlot {
    pub commands: Vec<DrawOp>,
}

#[derive(Clone, Debug)]
pub struct ElementNode {
    pub id: ElementId,
    pub kind: ElementKind,
    pub style: super::style::ComputedStyle,
    pub children: Vec<ElementNode>,
    pub draw_slot: ElementDrawSlot,
    pub fingerprints: ElementInputFingerprints,
}

#[derive(Clone, Debug)]
pub enum ElementKind {
    Div(ElementDiv),
    Timeline(ElementTimeline),
    Text(ElementText),
    Bitmap(ElementBitmap),
    Lottie(ElementLottie),
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
pub struct ElementLottie {
    pub bundle_id: AssetId,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub duration_frames: u32,
}

#[derive(Clone, Debug)]
pub struct ElementCanvas {
    pub commands: Vec<DrawOp>,
}

#[derive(Clone, Debug)]
pub struct ElementSvgPath {
    pub path_data: Vec<String>,
    pub view_box: [f32; 4],
    pub intrinsic_size: Option<(f32, f32)>,
}

impl ElementNode {}
