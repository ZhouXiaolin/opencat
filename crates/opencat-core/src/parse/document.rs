use std::path::PathBuf;

use crate::fonts::FontManifest;
use crate::media::VideoFrameTiming;
use crate::parse::{
    composition::CompositionAudioSource,
    node::Node,
    primitives::{AudioSource, ImageSource, VideoSource},
};
use crate::style::NodeStyle;

mod builder;

pub use builder::{
    BuildOptions, build_font_resources, build_parsed_document, build_tree, build_tree_with_options,
    build_tree_with_tl, build_tree_with_tl_options, join_scripts, script_driver_from_decls,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasChildrenMode {
    Forbid,
    HiddenPictureSubtree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedIdKind {
    Visual,
    Audio,
}

/// Script declaration collected during parse. External paths stay logical;
/// host supplies text via lifecycle [`crate::lifecycle::HostInputs`] (issue #20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclaredScript {
    /// Inline `src` / `content` / markup `<script>` body.
    Inline(String),
    /// Logical path or URL locator — not read by core.
    External { locator: String },
}

#[derive(Debug, Default, Clone)]
pub struct ParsedDocumentParts {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub duration: f64,
    pub elements: Vec<ParsedElement>,
    pub transitions: Vec<ParsedTransition>,
    pub audio_elements: Vec<ParsedAudioElement>,
    pub scripts_by_parent: std::collections::HashMap<String, Vec<DeclaredScript>>,
    pub global_scripts: Vec<DeclaredScript>,
    pub markup_root_script: Option<String>,
    pub font_manifest: FontManifest,
}

pub fn validate_unique_ids(
    elements: &[ParsedElement],
    audio: &[ParsedAudioElement],
) -> anyhow::Result<std::collections::HashMap<String, ParsedIdKind>> {
    let mut ids = std::collections::HashMap::new();
    for element in elements {
        if ids
            .insert(element.id.clone(), ParsedIdKind::Visual)
            .is_some()
        {
            anyhow::bail!("duplicate id `{}`", element.id);
        }
    }
    for audio in audio {
        if ids.insert(audio.id.clone(), ParsedIdKind::Audio).is_some() {
            anyhow::bail!("duplicate id `{}`", audio.id);
        }
    }
    Ok(ids)
}

#[derive(Debug, Clone)]
pub enum ParsedElementKind {
    Timeline,
    Div,
    Text {
        content: String,
    },
    Canvas,
    Image {
        source: ImageSource,
    },
    Lottie {
        source: crate::parse::primitives::LottieSource,
        timing: VideoFrameTiming,
    },
    Icon {
        name: String,
    },
    Path {
        data: String,
    },
    Video {
        source: VideoSource,
        timing: VideoFrameTiming,
    },
    Caption {
        path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct ParsedElement {
    pub id: String,
    pub parent_id: Option<String>,
    pub duration: Option<f64>,
    pub style: NodeStyle,
    pub kind: ParsedElementKind,
}

#[derive(Debug, Clone)]
pub struct ParsedTransition {
    pub parent_id: String,
    pub from: String,
    pub to: String,
    pub effect: String,
    pub duration: f64,
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
    pub attach: String,
    pub duration: Option<f64>,
    pub source: AudioSource,
}

#[derive(Debug, Clone)]
pub struct ParsedComposition {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub duration: f64,
    pub root: Node,
    pub script: Option<String>,
    pub audio_sources: Vec<CompositionAudioSource>,
    pub font_manifest: FontManifest,
}
