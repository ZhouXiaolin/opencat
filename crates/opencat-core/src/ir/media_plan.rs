use super::draw_types::{EffectRef, ImageRef};

#[derive(Clone, Debug, Default)]
pub struct FrameMediaPlan {
    pub images: Vec<ImageRef>,
    pub runtime_effects: Vec<EffectRef>,
}
