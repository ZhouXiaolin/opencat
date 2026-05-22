use opencat_core::platform::media::{FrameMediaPlan, MediaError, PrepareMode};
use crate::executor::EnginePreparedFrameMedia;

pub fn prepare_frame(
    _plan: &FrameMediaPlan,
    _mode: PrepareMode,
) -> Result<EnginePreparedFrameMedia, MediaError> {
    Ok(EnginePreparedFrameMedia::default())
}
