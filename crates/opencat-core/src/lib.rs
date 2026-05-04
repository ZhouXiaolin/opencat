//! opencat-core — 纯数据 + trait + 算法，零 IO/平台依赖。

pub mod display;
pub mod element;
pub mod frame_ctx;
pub mod jsonl;
pub mod layout;
mod lucide_icons;
pub mod resource;
pub mod runtime;
pub mod scene;
pub mod style;
pub mod text;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use self::frame_ctx::FrameCtx;
pub use self::jsonl::{ParsedComposition, parse};
pub use self::resource::catalog::{ResourceCatalog, VideoInfoMeta};
pub use self::resource::asset_catalog::{AssetCatalog, AssetId};
pub use self::runtime::preflight_collect::{ResourceRequests, collect_resource_requests};
pub use self::scene::script::{ScriptHost, ScriptDriverId};
pub use self::text::{FontProvider, DefaultFontProvider};
