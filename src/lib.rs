pub mod backend;
pub mod codec;
pub mod display;
pub mod element;
pub mod frame_ctx;
pub mod inspect;
pub mod jsonl;
pub mod layout;
mod lucide_icons;
pub mod render;
pub mod resource;
pub mod runtime;
pub mod scene;
pub mod style;
pub mod text;

pub use frame_ctx::FrameCtx;
pub use inspect::{FrameElementRect, collect_frame_layout_rects};
pub use jsonl::{ParsedComposition, parse, parse_file, parse_with_base_dir};
pub use render::{
    EncodingConfig, Mp4Config, OutputFormat, RenderBackend, RenderSession, build_audio_track,
    render_audio_chunk, render_frame_rgb, render_frame_rgba, render_frame_to_target,
};
pub use resource::media::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
pub use runtime::audio::AudioBuffer;
pub use runtime::target::{RenderFrameViewKind, RenderTargetHandle};
pub use scene::composition::{AudioAttachment, Composition, CompositionAudioSource};
pub use scene::easing::{Easing, SpringConfig, animate_value, easing_from_name};
pub use scene::node::{Node, NodeKind, component_node, component_node_with_duration};
pub use scene::primitives::{
    AudioSource, Canvas, CanvasAsset, CaptionNode, Image, ImageSource, OpenverseQuery, Path,
    SrtEntry, canvas, caption, div, image, lucide, parse_srt, path, text, video,
};
pub use scene::script::{NodeStyleMutations, ScriptDriver, StyleMutations};
pub use scene::transition::{
    ClockWipeBuilder, FadeBuilder, GlTransition, GlTransitionBuilder, IrisBuilder, SlideBuilder,
    SlideDirection, Timeline, TransitionKind, WipeBuilder, WipeDirection, clock_wipe, fade,
    gl_transition, iris, light_leak, slide, timeline, wipe,
};
