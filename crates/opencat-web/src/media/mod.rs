//! Web-owned media implementations.
//!
//! Core defines media contracts. This module owns WebAudio and browser-side
//! video frame injection storage.

pub mod audio;
pub mod video;

pub use audio::{DecodedAudio, WebAudio};
pub use video::WebVideoSource;
