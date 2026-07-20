use opencat_core::ir::draw_encoding::encode_draw_frame;
use opencat_core::ir::draw_frame::{DrawFrameScratch, DrawOpFrame};
use opencat_core::ir::media_plan::FrameMediaPlan;
use wasm_bindgen::JsValue;

use crate::wasm_bridge::{GeneratedImageRecord, encode_ir_envelope, intern_image_strings};

// ---------------------------------------------------------------------------
// Web-owned frame consumer seam.
//
// `RenderSessionHeader` + `FrameConsumer` previously lived in core's
// `platform` module, but core never implements or invokes them — only the
// engine and web hosts do. They now live in each host: here for the web
// (CanvasKit path), and in `opencat-engine::consumer` for the Skia path.
// The trait shape is intentionally identical so the two hosts stay in lockstep.
// ---------------------------------------------------------------------------

/// Header information passed to web frame consumers.
#[derive(Clone, Copy, Debug)]
pub struct RenderSessionHeader {
    pub composition_size: (u32, u32),
    pub fps: u32,
    pub frames: u32,
}

/// A consumer that processes a single rendered frame (web / CanvasKit path).
pub trait FrameConsumer {
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;

    fn consume_frame(
        &mut self,
        header: &RenderSessionHeader,
        draw: &mut DrawOpFrame,
        plan: &FrameMediaPlan,
    ) -> Result<Self::Output, Self::Error>;
}

/// Error wrapper bridging JsValue into std::error::Error.
#[derive(Debug)]
pub struct WebConsumeError(pub String);

impl std::fmt::Display for WebConsumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for WebConsumeError {}

impl From<WebConsumeError> for JsValue {
    fn from(e: WebConsumeError) -> Self {
        JsValue::from_str(&e.0)
    }
}

pub(crate) struct WebFrameConsumer<'a> {
    pub scratch: &'a mut DrawFrameScratch,
    /// Pipeline epoch stamped into the envelope header. JS keys its
    /// generated-image cache by `(epoch, id)` and evicts stale entries when the
    /// epoch bumps.
    pub pipeline_epoch: u32,
    /// Per-frame generated-image delta: glyphs whose RGBA has not yet been
    /// published in this epoch. Carried in section 12 of the envelope.
    pub generated_delta: &'a [GeneratedImageRecord],
}

impl FrameConsumer for WebFrameConsumer<'_> {
    type Output = Vec<u8>;
    type Error = WebConsumeError;

    fn consume_frame(
        &mut self,
        _header: &RenderSessionHeader,
        draw: &mut DrawOpFrame,
        _plan: &FrameMediaPlan,
    ) -> Result<Vec<u8>, WebConsumeError> {
        intern_image_strings(draw);
        let encoded = encode_draw_frame(draw, self.scratch);
        encode_ir_envelope(draw, &encoded, self.pipeline_epoch, self.generated_delta)
            .map_err(WebConsumeError)
    }
}
