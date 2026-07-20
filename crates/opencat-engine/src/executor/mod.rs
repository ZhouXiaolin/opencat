pub mod paint;
pub mod path;
mod replay;

use opencat_core::ir::cache::CachedDrawRange;
use opencat_core::ir::draw_frame::DrawOpFrame;
use opencat_core::ir::draw_types::ImageRef;
use crate::consumer::RenderSessionHeader;
use skia_safe::{Canvas, Image, Paint, PathBuilder, RuntimeEffect, skottie::Animation};
use std::collections::HashMap;
use std::path::Path;

/// Statistics returned after a frame execution.
#[derive(Debug, Default)]
pub struct DrawStats {
    pub op_count: u32,
    pub cache_hits: u32,
}

/// Error type for draw execution failures.
#[derive(Debug)]
pub struct DrawError(pub String);

impl std::fmt::Display for DrawError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DrawError: {}", self.0)
    }
}

impl std::error::Error for DrawError {}

/// Engine-side draw executor. Owns the current canvas state
/// and replays DrawOpFrame onto a skia_safe::Canvas.
pub struct EngineDrawExecutor {
    pub(crate) current_path: Option<PathBuilder>,
    pub(crate) current_fill_paint: Paint,
    pub(crate) current_stroke_paint: Paint,
    pub(crate) current_alpha: f32,
    pub(crate) compiled_pictures: HashMap<u64, skia_safe::Picture>,
    pub(crate) lottie_cache: HashMap<String, Animation>,
}

impl EngineDrawExecutor {
    pub fn new() -> Self {
        Self {
            current_path: None,
            current_fill_paint: Paint::default(),
            current_stroke_paint: Paint::default(),
            current_alpha: 1.0,
            compiled_pictures: HashMap::new(),
            lottie_cache: HashMap::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.current_path = None;
        self.current_fill_paint = Paint::default();
        let mut sp = Paint::default();
        sp.set_style(skia_safe::paint::Style::Stroke);
        self.current_stroke_paint = sp;
        self.current_alpha = 1.0;
    }
}

impl Default for EngineDrawExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Prepared frame media for executor consumption.
#[derive(Default)]
pub struct EnginePreparedFrameMedia {
    pub images: Vec<Image>,
    pub image_index: HashMap<ImageRef, usize>,
    pub runtime_effects: Vec<RuntimeEffect>,
}

impl EngineDrawExecutor {
    pub fn execute(
        &mut self,
        _header: &RenderSessionHeader,
        draw: &DrawOpFrame,
        media: &EnginePreparedFrameMedia,
        target: &mut Canvas,
    ) -> Result<DrawStats, DrawError> {
        self.begin_frame();
        replay::replay_frame(self, target, draw, media)
    }

    pub fn ensure_lottie_animations<P: Fn(&str) -> Option<Vec<u8>>>(
        &mut self,
        draw: &DrawOpFrame,
        resolve_bytes: P,
    ) {
        use opencat_core::ir::draw_op::DrawOp;
        let bundle_ids: Vec<String> = draw
            .ops
            .iter()
            .chain(draw.subtrees.iter().flat_map(|s| s.iter()))
            .filter_map(|op| match op {
                DrawOp::LottieRect { bundle_id, .. } => Some(bundle_id.clone()),
                _ => None,
            })
            .filter(|id| !self.lottie_cache.contains_key(id))
            .collect();
        for bundle_id in bundle_ids {
            if let Some(bytes) = resolve_bytes(&bundle_id) {
                if let Ok(json) = std::str::from_utf8(&bytes) {
                    if let Some(anim) = Animation::from_str(json) {
                        self.lottie_cache.insert(bundle_id, anim);
                    }
                }
            }
        }
    }

    pub fn compile_range(
        &mut self,
        _cached: &CachedDrawRange,
        _draw: &DrawOpFrame,
    ) -> Result<(), DrawError> {
        // TODO(optimization): compile range via PictureRecorder
        // Requires EnginePreparedFrameMedia which needs full pipeline integration
        Ok(())
    }

    pub fn evict_range(&mut self, fingerprint: u64) {
        self.compiled_pictures.remove(&fingerprint);
    }
}
