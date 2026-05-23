use anyhow::Result;

use crate::ir::{CompositionInfo, DrawOpFrame, FrameMediaPlan};
use crate::probe::AssetLoader;
use crate::script::js_context::JsContext;

pub mod default;
pub use default::DefaultPipeline;

pub trait Pipeline {
    type Loader: AssetLoader;
    type Scripts: JsContext;

    fn info(&self) -> &CompositionInfo;
    fn render_frame(&mut self, idx: u32) -> Result<(DrawOpFrame, FrameMediaPlan)>;
    fn loader(&self) -> &Self::Loader;
}
