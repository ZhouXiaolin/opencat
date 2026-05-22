pub mod prepare;

use opencat_core::platform::media::{
    AudioPlanSlice, FrameMediaPlan, MediaError, MediaPlatform, PrepareMode,
};
use crate::executor::EnginePreparedFrameMedia;

pub struct EngineMedia;

impl MediaPlatform for EngineMedia {
    type PreparedFrameMedia = EnginePreparedFrameMedia;
    type PreparedAudioSlice = ();

    fn prepare_frame(
        &mut self,
        plan: &FrameMediaPlan,
        mode: PrepareMode,
    ) -> Result<Self::PreparedFrameMedia, MediaError> {
        prepare::prepare_frame(plan, mode)
    }

    fn prepare_audio_slice(
        &mut self,
        _slice: &AudioPlanSlice,
        _mode: PrepareMode,
    ) -> Result<Self::PreparedAudioSlice, MediaError> {
        Ok(())
    }
}
