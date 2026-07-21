use crate::media::AudioPlan;
use crate::probe::catalog::ResourceRequests;

#[derive(Clone, Debug, Default)]
pub struct CompositionInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: f64,
    pub requests: ResourceRequests,
    /// Core-derived audio schedule (timeline / scene / transition / trim).
    /// Hosts decode and mix; they must not re-derive segment offsets.
    pub audio_plan: AudioPlan,
}
