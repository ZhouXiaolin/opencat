//! opencat-web — WASM/Web rendering target for opencat-core.

pub mod backend;
pub mod codec;
pub mod engine;
pub mod platform;
pub mod recorder;
pub mod resource;
pub mod video;
pub mod wasm_bridge;

/// Legacy WASM entry points (parse_jsonl, build_frame, etc.).
/// Prefer `wasm_bridge` for new code.
#[cfg(target_arch = "wasm32")]
mod wasm_entry;
