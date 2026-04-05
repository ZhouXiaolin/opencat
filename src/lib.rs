pub mod assets;
pub mod backend;
pub mod composition;
pub mod display;
pub mod element;
pub mod frame_ctx;
pub mod layout;
pub mod media;
pub mod nodes;
pub mod parser;
pub mod render;
pub mod script;
pub mod style;
pub mod transitions;
pub mod typography;
pub mod view;

pub use composition::Composition;
pub use frame_ctx::FrameCtx;
pub use nodes::{div, image, text, video};
pub use parser::{ParsedComposition, parse};
pub use render::{EncodingConfig, Mp4Config, OutputFormat, RenderSession, render_frame_rgb};
pub use script::{NodeStyleMutations, ScriptDriver, StyleMutations};
pub use transitions::{
    TransitionKind, TransitionSeries, light_leak, linear, slide, transition_series,
};
pub use view::{Node, NodeKind, component_node, component_node_with_duration};
