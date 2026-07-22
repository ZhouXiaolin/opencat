use anyhow::Result;

use crate::ir::{CompositionInfo, RenderFrame};
use crate::script::js_context::JsContext;

pub mod default;
pub mod frame;
pub mod inspect;
pub use default::DefaultPipeline;
pub use inspect::FrameElementRect;

/// Core rendering pipeline contract.
///
/// The pipeline is a pure derivation kernel: it consumes host-prepared
/// resource metadata and emits a deterministic [`RenderFrame`] per frame. It
/// owns no loader, fetcher, or decoder — hosts acquire resources themselves
/// and open the pipeline via the explicit lifecycle
/// (`CompositionDraft` → `prepare` → `PreparedComposition::open_pipeline`) or
/// the retained host-injected entry
/// [`DefaultPipeline::open_with_prepared_catalog`].
pub trait Pipeline {
    type Scripts: JsContext;

    fn info(&self) -> &CompositionInfo;
    fn render_frame(&mut self, idx: u32) -> Result<RenderFrame>;
}
