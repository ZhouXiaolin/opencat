use crate::probe::catalog::ResourceRequests;

#[derive(Clone, Debug, Default)]
pub struct CompositionInfo {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration: f64,
    pub requests: ResourceRequests,
}
