//! Open compositions with engine-embedded default fonts.

use anyhow::Result;

use crate::js_context::RqJsContext;
use crate::resource::loader::EngineLoader;
use crate::fonts::engine_default_font_db;
use crate::EnginePipeline;

/// Parse and open a composition with Noto Sans SC + Noto Color Emoji loaded by default.
pub fn open(input: &str, loader: EngineLoader, scripts: RqJsContext) -> Result<EnginePipeline> {
    EnginePipeline::open_with_font_db(input, loader, scripts, engine_default_font_db())
}