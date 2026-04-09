use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AudioSource {
    Unset,
    Path(PathBuf),
    Url(String),
}
