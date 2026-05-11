use crate::render::RenderBackend;

pub(crate) fn default_render_backend() -> RenderBackend {
    #[cfg(target_os = "macos")]
    {
        // macOS supports both software and accelerated; default to software
        // until the accelerated path is wired through the core pipeline.
        RenderBackend::Software
    }
    #[cfg(not(target_os = "macos"))]
    {
        RenderBackend::Software
    }
}
