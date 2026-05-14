use wasm_bindgen::prelude::*;

use opencat_core::display::build::build_display_tree;
use opencat_core::display::list::DisplayItem;
use opencat_core::display::tree::{DisplayNode, DisplayTree};
use opencat_core::element::resolve::resolve_ui_tree;
use opencat_core::frame_ctx::FrameCtx;
use opencat_core::jsonl::JsonLine;
use opencat_core::layout::LayoutSession;
use opencat_core::resource::asset_id::asset_id_for_query;
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::scene::primitives::OpenverseQuery;
use opencat_core::scene::script::PrecomputedScriptHost;
use opencat_core::scene::script::mutations::StyleMutations;
use opencat_core::text;
use opencat_core::text::{rasterization_to_display_glyphs, rasterize_glyphs};

fn parse_composition_info(input: &str) -> Option<(i32, i32, i32, i32)> {
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(JsonLine::Composition {
            width: w,
            height: h,
            fps: f,
            frames: fs,
        }) = serde_json::from_str(trimmed)
        {
            return Some((w, h, f, fs));
        }
    }
    None
}

#[wasm_bindgen]
pub fn parse_jsonl(input: &str) -> String {
    let mut composition: Option<serde_json::Value> = None;
    let mut elements: Vec<serde_json::Value> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<JsonLine>(trimmed) {
            Ok(JsonLine::Composition {
                width,
                height,
                fps,
                frames,
            }) => {
                composition = Some(serde_json::json!({
                    "width": width,
                    "height": height,
                    "fps": fps,
                    "frames": frames
                }));
            }
            Ok(parsed) => {
                let value = serde_json::to_value(&parsed).unwrap_or_default();
                elements.push(value);
            }
            Err(e) => {
                elements.push(serde_json::json!({
                    "type": "parse_error",
                    "error": e.to_string(),
                    "raw": trimmed
                }));
            }
        }
    }

    serde_json::json!({
        "composition": composition,
        "elements": elements,
        "elementCount": elements.len()
    })
    .to_string()
}

#[wasm_bindgen]
pub fn get_composition_info(input: &str) -> String {
    let (width, height, fps, frames) = parse_composition_info(input).unwrap_or((0, 0, 0, 0));

    serde_json::json!({
        "width": width,
        "height": height,
        "fps": fps,
        "frames": frames
    })
    .to_string()
}

/// Collect resource requests from JSONL input.
/// Returns JSON with lists of required images, videos, audios, and icons.
#[wasm_bindgen]
pub fn collect_resources_json(input: &str) -> String {
    let mut images: Vec<String> = Vec::new();
    let mut videos: Vec<String> = Vec::new();
    let mut audios: Vec<String> = Vec::new();
    let mut icons: Vec<String> = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<JsonLine>(trimmed) {
            match parsed {
                JsonLine::Image {
                    path,
                    url,
                    query,
                    query_count,
                    aspect_ratio,
                    ..
                } => {
                    if let Some(p) = path {
                        images.push(p);
                    }
                    if let Some(u) = url {
                        images.push(format!("url:{u}"));
                    }
                    if let Some(q) = query {
                        let count = query_count.map(|n| n as usize).unwrap_or(1);
                        let q = OpenverseQuery {
                            query: q,
                            count,
                            aspect_ratio,
                        };
                        images.push(asset_id_for_query(&q).0);
                    }
                }
                JsonLine::Video { path, url, .. } => {
                    if let Some(u) = url {
                        videos.push(format!("video:url:{u}"));
                    } else if let Some(p) = path {
                        videos.push(p);
                    }
                }
                JsonLine::Audio { path, url, .. } => {
                    if let Some(p) = path {
                        audios.push(p);
                    }
                    if let Some(u) = url {
                        audios.push(format!("audio:url:{u}"));
                    }
                }
                JsonLine::Icon { icon, .. } => {
                    icons.push(icon);
                }
                _ => {}
            }
        }
    }

    serde_json::json!({
        "images": images,
        "videos": videos,
        "audios": audios,
        "icons": icons,
    })
    .to_string()
}

