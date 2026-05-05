use wasm_bindgen::prelude::*;

use opencat_core::jsonl::JsonLine;

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
    let mut width = 0i32;
    let mut height = 0i32;
    let mut fps = 0i32;
    let mut frames = 0i32;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(JsonLine::Composition { width: w, height: h, fps: f, frames: fs }) =
            serde_json::from_str(trimmed)
        {
            width = w;
            height = h;
            fps = f;
            frames = fs;
            break;
        }
    }

    serde_json::json!({
        "width": width,
        "height": height,
        "fps": fps,
        "frames": frames
    }).to_string()
}
