use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BitmapSourceKind {
    StaticImage,
    Video,
}

pub(crate) fn bitmap_source_kind(path: &Path) -> BitmapSourceKind {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4" | "mov" | "m4v" | "webm" | "mkv" | "avi") => BitmapSourceKind::Video,
        _ => BitmapSourceKind::StaticImage,
    }
}
