//! 冒烟测试：opencat-web 可以构造并暴露 PathBoundsComputer。

use opencat_web::WebRenderEngine;

#[test]
fn web_render_engine_default_uses_default_path_bounds() {
    let engine = WebRenderEngine::default();
    let bounds = engine
        .path_bounds()
        .compute_view_box(&[String::from("M0 0 L10 10")])
        .expect("default path bounds always succeeds");
    assert_eq!(bounds, [0.0, 0.0, 100.0, 100.0]);
}
