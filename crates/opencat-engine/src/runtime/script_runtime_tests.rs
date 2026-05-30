//! Integration coverage for runtime JS plugins that need a real QuickJS host.

#![cfg(test)]

use opencat_core::frame_ctx::{FrameCtx, ScriptFrameCtx};
use opencat_core::script::recorder::{MutationRecorder, MutationStore};
use opencat_core::script::{ScriptRunner, ScriptTargetRegistry};

use crate::js_context::RqJsContext;

#[test]
fn filter_animation_plugin_writes_node_filter_values_through_value_api() {
    let source = r#"
        ctx.to("box", {
            duration: 10,
            brightness: 0.5,
            blur: 8
        });
        ctx.to("card", {
            duration: 10,
            filter: "contrast(2) brightness(0.25)"
        });
    "#;
    let mut runner =
        ScriptRunner::<RqJsContext>::new(source).expect("script runner should initialize");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".to_string());
    registry.visual_ids.insert("card".to_string());
    runner.set_target_registry(registry);

    let frame_ctx = FrameCtx {
        frame: 10,
        fps: 30,
        width: 320,
        height: 180,
        frames: 30,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut recorder = MutationStore::default();

    runner
        .run_into(&script_frame_ctx, None, &mut recorder)
        .expect("script should run");

    let snap = recorder.snapshot_mutations();
    let box_style = snap
        .mutations
        .get("box")
        .expect("individual filter animation should mutate the node");
    let card_style = snap
        .mutations
        .get("card")
        .expect("filter string animation should mutate the node");

    assert_eq!(box_style.css_filter.value("brightness"), Some(0.5));
    assert_eq!(box_style.css_filter.value("blur"), Some(8.0));
    assert_eq!(
        box_style.css_filter.to_css_string(),
        "brightness(0.5) blur(8px)"
    );
    assert_eq!(
        card_style.css_filter.to_css_string(),
        "contrast(2) brightness(0.25)"
    );
}
