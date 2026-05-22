use std::fmt;

#[cfg(feature = "profile")]
use tracing::{Level, event};

use crate::cache::lru::CacheMutationReport;

/// Emit tracing events for LRU cache eviction / replacement / utilization.
#[cfg(feature = "profile")]
pub(crate) fn record_cache_pressure<K>(cache_name: &'static str, report: &CacheMutationReport<K>) {
    if !report.evicted.is_empty() {
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "eviction",
            name = cache_name,
            result = "count",
            amount = report.evicted.len() as u64
        );
    }
    if report.replaced {
        event!(
            target: "render.cache",
            Level::TRACE,
            kind = "repeat",
            name = cache_name,
            result = "count",
            amount = 1_u64
        );
    }
    event!(
        target: "render.cache",
        Level::TRACE,
        kind = "utilization",
        name = cache_name,
        result = "count",
        amount = report.utilization as u64
    );
}

#[cfg(not(feature = "profile"))]
pub(crate) fn record_cache_pressure<K>(_cache_name: &'static str, _report: &CacheMutationReport<K>) {}

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

pub mod ctx;
pub mod media_plan;
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

pub use ctx::RenderCtx;
pub use state::DrawScriptPaintState;
