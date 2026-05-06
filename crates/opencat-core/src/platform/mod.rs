//! Platform abstractions: traits and generic data structures shared across
//! backends (engine/skia, web/wasm, future native backends).
//!
//! Phase A introduces:
//! - [`backend::BackendTypes`] — associated cache value types per backend
//!
//! Phase A 后续 task 还会加：
//! - `render_engine::RenderEngine`（提升自 engine 的 backend trait）
//! - `render_engine::{RecordCtx, RenderCtx, FrameView, FrameViewKind}`
//! - `scene_snapshot::SceneSnapshotCache<B>`
//! - `cache::CacheRegistry<B>`（在 `runtime::cache` 下，不是这里）
//!
//! `Platform` 门面 trait（聚合 ScriptHost + VideoFrameProvider 等）属于 Phase C。

pub mod backend;
pub mod platform;
pub mod render_engine;
pub mod scene_snapshot;
pub mod video;
