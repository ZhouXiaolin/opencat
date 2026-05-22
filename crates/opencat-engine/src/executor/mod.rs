pub mod paint;
pub mod path;
mod replay;

use opencat_core::draw::cache::CachedDrawRange;
use opencat_core::draw::frame::DrawOpFrame;
use opencat_core::draw::types::ImageRef;
use opencat_core::platform::draw::{DrawError, DrawPlatform, DrawStats, RenderSessionHeader};
use skia_safe::{Canvas, Image, Paint, PathBuilder, RuntimeEffect};
use std::collections::HashMap;

/// Engine-side draw executor. Owns the current canvas state
/// and replays DrawOpFrame onto a skia_safe::Canvas.
pub struct EngineDrawExecutor {
    pub(crate) current_path: Option<PathBuilder>,
    pub(crate) current_fill_paint: Paint,
    pub(crate) current_stroke_paint: Paint,
    pub(crate) current_alpha: f32,
    pub(crate) compiled_pictures: HashMap<u64, skia_safe::Picture>,
}

impl EngineDrawExecutor {
    pub fn new() -> Self {
        Self {
            current_path: None,
            current_fill_paint: Paint::default(),
            current_stroke_paint: Paint::default(),
            current_alpha: 1.0,
            compiled_pictures: HashMap::new(),
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

impl DrawPlatform for EngineDrawExecutor {
    type Target = Canvas;
    type PreparedFrameMedia = EnginePreparedFrameMedia;

    fn execute(
        &mut self,
        _header: &RenderSessionHeader,
        draw: &DrawOpFrame,
        media: &Self::PreparedFrameMedia,
        target: &mut Self::Target,
    ) -> Result<DrawStats, DrawError> {
        self.begin_frame();
        replay::replay_frame(self, target, draw, media)
    }

    fn compile_range(
        &mut self,
        _cached: &CachedDrawRange,
        _draw: &DrawOpFrame,
    ) -> Result<(), DrawError> {
        // TODO(optimization): compile range via PictureRecorder
        // Requires EnginePreparedFrameMedia which needs full pipeline integration
        Ok(())
    }

    fn evict_range(&mut self, fingerprint: u64) {
        self.compiled_pictures.remove(&fingerprint);
    }
}
