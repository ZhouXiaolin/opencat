//! opencat-core — 纯数据 + trait + 算法，零 IO/平台依赖。

pub mod analyze;
pub mod cache;
pub mod canvas;
pub mod display;
pub mod frame_ctx;
pub mod ir;
pub mod layout;
pub mod lifecycle;
pub mod media;
pub mod parse;
pub mod pipeline;
pub mod probe;
pub mod profile;
pub mod render;
pub mod resolve;
pub mod resource;
pub mod script;
pub mod semantic;
pub mod style;
pub mod text;
pub mod time;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use self::frame_ctx::FrameCtx;
pub use self::ir::asset_id::AssetId;
pub use self::lifecycle::{
    CompositionDraft, HostInputs, HostRequirements, PrepareError, PreparedComposition,
    ResourceKind as LifecycleResourceKind, ResourceLocator, ResourceRequest,
};
pub use self::media::{
    collect_audio_plan, AudioPlan, AudioSegment, VideoFrameRequest, VideoFrameTiming,
};
pub use self::time::{
    DurationMicros, DurationRange, FrameCount, FrameIndex, RationalFrameRate, TimestampMicros,
    duration_secs_to_frames, frames_to_duration_secs, frames_to_timestamp_micros, secs_to_micros,
    timestamp_micros_to_frame, timestamp_micros_to_secs,
};
pub use self::parse::node::Node;
pub use self::parse::preflight::{
    collect_resource_requests, collect_resource_requests_from_parsed,
};
pub use self::parse::{ParsedComposition, markup, parse};
pub use self::pipeline::{DefaultPipeline, Pipeline};
pub use self::probe::{
    AudioSource, ByteSource, ImageMeta, ImageSource, PreparedCatalog, PreparedResourceCatalog,
    ProbeOutcome, SubtitleSource, VideoInfoMeta, VideoSource, build_catalog, hydrate_captions,
};
pub use self::resource::catalog::ResourceResolver;
pub use self::resource::hash_map_catalog::{HashMapResourceCatalog, ResourceKind, ResourceMeta};
pub use self::script::{
    PrecomputedScriptHost, ScriptDriver, ScriptDriverId, ScriptHost, ScriptRealm, ScriptRunner,
    ScriptRuntimeCache, asset_id_for_script_locator,
};
#[cfg(any(test, feature = "test-support"))]
pub use self::test_support::TestCatalog;
pub use self::text::{
    DefaultFontProvider, FontProvider, empty_font_db, extend_font_db, font_db_from_bytes,
};
