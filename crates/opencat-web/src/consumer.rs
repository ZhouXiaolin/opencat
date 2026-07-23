use opencat_core::ir::draw_encoding::encode_ir_envelope;
use opencat_core::ir::draw_frame::DrawFrameScratch;
use opencat_core::ir::RenderFrame;

/// Error wrapper bridging encoding failures into std::error::Error / JsValue.
#[derive(Debug)]
pub struct WebConsumeError(pub String);

impl std::fmt::Display for WebConsumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for WebConsumeError {}

/// Encode a core RenderFrame into the single self-contained OCIR envelope
/// (issue #45). The same RenderFrame always produces byte-identical output;
/// no epoch/delta/history state is needed. Generated-image RGBA is fully
/// encoded every frame.
///
/// Core owns the full versioned wire protocol (#22). This host path only interns
/// image strings and forwards bytes — no second envelope, no protocol re-encoding.
pub(crate) fn encode_render_frame_envelope(
    render_frame: &mut RenderFrame,
    scratch: &mut DrawFrameScratch,
) -> Result<Vec<u8>, WebConsumeError> {
    // Intern any asset_id / bundle_id strings so the binary IR can reference them.
    opencat_core::ir::draw_encoding::intern_image_strings(&mut render_frame.draw);
    encode_ir_envelope(render_frame, scratch).map_err(|e| WebConsumeError(e.0))
}
