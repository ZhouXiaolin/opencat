//! Platform abstractions: traits and generic data structures shared across
//! backends (engine/skia, web/wasm, future native backends).
//!
//! `Platform` 门面 trait（聚合 ScriptHost + VideoFrameProvider 等）属于 Phase C。

#[allow(clippy::module_inception)]
pub mod platform;
pub mod video;
