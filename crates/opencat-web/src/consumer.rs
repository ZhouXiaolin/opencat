use opencat_core::ir::draw_encoding::{encode_ir_envelope, intern_image_strings};
use opencat_core::ir::draw_frame::{DrawFrameScratch, DrawOpFrame};
use opencat_core::ir::media_plan::FrameGeneratedImage;

/// Error wrapper bridging encoding failures into std::error::Error / JsValue.
#[derive(Debug)]
pub struct WebConsumeError(pub String);

impl std::fmt::Display for WebConsumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for WebConsumeError {}

/// Encode a core draw frame plus generated-image delta into the single OCIR envelope.
///
/// Core owns the full versioned wire protocol (#22). This host path only interns
/// image strings and forwards bytes — no second envelope, no protocol re-encoding.
pub(crate) fn encode_render_frame_envelope(
    draw: &mut DrawOpFrame,
    scratch: &mut DrawFrameScratch,
    pipeline_epoch: u32,
    generated_delta: &[FrameGeneratedImage],
) -> Result<Vec<u8>, WebConsumeError> {
    intern_image_strings(draw);
    encode_ir_envelope(draw, scratch, pipeline_epoch, generated_delta)
        .map_err(|e| WebConsumeError(e.0))
}
