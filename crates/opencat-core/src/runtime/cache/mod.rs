//! Backend-agnostic caches used by the render pipeline.
//!
//! Phase A 阶段只承载 `video_frames` 子模块；Phase A 后续 task 会引入
//! `CacheRegistry<B>` 等需要 `BackendTypes` 关联类型的 cache 容器。

pub mod video_frames;
