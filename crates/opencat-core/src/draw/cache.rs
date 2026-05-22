/// Stub type for importing cached draw segments into a builder.
/// Will be fully implemented in Task 3.1.
use super::op::DrawOp;
use super::types::*;
use crate::canvas::paint::PaintSpec;

#[derive(Clone, Debug, Default)]
pub struct CachedDrawSegment {
    pub ops: Vec<DrawOp>,
    pub paints: Vec<PaintSpec>,
    pub paths: Vec<EncodedPath>,
    pub children: Vec<RuntimeEffectChildRef>,
    pub strings: Vec<String>,
    pub bytes: Vec<u8>,
    pub byte_ranges: Vec<TableRange>,
    pub f32_pool: Vec<f32>,
    pub resources: Vec<ResourceRef>,
}
