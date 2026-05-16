use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    Platform(String),
    MissingResource(String),
    InvalidArgument(String),
}

impl From<&str> for RenderError {
    fn from(s: &str) -> Self {
        RenderError::Platform(s.to_string())
    }
}

impl From<String> for RenderError {
    fn from(s: String) -> Self {
        RenderError::Platform(s)
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Platform(s) => write!(f, "platform error: {}", s),
            RenderError::MissingResource(s) => write!(f, "missing resource: {}", s),
            RenderError::InvalidArgument(s) => write!(f, "invalid argument: {}", s),
        }
    }
}

impl std::error::Error for RenderError {}

pub mod cache;
pub mod ctx;
pub mod state;

// TODO: implement in later chunks
pub mod paint_conv;
pub mod script_conv;
pub mod display_tree;
pub mod display_item;
pub mod rect;
pub mod text;
pub mod bitmap;
pub mod svg_path;
pub mod draw_script;
pub mod timeline;
pub mod transition;

pub use cache::RenderCache;
pub use ctx::RenderCtx;
pub use state::DrawScriptPaintState;
