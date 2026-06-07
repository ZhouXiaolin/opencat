//! Integration coverage for runtime JS plugins that need a real QuickJS host.

#![cfg(test)]

use opencat_core::frame_ctx::{FrameCtx, ScriptFrameCtx};
use opencat_core::script::recorder::{MutationRecorder, MutationStore};
use opencat_core::script::{
    ScriptRunner, ScriptTargetRegistry, ScriptTextSource, ScriptTextSourceKind,
};

use crate::js_context::RqJsContext;

#[test]
fn filter_animation_plugin_writes_node_filter_values_through_value_api() {
    let source = r#"
        ctx.to("box", {
            duration: 10 / 30,
            brightness: 0.5,
            blur: 8
        });
        ctx.to("card", {
            duration: 10 / 30,
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

#[test]
fn scramble_text_plugin_scrambles_then_resolves_to_target_text() {
    let source = r#"
        ctx.to("title", {
            duration: 1,
            scrambleText: {
                text: "READY",
                chars: "01",
                speed: 12
            },
            ease: "linear"
        });
    "#;
    let mut runner =
        ScriptRunner::<RqJsContext>::new(source).expect("script runner should initialize");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("title".to_string());
    runner.set_target_registry(registry);

    let mut sources = std::collections::HashMap::new();
    sources.insert(
        "title".to_string(),
        ScriptTextSource {
            text: "HELLO".to_string(),
            kind: ScriptTextSourceKind::TextNode,
        },
    );
    runner.set_text_sources(&sources);

    let text_at = |runner: &mut ScriptRunner<RqJsContext>, frame: u32| {
        let frame_ctx = FrameCtx {
            frame,
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
        recorder
            .snapshot_mutations()
            .mutations
            .get("title")
            .and_then(|m| m.text_content.clone())
            .expect("scrambleText should write text content")
    };

    let mid = text_at(&mut runner, 15);
    assert_eq!(mid.len(), "READY".len());
    assert_ne!(mid, "READY");
    assert!(
        mid.chars().all(|ch| "READY01".contains(ch)),
        "mid scramble should only contain revealed target chars or scramble chars, got {mid}"
    );

    assert_eq!(text_at(&mut runner, 30), "READY");
}

#[test]
fn scramble_text_plugin_supports_string_shorthand_and_registers_plugin() {
    let source = r#"
        if (ctx.animation.plugins().indexOf("scramble-text") < 0) {
            throw new Error("scramble-text plugin not registered");
        }
        ctx.to("title", {
            duration: 1,
            scrambleText: "DONE",
            chars: "X",
            ease: "linear"
        });
    "#;
    let mut runner =
        ScriptRunner::<RqJsContext>::new(source).expect("script runner should initialize");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("title".to_string());
    runner.set_target_registry(registry);

    let frame_ctx = FrameCtx {
        frame: 30,
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
    let text = snap
        .mutations
        .get("title")
        .and_then(|m| m.text_content.as_deref())
        .expect("scrambleText should write final text");
    assert_eq!(text, "DONE");
}