/// Build display tree for a single frame.
/// Returns: DisplayTree JSON or `{"error": "message"}` on failure
///
/// **Deprecated**: Prefer `WebRenderer::build_frame` from `wasm_bridge`,
/// which runs through the full render pipeline and returns ordered-scene ops.
/// This legacy entry point is kept for backward compatibility with existing JS code.
#[wasm_bindgen]
pub fn build_frame(
    jsonl_input: &str,
    frame: u32,
    resource_meta: &str,
    mutations_json: &str,
) -> String {
    match build_frame_impl(jsonl_input, frame, resource_meta, mutations_json) {
        Ok(tree) => serde_json::to_string(&tree)
            .unwrap_or_else(|e| format!(r#"{{"error":"serialization failed: {}"}}"#, e)),
        Err(e) => format!(r#"{{"error":"{}"}}"#, e),
    }
}

fn build_frame_impl(
    jsonl_input: &str,
    frame: u32,
    resource_meta: &str,
    mutations_json: &str,
) -> anyhow::Result<DisplayTree> {
    // 1. Parse JSONL
    let parsed = opencat_core::jsonl::parse(jsonl_input)?;

    let frame_ctx = FrameCtx {
        frame,
        frames: parsed.frames as u32,
        fps: parsed.fps as u32,
        width: parsed.width,
        height: parsed.height,
    };

    // 2. Build resource catalog from JS-provided metadata
    let mut catalog = HashMapResourceCatalog::from_json(resource_meta)?;

    // 3. Parse style mutations from JS script engine
    let mutations: StyleMutations =
        serde_json::from_str(mutations_json).unwrap_or_default();

    let effective_mutations: Option<&StyleMutations> = if mutations.is_empty() {
        None
    } else {
        Some(&mutations)
    };

    // 4. Build script host from parsed mutations (script elements in tree)
    let mut script_host = PrecomputedScriptHost::from_single(mutations.clone());

    // 5. Get the scene node for this frame
    let scene_node = parsed.root;

    // 6. Resolve UI tree with mutations applied directly
    let element_root = resolve_ui_tree(
        &scene_node,
        &frame_ctx,
        &mut catalog,
        effective_mutations,
        &mut script_host,
    )?;

    // 7. Compute layout
    let font_db = text::default_font_db_with_embedded_only();
    let mut layout_session = LayoutSession::default();
    let (layout_tree, _) =
        layout_session.compute_layout_with_font_db(&element_root, &frame_ctx, &font_db)?;

    // 8. Build display tree
    let mut display_tree = build_display_tree(&element_root, &layout_tree)?;

    // 9. Enrich text nodes with cosmic-text glyph rasterization data
    enrich_text_with_glyphs(&mut display_tree);

    Ok(display_tree)
}

/// Walk the display tree and attach rasterized glyph data to every
/// `TextDisplayItem` so the web renderer can draw text paths/images
/// without needing its own font engine.
fn enrich_text_with_glyphs(tree: &mut DisplayTree) {
    enrich_node_with_glyphs(&mut tree.root);
}

fn enrich_node_with_glyphs(node: &mut DisplayNode) {
    if let DisplayItem::Text(text_item) = &mut node.item
        && text_item.glyphs.is_none()
    {
        let truncate = if text_item.text_unit_overrides.is_some() {
            false
        } else {
            text_item.truncate
        };
        let raster = rasterize_glyphs(
            &text_item.text,
            &text_item.style,
            text_item.bounds.width,
            text_item.allow_wrap,
            truncate,
        );
        text_item.glyphs = Some(rasterization_to_display_glyphs(&raster));
    }
    for child in &mut node.children {
        enrich_node_with_glyphs(child);
    }
}
