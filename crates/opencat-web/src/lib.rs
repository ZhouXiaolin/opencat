//! opencat-web — WASM/Web rendering target for opencat-core.

#[cfg(target_arch = "wasm32")]
pub mod canvaskit;

pub mod codec;
pub mod platform;
pub mod recorder;
pub mod resource;
pub mod video;
pub mod wasm_bridge;
