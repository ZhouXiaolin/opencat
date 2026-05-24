use opencat_core::ir::draw_encoding::encode_draw_frame;
use opencat_core::ir::draw_frame::{DrawFrameScratch, DrawOpFrame};
use opencat_core::ir::media_plan::FrameMediaPlan;
use opencat_core::platform::frame_consumer::{FrameConsumer, RenderSessionHeader};
use wasm_bindgen::JsValue;

use crate::wasm_bridge::{encode_ir_envelope, intern_image_strings};

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

pub struct WebFrameConsumer<'a> {
    pub scratch: &'a mut DrawFrameScratch,
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
        encode_ir_envelope(draw, &encoded)
            .map_err(|js| WebConsumeError(js.as_string().unwrap_or_else(|| "encode failed".into())))
    }
}
