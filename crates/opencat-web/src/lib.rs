//! opencat-web — WASM/Web rendering target for opencat-core.

#[cfg(target_arch = "wasm32")]
pub mod canvaskit;

pub mod codec;
#[cfg(target_arch = "wasm32")]
pub mod js_context;
#[cfg(target_arch = "wasm32")]
pub mod platform;
pub mod recorder;
pub mod resource;
#[cfg(target_arch = "wasm32")]
pub mod script;
pub mod video;
#[cfg(target_arch = "wasm32")]
pub mod wasm_bridge;
