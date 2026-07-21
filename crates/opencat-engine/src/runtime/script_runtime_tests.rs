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

#[test]
fn one_realm_shares_js_state_across_drivers() {
    // Same pipeline realm: global/scene/node drivers share JS state.
    use opencat_core::script::{ScriptHost, ScriptRealm};

    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let d1 = realm
        .install("globalThis.__sharedCounter = (globalThis.__sharedCounter || 0) + 1;")
        .expect("install d1");
    let d2 = realm
        .install(
            "if (typeof globalThis.__sharedCounter !== 'number') throw new Error('missing shared'); \
             globalThis.__sharedCounter += 10;",
        )
        .expect("install d2");

    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 64,
        height: 36,
        frames: 1,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut rec = MutationStore::default();
    realm
        .run_frame(d1, &script_frame_ctx, None, &mut rec)
        .expect("run d1");
    realm
        .run_frame(d2, &script_frame_ctx, None, &mut rec)
        .expect("run d2");

    // Read shared counter via a third driver that mutates opacity from it.
    let d3 = realm
        .install(
            "ctx.getNode('probe').opacity(globalThis.__sharedCounter / 100);",
        )
        .expect("install d3");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("probe".into());
    realm.set_target_registry(registry);
    let mut rec = MutationStore::default();
    realm
        .run_frame(d3, &script_frame_ctx, Some("probe"), &mut rec)
        .expect("run d3");
    let opacity = rec
        .snapshot_mutations()
        .mutations
        .get("probe")
        .and_then(|m| m.opacity)
        .expect("opacity written");
    // 1 + 10 = 11 → 0.11
    assert!((opacity - 0.11).abs() < 1e-6, "got {opacity}");
}

#[test]
fn separate_realms_do_not_share_js_globals() {
    use opencat_core::script::{ScriptHost, ScriptRealm};

    let mut a = ScriptRealm::<RqJsContext>::open().expect("realm a");
    let mut b = ScriptRealm::<RqJsContext>::open().expect("realm b");

    let da = a
        .install("globalThis.__pipelineSecret = 'from-a';")
        .expect("install a");
    let db = b
        .install(
            "if (globalThis.__pipelineSecret === 'from-a') throw new Error('leaked from a'); \
             globalThis.__pipelineSecret = 'from-b';",
        )
        .expect("install b");

    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 64,
        height: 36,
        frames: 1,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut rec = MutationStore::default();
    a.run_frame(da, &script_frame_ctx, None, &mut rec)
        .expect("run a");
    b.run_frame(db, &script_frame_ctx, None, &mut rec)
        .expect("run b — must not see a's global");
}

#[test]
fn realm_accepts_out_of_order_and_repeated_frames() {
    use opencat_core::script::{ScriptHost, ScriptRealm};

    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);
    let driver = realm
        .install("ctx.getNode('box').opacity(ctx.currentFrame / 10);")
        .expect("install");

    let run = |realm: &mut ScriptRealm<RqJsContext>, frame: u32| {
        let frame_ctx = FrameCtx {
            frame,
            fps: 30,
            width: 64,
            height: 36,
            frames: 30,
        };
        let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
        let mut rec = MutationStore::default();
        realm
            .run_frame(driver, &script_frame_ctx, Some("box"), &mut rec)
            .expect("run");
        rec.snapshot_mutations()
            .mutations
            .get("box")
            .and_then(|m| m.opacity)
            .expect("opacity")
    };

    let o5 = run(&mut realm, 5);
    let o2 = run(&mut realm, 2);
    let o5_again = run(&mut realm, 5);
    let o5_thrice = run(&mut realm, 5);

    assert!((o5 - 0.5).abs() < 1e-6);
    assert!((o2 - 0.2).abs() < 1e-6);
    assert!((o5_again - 0.5).abs() < 1e-6);
    assert!((o5_thrice - 0.5).abs() < 1e-6);
}

#[test]
fn script_runtime_cache_is_one_realm_not_per_driver() {
    // Historical ScriptRuntimeCache created one runner/context per source hash.
    // It must now install into a single realm so shared state works.
    use opencat_core::script::{ScriptHost, ScriptRuntimeCache};

    let mut cache = ScriptRuntimeCache::<RqJsContext>::default();
    let d1 = cache
        .install("globalThis.__cacheShared = 7;")
        .expect("d1");
    let d2 = cache
        .install(
            "if (globalThis.__cacheShared !== 7) throw new Error('not shared'); \
             globalThis.__cacheShared = 8;",
        )
        .expect("d2");

    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 64,
        height: 36,
        frames: 1,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut rec = MutationStore::default();
    cache
        .run_frame(d1, &script_frame_ctx, None, &mut rec)
        .expect("run d1");
    cache
        .run_frame(d2, &script_frame_ctx, None, &mut rec)
        .expect("run d2 shared");
}
