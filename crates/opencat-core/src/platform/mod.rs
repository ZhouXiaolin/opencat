//! Platform abstractions: traits and generic data structures shared across
//! backends (engine/skia, web/wasm, future native backends).
//!
//! The `Platform` facade trait aggregates ScriptHost, ResourcePlatform,
//! MediaPlatform, and DrawPlatform for each backend.

pub mod draw;
pub mod media;
#[allow(clippy::module_inception)]
pub mod platform;
pub mod resource;
pub mod video;
