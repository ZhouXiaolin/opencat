//! opencat-core — 纯数据 + trait + 算法，零 IO/平台依赖。

pub(crate) mod analyze;
pub mod cache;
pub mod canvas;
pub(crate) mod display;
pub mod fonts;
pub mod frame_ctx;
pub mod ir;
pub(crate) mod layout;
pub mod lifecycle;
pub mod lottie;
pub mod media;
pub mod parse;
pub mod pipeline;
pub mod probe;
pub mod profile;
pub(crate) mod render;
pub mod resolve;
pub mod script;
pub(crate) mod semantic;
pub mod style;
pub mod text;
pub mod time;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use self::frame_ctx::FrameCtx;
pub use self::ir::asset_id::{AssetId, ResourceKind};
pub use self::lifecycle::{
    CompositionDraft, HostInputs, HostRequirements, PrepareError, PreparedComposition,
    ResourceRequest,
};
pub use self::media::{
    AudioPlan, AudioSegment, VideoFrameRequest, VideoFrameTiming, collect_audio_plan,
};
pub use self::parse::node::Node;
pub use self::parse::preflight::{
    collect_resource_requests, collect_resource_requests_from_parsed,
};
pub use self::parse::{ParsedComposition, markup, parse};
pub use self::pipeline::{DefaultPipeline, FrameElementRect, Pipeline};
pub use self::probe::{
    AudioSource, ImageMeta, ImageSource, PreparedResourceCatalog, VideoInfoMeta, hydrate_captions,
};
pub use self::script::{
    PrecomputedScriptHost, ScriptDriver, ScriptDriverId, ScriptHost, ScriptRealm,
    asset_id_for_script_locator,
};
pub use self::text::{
    DefaultFontProvider, FontProvider, empty_font_db, extend_font_db, font_db_from_bytes,
};
pub use self::time::{
    DurationMicros, DurationRange, FrameCount, FrameIndex, RationalFrameRate, TimestampMicros,
    duration_secs_to_frames, frames_to_duration_secs, frames_to_timestamp_micros, secs_to_micros,
    timestamp_micros_to_frame, timestamp_micros_to_secs,
};
