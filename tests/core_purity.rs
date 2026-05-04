#![cfg(not(any(
    feature = "host-codec",
    feature = "host-script-quickjs",
    feature = "host-resource-net",
    feature = "host-backend-skia"
)))]

#[test]
fn core_public_api_compiles() {
    use opencat::core::{
        FontProvider, ResourceCatalog, ScriptHost,
        collect_resource_requests, parse,
    };
    let _ = parse;
    let _ = collect_resource_requests;
    fn _c<R: ResourceCatalog, F: FontProvider, S: ScriptHost>() {}
}
