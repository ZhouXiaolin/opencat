use std::sync::OnceLock;

/// Returns a shared `Arc<SkiaRenderEngine>` — kept for compatibility with
/// older code paths that still reference this type.
pub fn shared_raster_engine_typed() -> std::sync::Arc<SkiaRenderEngine> {
    static ENGINE: OnceLock<std::sync::Arc<SkiaRenderEngine>> = OnceLock::new();
    ENGINE
        .get_or_init(|| std::sync::Arc::new(SkiaRenderEngine))
        .clone()
}

pub struct SkiaRenderEngine;
