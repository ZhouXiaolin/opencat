//! opencat-web — WASM/Web rendering target for opencat-core.

pub mod backend;
pub mod engine;
pub mod recorder;

#[cfg(target_arch = "wasm32")]
mod wasm_entry;
