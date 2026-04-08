use anyhow::{Result, anyhow};

use crate::{
    backend::skia::renderer as skia_renderer,
    render::RenderBackend,
    runtime::{
        render_engine::SharedRenderEngine, target::RenderSurfaceKind, text_engine::SharedTextEngine,
    },
};

struct RegisteredFrameBackend {
    backend: RenderBackend,
    engine_factory: fn() -> SharedRenderEngine,
    default: bool,
    accelerated: bool,
    available: fn() -> bool,
}

struct RegisteredSurfaceBackend {
    surface_kind: RenderSurfaceKind,
    engine_factory: fn() -> SharedRenderEngine,
}

fn always_available() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn accelerated_available() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
fn accelerated_available() -> bool {
    false
}

const FRAME_BACKENDS: &[RegisteredFrameBackend] = &[
    RegisteredFrameBackend {
        backend: RenderBackend::Software,
        engine_factory: skia_renderer::shared_raster_engine,
        default: true,
        accelerated: false,
        available: always_available,
    },
    RegisteredFrameBackend {
        backend: RenderBackend::Accelerated,
        engine_factory: skia_renderer::shared_metal_engine,
        default: false,
        accelerated: true,
        available: accelerated_available,
    },
];

const SURFACE_BACKENDS: &[RegisteredSurfaceBackend] = &[RegisteredSurfaceBackend {
    surface_kind: RenderSurfaceKind::Canvas,
    engine_factory: skia_renderer::shared_raster_engine,
}];

pub(crate) fn default_render_backend() -> RenderBackend {
    FRAME_BACKENDS
        .iter()
        .find(|entry| entry.default)
        .map(|entry| entry.backend)
        .expect("at least one frame backend must be registered")
}

pub(crate) fn default_render_engine() -> SharedRenderEngine {
    render_engine_for_backend(default_render_backend())
        .expect("default render backend must resolve to a registered engine")
}

pub(crate) fn default_text_engine() -> SharedTextEngine {
    default_render_engine().text_engine()
}

pub(crate) fn render_engine_for_backend(backend: RenderBackend) -> Result<SharedRenderEngine> {
    let entry = FRAME_BACKENDS
        .iter()
        .find(|entry| entry.backend == backend)
        .ok_or_else(|| anyhow!("no render engine registered for backend {backend:?}"))?;
    if !(entry.available)() {
        let backend_class = if entry.accelerated {
            "accelerated"
        } else {
            "software"
        };
        return Err(anyhow!(
            "{backend_class} render backend {backend:?} is not available on this platform"
        ));
    }
    Ok((entry.engine_factory)())
}

pub(crate) fn render_engine_for_surface_kind(
    surface_kind: RenderSurfaceKind,
) -> Result<SharedRenderEngine> {
    SURFACE_BACKENDS
        .iter()
        .find(|entry| entry.surface_kind == surface_kind)
        .map(|entry| (entry.engine_factory)())
        .ok_or_else(|| anyhow!("no render engine registered for target surface {surface_kind:?}"))
}
