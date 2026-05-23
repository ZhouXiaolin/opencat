use std::fmt;

#[cfg(feature = "profile")]
use tracing::{Level, event};

use crate::cache::lru::CacheMutationReport;

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
pub(crate) fn record_cache_pressure<K>(
    _cache_name: &'static str,
    _report: &CacheMutationReport<K>,
) {
}

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
pub(crate) mod helpers;
pub(crate) mod media_plan;
pub(crate) mod text;

pub(crate) use builder::DrawOpBuilder;
pub(crate) use ctx::RenderCtx;
pub(crate) use dispatch::render_display_tree;
pub(crate) use media_plan::build_media_plan;
