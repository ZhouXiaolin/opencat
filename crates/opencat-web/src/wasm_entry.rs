use wasm_bindgen::prelude::*;

use opencat_core::jsonl::JsonLine;

fn parse_composition_info(input: &str) -> Option<(i32, i32, i32, i32)> {
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(JsonLine::Composition { width: w, height: h, fps: f, frames: fs }) =
            serde_json::from_str(trimmed)
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
            Ok(JsonLine::Composition { width, height, fps, frames }) => {
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
    }).to_string()
}

#[wasm_bindgen]
pub fn get_composition_info(input: &str) -> String {
    let (width, height, fps, frames) = parse_composition_info(input).unwrap_or((0, 0, 0, 0));

    serde_json::json!({
        "width": width,
        "height": height,
        "fps": fps,
        "frames": frames
    }).to_string()
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
                JsonLine::Image { path, url, .. } => {
                    if let Some(p) = path {
                        images.push(p);
                    }
                    if let Some(u) = url {
                        images.push(u);
                    }
                }
                JsonLine::Video { path, .. } => {
                    videos.push(path);
                }
                JsonLine::Audio { path, url, .. } => {
                    if let Some(p) = path {
                        audios.push(p);
                    }
                    if let Some(u) = url {
                        audios.push(u);
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
    }).to_string()
}
