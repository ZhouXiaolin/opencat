pub mod assets;
mod backend;
mod cache_policy;
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
pub mod script;
pub mod style;
mod timeline;
pub mod transitions;
pub mod typography;
pub mod view;

pub use composition::Composition;
pub use frame_ctx::FrameCtx;
pub use nodes::{Image, ImageSource, OpenverseQuery, div, image, lucide, text, video};
pub use parser::{ParsedComposition, parse};
pub use render::{
    EncodingConfig, Mp4Config, OutputFormat, RenderSession, render_frame_rgb, render_frame_rgba,
};
pub use script::{NodeStyleMutations, ScriptDriver, StyleMutations};
pub use transitions::{
    ClockWipeBuilder, FadeBuilder, IrisBuilder, SlideBuilder, SlideDirection, SpringConfig,
    TransitionKind, TransitionSeries, WipeBuilder, WipeDirection, clock_wipe, fade, iris,
    light_leak, linear, slide, spring, transition_series, wipe,
};
pub use view::{Node, NodeKind, component_node, component_node_with_duration};
