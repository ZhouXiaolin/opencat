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

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "__opencatShader")]
pub enum ScriptChildSpec {
    #[serde(rename = "image")]
    Image {
        #[serde(rename = "assetId")] asset_id: String,
        #[serde(rename = "tileX", default = "default_tile_mode")] tile_x: TileModeName,
        #[serde(rename = "tileY", default = "default_tile_mode")] tile_y: TileModeName,
    },
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TileModeName {
    Clamp,
    Repeat,
    Mirror,
    Decal,
}

fn default_tile_mode() -> TileModeName {
    TileModeName::Clamp
}

impl ScriptChildSpec {
    pub fn to_ir_child_ref(&self) -> crate::ir::draw_types::RuntimeEffectChildRef {
        match self {
            ScriptChildSpec::Image { asset_id, .. } => {
                crate::ir::draw_types::RuntimeEffectChildRef::Image(
                    crate::ir::draw_types::ImageRef::Static {
                        asset_id: asset_id.clone(),
                    },
                )
            }
        }
    }
}

pub fn parse_script_children(json: &str) -> Result<Vec<ScriptChildSpec>, anyhow::Error> {
    serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("children_json decode: {e}"))
}

#[cfg(test)]
mod script_children_tests {
    use super::*;

    #[test]
    fn parses_image_child_spec_with_tile_modes() {
        let specs = parse_script_children(
            r#"[{"__opencatShader":"image","assetId":"decor","tileX":"clamp","tileY":"repeat"}]"#,
        ).unwrap();
        assert_eq!(specs.len(), 1);
        match &specs[0] {
            ScriptChildSpec::Image { asset_id, .. } => assert_eq!(asset_id, "decor"),
        }
    }

    #[test]
    fn rejects_unknown_shader_kind() {
        let err = parse_script_children(
            r#"[{"__opencatShader":"picture","ownerId":"foo"}]"#,
        ).err().expect("should reject picture child for now");
        assert!(err.to_string().contains("decode"));
    }
}
