//! Platform role for media preparation: decoding video frames, audio slices,
//! and compiling runtime effects.

pub use crate::ir::media_plan::FrameMediaPlan;

#[derive(Clone, Debug, Default)]
pub struct AudioPlanSlice {}

pub trait MediaPlatform {
    type PreparedFrameMedia;
    type PreparedAudioSlice;

    fn prepare_frame(
        &mut self,
        plan: &FrameMediaPlan,
        mode: PrepareMode,
    ) -> Result<Self::PreparedFrameMedia, MediaError>;

    fn prepare_audio_slice(
        &mut self,
        slice: &AudioPlanSlice,
        mode: PrepareMode,
    ) -> Result<Self::PreparedAudioSlice, MediaError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrepareMode {
    Preview,
    Export,
}

#[derive(Debug)]
pub struct MediaError(pub String);

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MediaError: {}", self.0)
    }
}

impl std::error::Error for MediaError {}
