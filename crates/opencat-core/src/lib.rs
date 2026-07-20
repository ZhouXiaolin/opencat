//! opencat-core — 纯数据 + trait + 算法，零 IO/平台依赖。

pub mod analyze;
pub mod cache;
pub mod canvas;
pub mod display;
pub mod frame_ctx;
pub mod ir;
pub mod layout;
pub mod media;
pub mod parse;
pub mod pipeline;
pub mod platform;
pub mod probe;
pub mod profile;
pub mod render;
pub mod resolve;
pub mod resource;
pub mod runtime;
pub mod script;
pub mod semantic;
pub mod style;
pub mod text;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use self::frame_ctx::FrameCtx;
pub use self::ir::asset_id::AssetId;
pub use self::media::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
pub use self::parse::node::Node;
pub use self::parse::preflight::{
    collect_external_manifest, collect_resource_requests, collect_resource_requests_from_parsed,
};
pub use self::parse::{ParsedComposition, markup, parse};
pub use self::pipeline::{DefaultPipeline, Pipeline};
pub use self::platform::video::{FrameBitmap, VideoFrameProvider};
pub use self::probe::{
    AssetHandle, AssetLoader, AudioPlan, AudioSegment, AudioSource, ByteSource, ImageMeta,
    ImageSource, NoopAssetLoader, PreparedCatalog, ProbeOutcome,
    ResourceCatalog as ProbeResourceCatalog, SubtitleSource, VideoInfoMeta, VideoSource,
    build_catalog, hydrate_captions,
};
pub use self::resource::catalog::ResourceCatalog;
pub use self::resource::hash_map_catalog::{HashMapResourceCatalog, ResourceKind, ResourceMeta};
pub use self::runtime::session::RenderSession;
pub use self::script::{
    PrecomputedScriptHost, ScriptDriver, ScriptDriverId, ScriptHost, ScriptRunner,
    ScriptRuntimeCache,
};
#[cfg(any(test, feature = "test-support"))]
pub use self::test_support::TestCatalog;
pub use self::text::{
    DefaultFontProvider, FontProvider, empty_font_db, extend_font_db, font_db_from_bytes,
};
