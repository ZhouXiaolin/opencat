//! Platform abstractions: traits and generic data structures shared across
//! backends (engine/skia, web/wasm, future native backends).
//!
//! The `Platform` facade trait aggregates ScriptHost, ResourcePlatform,
//! MediaPlatform, and DrawPlatform for each backend.

#[allow(clippy::module_inception)]
pub mod platform;
pub mod video;
pub mod resource;
pub mod media;
pub mod draw;
