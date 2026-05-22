//! Platform role for draw execution: consuming DrawOpFrame and producing pixels.

use crate::draw::cache::CachedDrawRange;
use crate::draw::frame::DrawOpFrame;

/// Platform role for draw execution (consuming DrawOpFrame and producing pixels).
pub trait DrawPlatform {
    type Target;
    type PreparedFrameMedia;

    /// Execute a DrawOpFrame against a target surface.
    fn execute(
        &mut self,
        header: &RenderSessionHeader,
        draw: &DrawOpFrame,
        media: &Self::PreparedFrameMedia,
        target: &mut Self::Target,
    ) -> Result<DrawStats, DrawError>;

    /// Compile a cached DrawOp range into a platform-native object for fast replay.
    fn compile_range(
        &mut self,
        cached: &CachedDrawRange,
        draw: &DrawOpFrame,
    ) -> Result<(), DrawError>;

    /// Evict a cached range by fingerprint.
    fn evict_range(&mut self, fingerprint: u64);
}

/// Statistics returned after a frame execution.
#[derive(Debug, Default)]
pub struct DrawStats {
    pub op_count: u32,
    pub cache_hits: u32,
}

/// Error type for draw execution failures.
#[derive(Debug)]
pub struct DrawError(pub String);

impl std::fmt::Display for DrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DrawError: {}", self.0)
    }
}

impl std::error::Error for DrawError {}

/// Header information passed to draw executors.
#[derive(Clone, Copy, Debug)]
pub struct RenderSessionHeader {
    pub composition_size: (u32, u32),
    pub fps: u32,
    pub frames: u32,
}
