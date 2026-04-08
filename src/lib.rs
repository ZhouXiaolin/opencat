mod backend;
pub mod codec;
pub mod display;
pub mod element;
pub mod frame_ctx;
pub mod jsonl;
pub mod layout;
mod lucide_icons;
pub mod render;
pub mod resource;
pub mod runtime;
pub mod scene;
pub mod style;

pub use frame_ctx::FrameCtx;
pub use jsonl::{ParsedComposition, parse};
pub use render::{
    EncodingConfig, Mp4Config, OutputFormat, RenderBackend, RenderSession, render_frame_rgb,
    render_frame_rgba, render_frame_to_target,
};
pub use runtime::target::{RenderFrameViewKind, RenderTargetHandle};
pub use scene::composition::Composition;
pub use scene::node::{Node, NodeKind, component_node, component_node_with_duration};
pub use scene::primitives::{
    Audio, AudioSource, Canvas, CanvasAsset, Image, ImageSource, OpenverseQuery, audio, canvas,
    div, image, lucide, text, video,
};
pub use scene::script::{NodeStyleMutations, ScriptDriver, StyleMutations};
pub use scene::transition::{
    ClockWipeBuilder, FadeBuilder, IrisBuilder, SlideBuilder, SlideDirection, SpringConfig,
    Timeline, TransitionKind, WipeBuilder, WipeDirection, clock_wipe, fade, iris, light_leak,
    linear, slide, spring, timeline, wipe,
};
