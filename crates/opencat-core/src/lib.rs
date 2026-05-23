//! opencat-core — 纯数据 + trait + 算法，零 IO/平台依赖。

pub mod cache;
pub mod canvas;
pub mod display;
pub mod element;
pub mod frame_ctx;
pub mod ir;
pub mod layout;
mod lucide_icons;
pub mod parse;
pub mod platform;
pub mod render;
pub mod resource;
pub mod runtime;
pub mod scene;
pub mod script;
pub mod style;
pub mod text;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use self::frame_ctx::FrameCtx;
pub use self::parse::node::Node;
pub use self::parse::preflight::{ResourceRequests, collect_resource_requests};
pub use self::parse::{ParsedComposition, parse};
pub use self::platform::video::{FrameBitmap, VideoFrameProvider};
pub use self::resource::asset_id::AssetId;
pub use self::resource::catalog::{ResourceCatalog, VideoInfoMeta};
pub use self::resource::hash_map_catalog::{HashMapResourceCatalog, ResourceKind, ResourceMeta};
pub use self::runtime::session::RenderSession;
pub use self::scene::script::{
    PrecomputedScriptHost, ScriptDriver, ScriptDriverId, ScriptHost, ScriptRunner,
    ScriptRuntimeCache,
};
#[cfg(any(test, feature = "test-support"))]
pub use self::test_support::TestCatalog;
pub use self::text::{DefaultFontProvider, FontProvider};
