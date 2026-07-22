//! App/CLI facade for desktop targets.
//!
//! Narrow surface for `opencat` / `opencat-see` binaries. Prefer importing
//! `opencat_core` / `opencat_engine` modules directly for anything else — no
//! long-term deprecation re-exports of internal modules.

pub use opencat_core::frame_ctx::duration_secs_to_frames;
pub use opencat_engine::consumer::execute_render_frame;
pub use opencat_engine::executor::EngineDrawExecutor;
pub use opencat_engine::js_context::RqJsContext;
pub use opencat_engine::media::{AudioTrack, MediaContext};
pub use opencat_engine::pipeline::open;
pub use opencat_engine::render::build_audio_track_from_pipeline;
pub use opencat_engine::resource::loader::EngineLoader;
pub use opencat_engine::EnginePipeline;
