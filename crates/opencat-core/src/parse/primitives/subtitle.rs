use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SubtitleSource {
    Path(PathBuf),
    Url(String),
}
