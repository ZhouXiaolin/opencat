#[cfg(feature = "host-default")]
pub mod backend;
#[cfg(feature = "host-default")]
pub mod codec;
#[cfg(feature = "host-default")]
pub mod render;
#[cfg(feature = "host-default")]
pub mod host;
#[cfg(feature = "host-default")]
pub mod runtime { pub use crate::host::runtime::*; }

// 兼容外部既有用法 `opencat::core::*`：
pub use opencat_core as core;

// Backward-compatible module re-exports
pub use opencat_core::display;
pub use opencat_core::element;
pub use opencat_core::frame_ctx;
pub use opencat_core::jsonl;
pub use opencat_core::layout;
pub use opencat_core::scene;
pub use opencat_core::style;
pub use opencat_core::text;

pub use opencat_core::frame_ctx::FrameCtx;
#[cfg(feature = "host-default")]
pub use crate::host::inspect::{FrameElementRect, collect_frame_layout_rects};
pub use opencat_core::jsonl::{ParsedComposition, parse};
#[cfg(feature = "host-default")]
pub use crate::host::jsonl_io::{parse_file, parse_with_base_dir};
#[cfg(feature = "host-default")]
pub use crate::render::{
    EncodingConfig, Mp4Config, OutputFormat, RenderBackend, RenderSession, build_audio_track,
    render_audio_chunk, render_frame_rgb, render_frame_rgba, render_frame_to_target,
};
#[cfg(feature = "host-default")]
pub use crate::host::resource::media::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
#[cfg(feature = "host-default")]
pub use crate::runtime::audio::AudioBuffer;
#[cfg(feature = "host-default")]
pub use crate::runtime::target::{RenderFrameViewKind, RenderTargetHandle};
pub use opencat_core::scene::composition::{AudioAttachment, Composition, CompositionAudioSource};
pub use opencat_core::scene::easing::{Easing, SpringConfig, animate_value, easing_from_name};
pub use opencat_core::scene::node::{Node, NodeKind, component_node, component_node_with_duration};
pub use opencat_core::scene::primitives::{
    AudioSource, Canvas, CanvasAsset, CaptionNode, Image, ImageSource, OpenverseQuery, Path,
    SrtEntry, canvas, caption, div, image, lucide, parse_srt, path, text, video,
};
pub use opencat_core::scene::script::{NodeStyleMutations, ScriptDriver, StyleMutations};
pub use opencat_core::scene::transition::{
    ClockWipeBuilder, FadeBuilder, GlTransition, GlTransitionBuilder, IrisBuilder, SlideBuilder,
    SlideDirection, Timeline, TransitionKind, WipeBuilder, WipeDirection, clock_wipe, fade,
    gl_transition, iris, light_leak, slide, timeline, wipe,
};
