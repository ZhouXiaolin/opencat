//! Web backend types — plain Rust structs (no wasm-bindgen yet).

/// Subtree snapshot marker. Actual CK.Picture stored in JS-side Map.
#[derive(Clone)]
pub struct WebPicture {
    pub fingerprint: u64,
}

/// Glyph path data produced by cosmic-text, serialized to JS via wasm-bindgen.
#[derive(Clone)]
pub struct GlyphPathData {
    pub commands: Vec<GlyphPathCommand>,
    pub bounds_x: f32,
    pub bounds_y: f32,
    pub bounds_w: f32,
    pub bounds_h: f32,
}

#[derive(Clone)]
pub enum GlyphPathCommand {
    MoveTo { x: f32, y: f32 },
    LineTo { x: f32, y: f32 },
    QuadTo { cx: f32, cy: f32, x: f32, y: f32 },
    CurveTo { cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32 },
    Close,
}
