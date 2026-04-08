pub mod assets;
mod backend;
mod bitmap_source;
pub mod codec;
pub mod composition;
pub mod display;
pub mod element;
pub mod frame_ctx;
pub mod layout;
mod lucide_icons;
mod media;
pub mod nodes;
pub mod parser;
mod profile;
pub mod render;
mod render_cache;
pub mod render_target;
mod scene_snapshot;
pub mod script;
pub mod style;
mod timeline;
pub mod transitions;
pub mod typography;
pub mod view;

pub use composition::Composition;
pub use frame_ctx::FrameCtx;
pub use nodes::{
    Audio, AudioSource, Canvas, CanvasAsset, Image, ImageSource, OpenverseQuery, audio, canvas,
    div, image, lucide, text, video,
};
pub use parser::{ParsedComposition, parse};
pub use render::{
    EncodingConfig, Mp4Config, OutputFormat, RenderBackend, RenderSession, render_frame_rgb,
    render_frame_rgba, render_frame_to_target,
};
pub use render_target::{RenderBackendKind, RenderTargetHandle};
pub use script::{NodeStyleMutations, ScriptDriver, StyleMutations};
pub use transitions::{
    ClockWipeBuilder, FadeBuilder, IrisBuilder, SlideBuilder, SlideDirection, SpringConfig,
    Timeline, TransitionKind, WipeBuilder, WipeDirection, clock_wipe, fade, iris, light_leak,
    linear, slide, spring, timeline, wipe,
};
pub use view::{Node, NodeKind, component_node, component_node_with_duration};
