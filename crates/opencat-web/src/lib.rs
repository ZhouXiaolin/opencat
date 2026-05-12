//! opencat-web — WASM/Web rendering target for opencat-core.

pub mod backend;
pub mod engine;
pub mod platform;
pub mod recorder;
pub mod video;
pub mod wasm_bridge;

#[cfg(target_arch = "wasm32")]
mod wasm_entry;
