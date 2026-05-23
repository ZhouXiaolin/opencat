//! Runtime-agnostic helper functions for script bindings.
//! All errors are `anyhow::Error`; consumers convert to their own error type.

use anyhow::anyhow;

use crate::ir::draw_op::ColorU8;
use crate::script::{parse_drrect_coords, parse_image_rect_coords, script_color_from_value};

/// Create a binding error from an operation name and message.
pub fn script_error(op: &str, message: String) -> anyhow::Error {
    anyhow!("script binding `{op}`: {message}")
}

/// Parse a color string for script bindings.
pub fn parse_color(color: &str, op: &str) -> anyhow::Result<ColorU8> {
    script_color_from_value(color)
        .ok_or_else(|| script_error(op, format!("unsupported color `{color}`")))
}

/// Parse image rect coordinates for script bindings.
pub fn parse_image_rect(op: &str, coords: &[f32]) -> anyhow::Result<[f32; 4]> {
    parse_image_rect_coords(coords).ok_or_else(|| {
        script_error(
            op,
            "expected source rect as [x, y, width, height]".to_string(),
        )
    })
}

/// Parse DRRect coordinates for script bindings.
pub fn parse_drrect(
    op: &str,
    coords: &[f32],
) -> anyhow::Result<(f32, f32, f32, f32, f32, f32, f32, f32, f32, f32)> {
    parse_drrect_coords(coords)
        .ok_or_else(|| script_error(op, "expected 10 coordinate values".to_string()))
}
