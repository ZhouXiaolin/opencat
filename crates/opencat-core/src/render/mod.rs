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

pub(crate) mod builder;
pub(crate) mod ctx;
pub(crate) mod dispatch;
pub(crate) mod canvas_lower;
pub(crate) mod gradient;
pub(crate) mod items;
pub(crate) mod shadow_filter;
pub(crate) mod transition;
pub(crate) mod media_plan;
pub(crate) mod offscreen;
pub(crate) mod svg;
pub(crate) mod text;

pub(crate) use ctx::RenderCtx;
pub(crate) use canvas_lower::*;
pub(crate) use items::*;
pub(crate) use shadow_filter::*;
pub(crate) use transition::*;
