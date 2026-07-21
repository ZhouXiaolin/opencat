//! Web-owned media implementations.
//!
//! Core defines media contracts. This module owns WebAudio and browser-side
//! video frame injection storage.

pub mod audio;

pub use audio::{DecodedAudio, WebAudio};
