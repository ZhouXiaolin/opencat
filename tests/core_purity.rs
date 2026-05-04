//! Phase 4 才真正通过；Phase 1-3 期间作为 progress beacon。
//! 这个测试在 --no-default-features 下编译 core 公共 API 路径，
//! 用 cargo check --no-default-features --lib --tests 触发。
//! 目前预期失败，因为 src/* 大量直接 use 了 host 依赖。

#![cfg(not(any(
    feature = "host-codec",
    feature = "host-script-quickjs",
    feature = "host-resource-net",
    feature = "host-backend-skia"
)))]

#[test]
fn core_public_api_compiles() {
    use opencat::core::{
        FontProvider, ResourceCatalog, ScriptHost, build_frame_display_tree,
        collect_resource_requests, parse,
    };
    let _: fn(&str) -> _ = parse;
    let _: fn(&_) -> _ = collect_resource_requests;
    let _ = build_frame_display_tree;
    fn _check_traits<R: ResourceCatalog, F: FontProvider, S: ScriptHost>() {}
}
