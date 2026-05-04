//! Core module — 永不依赖 host features，可在 wasm32 编译。
//! 暴露 parse / collect_resource_requests / build_frame_display_tree
//! 三个公共入口，及 ResourceCatalog / ScriptHost / FontProvider trait。

// Phase 3 各 task 会逐步 pub mod 进来。
