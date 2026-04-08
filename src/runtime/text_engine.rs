use std::sync::Arc;

use crate::style::ComputedTextStyle;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextMeasurement {
    pub width: f32,
    pub height: f32,
}

pub(crate) struct TextMeasureRequest<'a> {
    pub text: &'a str,
    pub style: &'a ComputedTextStyle,
    pub max_width: f32,
    pub allow_wrap: bool,
}

pub(crate) trait TextEngine: Send + Sync {
    fn measure(&self, request: &TextMeasureRequest<'_>) -> TextMeasurement;
}

pub(crate) type SharedTextEngine = Arc<dyn TextEngine>;
