//! Animation utilities — pure functions used by the script runtime.
//!
//! These are pure Rust algorithms (no skia / v8 / wasm deps).
//! Entry storage lives in `MutationStore` (in `script::recorder::store`).

pub mod color;
pub mod path_measure;
pub mod state;

pub use color::{HSLA, hsl_to_rgb, parse_color};
pub use path_measure::PathMeasureEntry;
pub use state::random_from_seed;
