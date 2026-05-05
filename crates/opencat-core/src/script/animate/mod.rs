//! Animation state machines used by the script runtime.
//!
//! These are pure Rust algorithms (no skia / quickjs / wasm deps) and live
//! in core so that both the engine bindings and wasm-bindgen wrappers can
//! drive them through the same `MutationRecorder` API.

pub mod color;
pub mod morph_svg;
pub mod path_measure;
pub mod state;

pub use color::{HSLA, hsl_to_rgb, hsla_to_rgba_string, lerp_hsla_clamped, parse_color};
pub use morph_svg::{MorphSvgEntry, MorphSvgState};
pub use path_measure::{PathMeasureEntry, PathMeasureState};
pub use state::{AnimateEntry, AnimateState};
