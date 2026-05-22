//! Platform role for media preparation: decoding video frames, audio slices,
//! and compiling runtime effects.

/// Mode for media preparation: Preview (fast, lower quality) or Export (full quality).
#[derive(Clone, Copy, Debug)]
pub enum MediaPrepareMode {
    Preview,
    Export,
}

/// Mode for audio preparation.
#[derive(Clone, Copy, Debug)]
pub enum AudioPrepareMode {
    Preview,
    Export,
}

/// Stub: plan describing which media assets a frame needs.
/// Will be fleshed out when the media pipeline is integrated.
#[derive(Clone, Debug, Default)]
pub struct FrameMediaPlan {}

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
        mode: MediaPrepareMode,
    ) -> Result<Self::PreparedFrameMedia, MediaError>;

    /// Prepare an audio slice for playback/export.
    fn prepare_audio_slice(
        &mut self,
        slice: &AudioPlanSlice,
        mode: AudioPrepareMode,
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
