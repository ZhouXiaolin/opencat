//! Skia backend: implements `opencat_core::platform::backend::BackendTypes`.

use opencat_core::platform::backend::BackendTypes;

/// Zero-sized marker for the Skia backend.
pub struct SkiaBackend;

impl BackendTypes for SkiaBackend {
    type Picture = skia_safe::Picture;
    type Image = skia_safe::Image;
    type GlyphPath = skia_safe::Path;
    type GlyphImage = skia_safe::Image;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skia_backend_associated_types_match_skia_types() {
        fn _picture(_p: <SkiaBackend as BackendTypes>::Picture) {}
        fn _image(_i: <SkiaBackend as BackendTypes>::Image) {}
        fn _glyph_path(_p: <SkiaBackend as BackendTypes>::GlyphPath) {}
        fn _glyph_image(_i: <SkiaBackend as BackendTypes>::GlyphImage) {}
    }
}
