//! Host module — 默认 features 全开。
//! 承载 IO / ffmpeg / quickjs / skia / 系统字体 / RenderSession。

#![cfg(feature = "host-default")]

pub mod fonts;
pub mod inspect;
pub mod jsonl_io;
pub mod resource;
pub mod runtime;
pub mod script;
