#[cfg(feature = "host-default")]
pub mod backend;
#[cfg(feature = "host-default")]
pub mod codec;
#[cfg(feature = "host-default")]
pub mod render;
pub mod core;
#[cfg(feature = "host-default")]
pub mod host;
#[cfg(feature = "host-default")]
pub mod runtime { pub use crate::host::runtime::*; }

// Backward-compatible module re-exports
pub use crate::core::display;
pub use crate::core::element;
pub use crate::core::frame_ctx;
pub use crate::core::jsonl;
pub use crate::core::layout;
pub use crate::core::scene;
pub use crate::core::style;
pub use crate::core::text;

pub use crate::core::frame_ctx::FrameCtx;
#[cfg(feature = "host-default")]
pub use crate::host::inspect::{FrameElementRect, collect_frame_layout_rects};
pub use crate::core::jsonl::{ParsedComposition, parse};
#[cfg(feature = "host-default")]
pub use crate::host::jsonl_io::{parse_file, parse_with_base_dir};
#[cfg(feature = "host-default")]
pub use render::{
    EncodingConfig, Mp4Config, OutputFormat, RenderBackend, RenderSession, build_audio_track,
    render_audio_chunk, render_frame_rgb, render_frame_rgba, render_frame_to_target,
};
#[cfg(feature = "host-default")]
pub use crate::host::resource::media::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
#[cfg(feature = "host-default")]
pub use runtime::audio::AudioBuffer;
#[cfg(feature = "host-default")]
pub use runtime::target::{RenderFrameViewKind, RenderTargetHandle};
pub use crate::core::scene::composition::{AudioAttachment, Composition, CompositionAudioSource};
pub use crate::core::scene::easing::{Easing, SpringConfig, animate_value, easing_from_name};
pub use crate::core::scene::node::{Node, NodeKind, component_node, component_node_with_duration};
pub use crate::core::scene::primitives::{
    AudioSource, Canvas, CanvasAsset, CaptionNode, Image, ImageSource, OpenverseQuery, Path,
    SrtEntry, canvas, caption, div, image, lucide, parse_srt, path, text, video,
};
pub use crate::core::scene::script::{NodeStyleMutations, ScriptDriver, StyleMutations};
pub use crate::core::scene::transition::{
    ClockWipeBuilder, FadeBuilder, GlTransition, GlTransitionBuilder, IrisBuilder, SlideBuilder,
    SlideDirection, Timeline, TransitionKind, WipeBuilder, WipeDirection, clock_wipe, fade,
    gl_transition, iris, light_leak, slide, timeline, wipe,
};
