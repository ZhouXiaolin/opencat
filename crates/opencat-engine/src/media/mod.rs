pub mod prepare;

use opencat_core::platform::media::{
    AudioPlanSlice, FrameMediaPlan, MediaError, MediaPlatform, PrepareMode,
};
use crate::executor::EnginePreparedFrameMedia;
use crate::resource::media::MediaContext;

pub struct EngineMedia {
    asset_paths: *const crate::resource::AssetPathStore,
    video: *mut MediaContext,
}

impl EngineMedia {
    pub fn new(
        asset_paths: &crate::resource::AssetPathStore,
        video: *mut MediaContext,
    ) -> Self {
        Self {
            asset_paths: asset_paths as *const _,
            video,
        }
    }

    fn asset_paths(&self) -> &crate::resource::AssetPathStore {
        unsafe { &*self.asset_paths }
    }

    fn video_mut(&mut self) -> Option<&mut MediaContext> {
        if self.video.is_null() {
            None
        } else {
            Some(unsafe { &mut *self.video })
        }
    }
}

impl MediaPlatform for EngineMedia {
    type PreparedFrameMedia = EnginePreparedFrameMedia;
    type PreparedAudioSlice = ();

    fn prepare_frame(
        &mut self,
        plan: &FrameMediaPlan,
        mode: PrepareMode,
    ) -> Result<Self::PreparedFrameMedia, MediaError> {
        prepare::prepare_frame(plan, mode, self.asset_paths(), self.video)
    }

    fn prepare_audio_slice(
        &mut self,
        _slice: &AudioPlanSlice,
        _mode: PrepareMode,
    ) -> Result<Self::PreparedAudioSlice, MediaError> {
        Ok(())
    }
}
