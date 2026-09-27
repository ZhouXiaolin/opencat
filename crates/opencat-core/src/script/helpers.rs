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
        #[serde(rename = "assetId")]
        asset_id: String,
        #[serde(rename = "tileX", default = "default_tile_mode")]
        tile_x: TileModeName,
        #[serde(rename = "tileY", default = "default_tile_mode")]
        tile_y: TileModeName,
    },
    #[serde(rename = "picture")]
    Picture {
        #[serde(rename = "ownerId")]
        owner_id: String,
        // Tile modes accepted for parity with the JS API; the engine currently
        // samples picture-as-shader with TileMode::Clamp regardless.
        #[serde(rename = "tileX", default = "default_tile_mode")]
        _tile_x: TileModeName,
        #[serde(rename = "tileY", default = "default_tile_mode")]
        _tile_y: TileModeName,
    },
    /// Frame-scoped generated image (script `surface.bake` / putImageData
    /// path). Pixels were registered for THIS frame via the pending
    /// generated-image table.
    #[serde(rename = "generated")]
    Generated {
        key: String,
        #[serde(rename = "tileX", default = "default_tile_mode")]
        _tile_x: TileModeName,
        #[serde(rename = "tileY", default = "default_tile_mode")]
        _tile_y: TileModeName,
    },
    /// Session-scoped offscreen surface (render target). CPU-side lambda
    /// backends sample its pixels directly; SKSL-path children must bake to a
    /// generated image first (surfaces live in core, not on the wire).
    #[serde(rename = "surface")]
    Surface {
        id: String,
        #[serde(rename = "tileX", default = "default_tile_mode")]
        _tile_x: TileModeName,
        #[serde(rename = "tileY", default = "default_tile_mode")]
        _tile_y: TileModeName,
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
    /// SKSL / wire 侧的 child 形态。`Surface` 不能跨线（surfaces 是核心
    /// 进程内的 session 缓冲）——需要先 `surface.bake(key)` 成 generated。
    pub fn to_script_child(&self) -> anyhow::Result<crate::ir::draw_types::ScriptRuntimeEffectChild> {
        let child = match self {
            ScriptChildSpec::Image { asset_id, .. } => {
                crate::ir::draw_types::ScriptRuntimeEffectChild::Image(
                    crate::ir::draw_types::ImageRef::Static {
                        asset_id: asset_id.clone(),
                    },
                )
            }
            ScriptChildSpec::Picture { owner_id, .. } => {
                crate::ir::draw_types::ScriptRuntimeEffectChild::PictureSubtree {
                    owner_id: owner_id.clone(),
                }
            }
            ScriptChildSpec::Generated { key, .. } => {
                crate::ir::draw_types::ScriptRuntimeEffectChild::Image(
                    crate::ir::draw_types::ImageRef::Generated {
                        id: crate::ir::GeneratedImageId::from_key(key),
                    },
                )
            }
            ScriptChildSpec::Surface { id, .. } => {
                return Err(anyhow::anyhow!(
                    "surface child `{id}` 不能用于 SKSL/绘制路径：请先 surface.bake(key) 烘为 generated child（CPU lambda 可直接采样 surface）"
                ));
            }
        };
        Ok(child)
    }
}

pub fn parse_script_children(json: &str) -> Result<Vec<ScriptChildSpec>, anyhow::Error> {
    serde_json::from_str(json).map_err(|e| anyhow::anyhow!("children_json decode: {e}"))
}

/// f32 平铺 uniforms → `Val`（按 spec 声明序消费；CPU 解释器入口共用）。
pub fn uniform_f32s_to_vals(
    uniforms_spec: &[crate::script::effects_lambda::program::UniformSpec],
    uniforms: &[f32],
) -> anyhow::Result<Vec<crate::script::effects_lambda::program::Val>> {
    use crate::script::effects_lambda::program::Val;
    let mut cursor = 0usize;
    let mut out = Vec::with_capacity(uniforms_spec.len());
    for u in uniforms_spec {
        let n = u.ty.vec_len().unwrap_or(1);
        if cursor + n > uniforms.len() {
            anyhow::bail!(
                "lambda uniforms: need {} f32s, got {}",
                uniforms_spec
                    .iter()
                    .map(|u| u.ty.vec_len().unwrap_or(1))
                    .sum::<usize>(),
                uniforms.len()
            );
        }
        let take = |i: usize| uniforms[cursor + i] as f64;
        let val = match n {
            1 => Val::F(take(0)),
            2 => Val::V2([take(0), take(1)]),
            3 => Val::V3([take(0), take(1), take(2)]),
            _ => Val::V4([take(0), take(1), take(2), take(3)]),
        };
        cursor += n;
        out.push(val);
    }
    Ok(out)
}

/// CPU 后端的 child 像素解析：`generated` → 本帧 pending 表（先烘焙），
/// `surface` → 核心内 session 级 surface（render target 直接采样）。
pub fn cpu_child_images(
    store: &crate::script::recorder::MutationStore,
    children: &[ScriptChildSpec],
) -> anyhow::Result<Vec<crate::script::effects_lambda::interp::ChildImage>> {
    use crate::script::effects_lambda::interp::ChildImage;
    let mut out = Vec::new();
    for c in children {
        match c {
            ScriptChildSpec::Generated { key, .. } => {
                let gid = crate::ir::GeneratedImageId::from_key(key);
                let Some((w, h, rgba)) = store.pending_generated_image(&gid) else {
                    anyhow::bail!(
                        "lambda CPU backend: generated child `{key}` 本帧未注册（需先烘焙）"
                    );
                };
                out.push(ChildImage { width: w, height: h, rgba: rgba.clone() });
            }
            ScriptChildSpec::Surface { id, .. } => {
                let (w, h) = crate::text::surface::surface_dimensions(id)?;
                let rgba = crate::text::surface::surface_rgba_arc(id)?;
                out.push(ChildImage { width: w, height: h, rgba });
            }
            _ => {
                anyhow::bail!(
                    "lambda CPU backend: 仅支持 generated / surface child，当前 child 类型不受支持"
                );
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod script_children_tests {
    use super::*;

    #[test]
    fn parses_image_child_spec_with_tile_modes() {
        let specs = parse_script_children(
            r#"[{"__opencatShader":"image","assetId":"decor","tileX":"clamp","tileY":"repeat"}]"#,
        )
        .unwrap();
        assert_eq!(specs.len(), 1);
        match &specs[0] {
            ScriptChildSpec::Image { asset_id, .. } => assert_eq!(asset_id, "decor"),
            other => panic!("expected image spec, got {other:?}"),
        }
    }

    #[test]
    fn parses_picture_child_spec() {
        let specs = parse_script_children(
            r#"[{"__opencatShader":"picture","ownerId":"c-card","tileX":"clamp","tileY":"clamp"}]"#,
        )
        .unwrap();
        assert_eq!(specs.len(), 1);
        match &specs[0] {
            ScriptChildSpec::Picture { owner_id, .. } => assert_eq!(owner_id, "c-card"),
            other => panic!("expected picture spec, got {other:?}"),
        }
    }
}
