//! Animation utilities — pure functions used by the script runtime.
//!
//! These are pure Rust algorithms (no skia / quickjs / wasm deps).
//! Entry storage lives in `MutationStore` (in `script::recorder::store`).

pub mod color;
pub mod morph_svg;
pub mod path_measure;
pub mod state;

pub use color::{HSLA, hsl_to_rgb, hsla_to_rgba_string, lerp_hsla_clamped, parse_color};
pub use morph_svg::MorphSvgEntry;
pub use path_measure::PathMeasureEntry;
pub use crate::script::recorder::AnimateEntry;
pub use state::{parse_easing_from_tag, random_from_seed};
