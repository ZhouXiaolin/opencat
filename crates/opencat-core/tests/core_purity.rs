//! Workspace-level smoke test: opencat-core compiles standalone with zero host features.

#[test]
fn core_public_api_compiles() {
    use opencat_core::{
        FontProvider, ResourceCatalog, ScriptHost,
        collect_resource_requests, parse,
    };
    let _ = parse;
    let _ = collect_resource_requests;
    fn _c<R: ResourceCatalog, F: FontProvider, S: ScriptHost>() {}
}
