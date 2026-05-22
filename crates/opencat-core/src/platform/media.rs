//! Platform role for media preparation: decoding video frames, audio slices,
//! and compiling runtime effects.

use crate::draw::types::{EffectId, ImageRef};

/// Unified preparation mode for both media and audio.
/// Preview favors speed over quality; Export favors quality over speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareMode {
    Preview,
    Export,
}

/// Plan for preparing media assets needed by a single frame.
#[derive(Clone, Debug, Default)]
pub struct FrameMediaPlan {
    /// Deduplicated image references needed for this frame.
    pub images: Vec<ImageRef>,
    /// Runtime effect IDs needed for this frame (resolved to EffectRef during media preparation).
    pub runtime_effects: Vec<EffectId>,
}

/// Stub: a slice of an audio track to prepare for playback or export.
#[derive(Clone, Debug, Default)]
pub struct AudioPlanSlice {}

/// Platform role for media preparation (decoding video frames, audio slices,
/// compiling runtime effects).
pub trait MediaPlatform {
    type PreparedFrameMedia;
    type PreparedAudioSlice;

    /// Prepare a frame's media assets for rendering.
    fn prepare_frame(
        &mut self,
        plan: &FrameMediaPlan,
        mode: PrepareMode,
    ) -> Result<Self::PreparedFrameMedia, MediaError>;

    /// Prepare an audio slice for playback/export.
    fn prepare_audio_slice(
        &mut self,
        slice: &AudioPlanSlice,
        mode: PrepareMode,
    ) -> Result<Self::PreparedAudioSlice, MediaError>;
}

/// Error type for media preparation failures.
#[derive(Debug)]
pub struct MediaError(pub String);

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MediaError: {}", self.0)
    }
}

impl std::error::Error for MediaError {}
