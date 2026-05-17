//! Web 侧资源解析：通过浏览器 `fetch()` 拉字节，imagesize/nom-exif 读元数据
//! （探测函数下沉到 core），BlobStore 暂存字节供 JS 后续消费
//! （CanvasKit 解码、`URL.createObjectURL` 等）。

pub mod blob_store;
pub mod fetch;
pub mod resolver;

#[cfg(target_arch = "wasm32")]
pub mod wasm_api;

pub use blob_store::BlobStore;
pub use resolver::WebAssetResolver;
