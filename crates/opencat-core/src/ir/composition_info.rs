use crate::media::AudioPlan;

/// Per-composition metadata that core derives during pipeline open.
///
/// `audio_plan` is the **sole canonical audio output**: hosts must not re-walk
/// the composition tree to invent timeline/scene/transition offsets. Every
/// host (Engine, Web) reads this plan for decode, mix, preview, and export.
#[derive(Clone, Debug, Default)]
pub struct CompositionInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: f64,
    /// Core-derived audio schedule (timeline / scene / transition / trim).
    ///
    /// This is the **canonical** composition-level audio plan. Hosts (Engine,
    /// Web) decode, mix, preview, and export from this plan; they must not
    /// re-traverse the AST to produce a second set of audio semantics.
    /// [`collect_audio_plan`] is called exactly once, during pipeline open,
    /// and the result is frozen for the lifetime of the pipeline.
    pub audio_plan: AudioPlan,
}
