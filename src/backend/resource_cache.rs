use crate::backend::skia::resources::SkiaBackendResources;

pub(crate) struct BackendResourceCache {
    skia: SkiaBackendResources,
}

impl BackendResourceCache {
    pub(crate) fn new() -> Self {
        Self {
            skia: SkiaBackendResources::new(),
        }
    }

    pub(crate) fn skia(&self) -> &SkiaBackendResources {
        &self.skia
    }
}

impl Default for BackendResourceCache {
    fn default() -> Self {
        Self::new()
    }
}
