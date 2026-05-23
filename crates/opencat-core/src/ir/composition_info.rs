use crate::probe::catalog::{AudioPlan, ResourceRequests};

#[derive(Clone, Debug, Default)]
pub struct CompositionInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u32,
    pub requests: ResourceRequests,
    pub audio_plan: AudioPlan,
}
