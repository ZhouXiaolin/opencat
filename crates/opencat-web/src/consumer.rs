use opencat_core::ir::draw_encoding::encode_draw_frame;
use opencat_core::ir::draw_frame::{DrawFrameScratch, DrawOpFrame};

use crate::wasm_bridge::{GeneratedImageRecord, encode_ir_envelope, intern_image_strings};

/// Error wrapper bridging encoding failures into std::error::Error / JsValue.
#[derive(Debug)]
pub struct WebConsumeError(pub String);

impl std::fmt::Display for WebConsumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for WebConsumeError {}

/// Encode a core draw frame plus generated-image delta into the OCIR envelope.
///
/// Hosts pass RGBA from [`opencat_core::ir::FrameMediaPlan::generated_images`]
/// (delta-filtered by epoch); no FrameConsumer / RenderSessionHeader protocol.
pub(crate) fn encode_render_frame_envelope(
    draw: &mut DrawOpFrame,
    scratch: &mut DrawFrameScratch,
    pipeline_epoch: u32,
    generated_delta: &[GeneratedImageRecord],
) -> Result<Vec<u8>, WebConsumeError> {
    intern_image_strings(draw);
    let encoded = encode_draw_frame(draw, scratch);
    encode_ir_envelope(draw, &encoded, pipeline_epoch, generated_delta).map_err(WebConsumeError)
}
