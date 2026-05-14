//! AssetResolver trait — 统一资源解析接口。
//!
//! 输入：[`ImageSource`] / [`AudioSource`] / [`VideoSource`] / Path / Openverse query。
//! 输出：`*Meta`（含 [`AssetId`] + 宽高/时长等元数据）。
//!
//! 职责：把每种 source 变体「解析」成可被后续 layout / render 使用的元数据。
//! 实际的字节/路径存储由各 platform 在 impl 内部处理（engine 写盘到 `AssetPathStore`，
//! wasm 灌内存 `BlobStore`）—— core 不感知。
//!
//! 当前为 v1：每个方法独立 async，串行调度由 [`crate::resource::preload`] 负责。
//! 并发优化（batch / buffer_unordered）留到后续迭代。

use std::future::Future;
use std::path::Path;

use anyhow::Result;

use crate::resource::asset_id::AssetId;
use crate::scene::primitives::OpenverseQuery;

#[derive(Clone, Debug, PartialEq)]
pub struct ImageMeta {
    pub id: AssetId,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoMeta {
    pub id: AssetId,
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioMeta {
    pub id: AssetId,
}

/// 统一资源解析 + 元数据读取接口。
///
/// 没有 `Send` bound：原生 tokio multi-thread 实现自然返回 Send future，
/// wasm 单线程实现返回 !Send future，两端都能编译。
pub trait AssetResolver {
    fn resolve_image_url(&mut self, url: &str) -> impl Future<Output = Result<ImageMeta>>;
    fn resolve_audio_url(&mut self, url: &str) -> impl Future<Output = Result<AudioMeta>>;
    fn resolve_video_url(&mut self, url: &str) -> impl Future<Output = Result<VideoMeta>>;

    /// 本地文件系统路径。engine 实现；wasm 默认 bail（web 无文件系统语义）。
    fn resolve_image_path(&mut self, _path: &Path) -> impl Future<Output = Result<ImageMeta>> {
        async { anyhow::bail!("resolve_image_path not supported on this platform") }
    }
    fn resolve_video_path(&mut self, _path: &Path) -> impl Future<Output = Result<VideoMeta>> {
        async { anyhow::bail!("resolve_video_path not supported on this platform") }
    }
    fn resolve_audio_path(&mut self, _path: &Path) -> impl Future<Output = Result<AudioMeta>> {
        async { anyhow::bail!("resolve_audio_path not supported on this platform") }
    }

    /// Openverse 图片搜索。engine 实现；wasm v1 不支持。
    fn resolve_image_query(
        &mut self,
        _query: &OpenverseQuery,
    ) -> impl Future<Output = Result<ImageMeta>> {
        async { anyhow::bail!("resolve_image_query not supported on this platform") }
    }
}
