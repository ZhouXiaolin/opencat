// 兼容外部既有用法 `opencat::core::*`：
pub use opencat_core as core;

// 兼容外部既有用法 `opencat::host::*`：
pub use opencat_engine as host;

// Backward-compatible module re-exports from core
pub use opencat_core::display;
pub use opencat_core::element;
pub use opencat_core::frame_ctx;
pub use opencat_core::jsonl;
pub use opencat_core::layout;
pub use opencat_core::scene;
pub use opencat_core::style;
pub use opencat_core::text;

// Backward-compatible module re-exports from engine
pub use opencat_engine::backend;
pub use opencat_engine::codec;
pub use opencat_engine::render;
pub use opencat_engine::runtime;
pub use opencat_engine::fonts;
pub use opencat_engine::inspect;
pub use opencat_engine::jsonl_io;
pub use opencat_engine::resource;
pub use opencat_engine::script;

// Top-level re-exports
pub use opencat_core::frame_ctx::FrameCtx;
pub use opencat_engine::inspect::{FrameElementRect, collect_frame_layout_rects};
pub use opencat_core::jsonl::{ParsedComposition, parse};
pub use opencat_engine::jsonl_io::{parse_file, parse_with_base_dir};
pub use opencat_engine::render::{
    EncodingConfig, Mp4Config, OutputFormat, RenderBackend, RenderSession, build_audio_track,
    render_audio_chunk, render_frame_rgb, render_frame_rgba, render_frame_to_target,
    render_frame_with_target, render,
};
pub use opencat_engine::resource::media::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
pub use opencat_engine::runtime::audio::AudioBuffer;
pub use opencat_engine::runtime::target::{RenderFrameViewKind, RenderTargetHandle};
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
