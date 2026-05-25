use std::path::PathBuf;

use crate::parse::{
    composition::CompositionAudioSource,
    node::Node,
    primitives::{AudioSource, ImageSource, VideoSource},
};
use crate::style::NodeStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasChildrenMode {
    Forbid,
    HiddenPictureSubtree,
}

#[derive(Debug, Clone)]
pub enum ParsedElementKind {
    Timeline,
    Div,
    Text { content: String },
    Canvas,
    Image { source: ImageSource },
    Icon { name: String },
    Path { data: String },
    Video { source: VideoSource },
    Caption { path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct ParsedElement {
    pub id: String,
    pub parent_id: Option<String>,
    pub duration: Option<u32>,
    pub style: NodeStyle,
    pub kind: ParsedElementKind,
}

#[derive(Debug, Clone)]
pub struct ParsedTransition {
    pub parent_id: String,
    pub from: String,
    pub to: String,
    pub effect: String,
    pub duration: u32,
    pub direction: Option<String>,
    pub timing: Option<String>,
    pub damping: Option<f32>,
    pub stiffness: Option<f32>,
    pub mass: Option<f32>,
    pub seed: Option<f32>,
    pub hue_shift: Option<f32>,
    pub mask_scale: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ParsedAudioElement {
    pub id: String,
    pub parent_id: Option<String>,
    pub duration: Option<u32>,
    pub source: AudioSource,
}

#[derive(Debug, Clone)]
pub struct ParsedComposition {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub frames: i32,
    pub root: Node,
    pub script: Option<String>,
    pub audio_sources: Vec<CompositionAudioSource>,
}
