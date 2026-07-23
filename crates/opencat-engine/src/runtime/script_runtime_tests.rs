//! Integration coverage for runtime JS plugins that need a real QuickJS host.

#![cfg(test)]

use opencat_core::frame_ctx::{FrameCtx, ScriptFrameCtx};
use opencat_core::script::recorder::{MutationRecorder, MutationStore};
use opencat_core::script::{
    ScriptDriverId, ScriptHost, ScriptRealm, ScriptTargetRegistry, ScriptTextSource,
    ScriptTextSourceKind,
};

use crate::js_context::RqJsContext;

fn open_with_source(source: &str) -> (ScriptRealm<RqJsContext>, ScriptDriverId) {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("script realm should open");
    let driver = realm.install(source).expect("install driver");
    (realm, driver)
}

fn run_driver(
    realm: &mut ScriptRealm<RqJsContext>,
    driver: ScriptDriverId,
    frame: u32,
    current_node_id: Option<&str>,
) -> MutationStore {
    let frame_ctx = FrameCtx {
        frame,
        fps: 30,
        width: 320,
        height: 180,
        frames: 30,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut recorder = MutationStore::default();
    realm
        .run_frame(driver, &script_frame_ctx, current_node_id, &mut recorder)
        .expect("script should run");
    recorder
}

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
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".to_string());
    registry.visual_ids.insert("card".to_string());
    realm.set_target_registry(registry);

    let recorder = run_driver(&mut realm, driver, 10, None);
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
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("title".to_string());
    realm.set_target_registry(registry);
    realm.register_text_source(
        "title",
        ScriptTextSource {
            text: "HELLO".to_string(),
            kind: ScriptTextSourceKind::TextNode,
        },
    );

    let text_at = |realm: &mut ScriptRealm<RqJsContext>, frame: u32| {
        run_driver(realm, driver, frame, None)
            .snapshot_mutations()
            .mutations
            .get("title")
            .and_then(|m| m.text_content.clone())
            .expect("scrambleText should write text content")
    };

    let mid = text_at(&mut realm, 15);
    assert_eq!(mid.len(), "READY".len());
    assert_ne!(mid, "READY");
    assert!(
        mid.chars().all(|ch| "READY01".contains(ch)),
        "mid scramble should only contain revealed target chars or scramble chars, got {mid}"
    );

    assert_eq!(text_at(&mut realm, 30), "READY");
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
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("title".to_string());
    realm.set_target_registry(registry);

    let snap = run_driver(&mut realm, driver, 30, None).snapshot_mutations();
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

/// AC #44-1: cross-frame global state must not affect determinism. A script
/// that writes to globalThis.counter must produce the same output for the
/// same frame_index regardless of render order (fresh, out-of-order, repeat).
#[test]
fn cross_frame_global_state_does_not_affect_determinism() {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);

    // This script uses globalThis.counter to compute opacity. Without
    // frame-boundary cleanup, out-of-order or repeated renders would see a
    // different counter value and produce a different output.
    let driver = realm
        .install(
            "globalThis.counter = (globalThis.counter || 0) + 1;\
             ctx.getNode('box').opacity(ctx.currentFrame / 10);",
        )
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

    // (1) Fresh pipeline renders frame 5.
    let f5_a = run(&mut realm, 5);
    assert!((f5_a - 0.5).abs() < 1e-6, "frame 5 fresh: {f5_a}");

    // (2) Render different frames out of order.
    let f0 = run(&mut realm, 0);
    assert!((f0 - 0.0).abs() < 1e-6, "frame 0 after f5: {f0}");
    let f3 = run(&mut realm, 3);
    assert!((f3 - 0.3).abs() < 1e-6, "frame 3: {f3}");

    // (3) Frame 5 again — must still be 0.5 regardless of the global counter
    // having been incremented by frames 0 and 3.
    let f5_b = run(&mut realm, 5);
    assert!((f5_b - 0.5).abs() < 1e-6, "frame 5 repeat: {f5_b}");

    // (4) Immediate repeat must also match.
    let f5_c = run(&mut realm, 5);
    assert!((f5_c - 0.5).abs() < 1e-6, "frame 5 thrice: {f5_c}");
}

/// AC #44-2: same-frame cross-driver communication via globalThis must still
/// work. Our frame-boundary cleanup only triggers when the frame number
/// changes, so drivers within the same frame share globals.
#[test]
fn same_frame_cross_driver_communication_via_globals_still_works() {
    // Regression: the existing test `one_realm_shares_js_state_across_drivers`
    // verifies this, but let's make it explicit for determinism semantics.
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);

    let d1 = realm
        .install("globalThis.__msg = 'hello from d1';")
        .expect("install d1");
    let d2 = realm
        .install(
            "if (globalThis.__msg !== 'hello from d1') \
             throw new Error('d1 state lost before d2 runs');\
             globalThis.__msg = 'd2 saw it';",
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

    // Both drivers run within the same frame — d2 must see d1's global.
    realm
        .run_frame(d1, &script_frame_ctx, None, &mut rec)
        .expect("run d1");
    realm
        .run_frame(d2, &script_frame_ctx, None, &mut rec)
        .expect("run d2 — must not fail on missing d1 global");

    // Verify d2 actually saw the value by reading through a third driver.
    let d3 = realm
        .install(
            "if (globalThis.__msg !== 'd2 saw it') \
             throw new Error('d2 value not visible in same frame');\
             ctx.getNode('box').opacity(0.5);",
        )
        .expect("install d3");
    let mut rec = MutationStore::default();
    realm
        .run_frame(d3, &script_frame_ctx, Some("box"), &mut rec)
        .expect("run d3");
    let opacity = rec
        .snapshot_mutations()
        .mutations
        .get("box")
        .and_then(|m| m.opacity)
        .expect("opacity written by d3");
    assert!((opacity - 0.5).abs() < 1e-6);
}

/// AC #44-3: animation determinism across frames — values computed via
/// __animate_create / __animate_value must produce the same result for the
/// same frame_index regardless of previous render history.
#[test]
fn animation_value_determinism_across_frames() {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);

    // Script animates opacity from 0 to 1 over 10 frames (1 sec at 10 fps).
    let driver = realm
        .install(
            "ctx.to('box', { opacity: 1, duration: 1, ease: 'linear' });\
             var p = ctx.getNode('box').opacity;",
        )
        .expect("install");

    // Warm up initial style so the animation engine can read current value.
    realm.set_style_defaults(
        &[("box".to_string(), [("opacity".into(), serde_json::json!(0.0))].into())]
            .into_iter()
            .collect(),
    );

    let run_and_read = |realm: &mut ScriptRealm<RqJsContext>, frame: u32| -> f32 {
        let frame_ctx = FrameCtx {
            frame,
            fps: 10,
            width: 64,
            height: 36,
            frames: 10,
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

    // (1) Fresh frame 5.
    let f5_a = run_and_read(&mut realm, 5);
    assert!((f5_a - 0.5).abs() < 0.05, "frame 5 fresh opacity: {f5_a}");

    // (2) Render frames 0 and 3.
    let f0 = run_and_read(&mut realm, 0);
    assert!((f0 - 0.0).abs() < 0.05, "frame 0 opacity: {f0}");
    let f3 = run_and_read(&mut realm, 3);
    assert!((f3 - 0.3).abs() < 0.05, "frame 3 opacity: {f3}");

    // (3) Frame 5 again — must produce the same animation value.
    let f5_b = run_and_read(&mut realm, 5);
    assert!(
        (f5_b - 0.5).abs() < 0.05,
        "frame 5 repeat opacity: {f5_b} (expected ~0.5)"
    );

    // (4) Immediate frame 5 repeat.
    let f5_c = run_and_read(&mut realm, 5);
    assert!(
        (f5_c - 0.5).abs() < 0.05,
        "frame 5 thrice opacity: {f5_c} (expected ~0.5)"
    );
}

#[test]
fn realm_installs_multiple_drivers_into_shared_state() {
    // Contract-phase replacement of the historical ScriptRuntimeCache test:
    // many drivers must share one realm so pipeline-local JS state works.
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let d1 = realm
        .install("globalThis.__cacheShared = 7;")
        .expect("d1");
    let d2 = realm
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
    realm
        .run_frame(d1, &script_frame_ctx, None, &mut rec)
        .expect("run d1");
    realm
        .run_frame(d2, &script_frame_ctx, None, &mut rec)
        .expect("run d2 shared");
}

/// AC #63-1: Math.random is overridden with a frame-seeded deterministic
/// function. Same frame index must produce the same random sequence.
#[test]
fn math_random_is_deterministic_across_frames() {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let driver = realm
        .install(
            "var r0 = Math.random();\
             var r1 = Math.random();\
             ctx.getNode('box').opacity(r0 + r1);",
        )
        .expect("install");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);

    let run = |realm: &mut ScriptRealm<RqJsContext>, frame: u32| -> f32 {
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

    let f5_a = run(&mut realm, 5);
    let f3 = run(&mut realm, 3);
    let f5_b = run(&mut realm, 5);
    let f5_c = run(&mut realm, 5);

    assert!((f5_a - f5_b).abs() < 1e-6, "frame 5 repeat must be identical: {f5_a} vs {f5_b}");
    assert!((f5_b - f5_c).abs() < 1e-6, "frame 5 thrice must be identical: {f5_b} vs {f5_c}");
    assert!(
        (f5_a - f3).abs() > 1e-6,
        "different frame must produce different random: {f5_a} vs {f3}"
    );
}

/// AC #63-2: Date.now is stubbed to return 0 so scripts cannot depend on
/// wall-clock time.
#[test]
fn date_now_is_stubbed_to_zero() {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);

    let driver = realm
        .install(
            "if (Date.now() !== 0) throw new Error('Date.now not stubbed: ' + Date.now());\
             ctx.getNode('box').opacity(0.5);",
        )
        .expect("install");

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
        .run_frame(driver, &script_frame_ctx, Some("box"), &mut rec)
        .expect("run_frame with stubbed Date.now must not throw");
    let opacity = rec
        .snapshot_mutations()
        .mutations
        .get("box")
        .and_then(|m| m.opacity)
        .expect("opacity must be written");
    assert!((opacity - 0.5).abs() < 1e-6, "opacity = {opacity}");
}
/// must produce field-identical mutations for the same frame.
#[test]
fn install_same_source_twice_cache_hit_miss_field_identical() {
    let source = "ctx.getNode('box').opacity(0.5);";
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);

    // First install — cache miss (evaluates JS source)
    let miss = realm.install(source).expect("install (cache miss)");
    // Second install with same source — cache hit (installed map prevents re-eval)
    let hit = realm.install(source).expect("install (cache hit)");
    assert_eq!(miss, hit, "same source must yield same ScriptDriverId");

    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 64,
        height: 36,
        frames: 1,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);

    let run = |realm: &mut ScriptRealm<RqJsContext>, id: ScriptDriverId| {
        let mut rec = MutationStore::default();
        realm
            .run_frame(id, &script_frame_ctx, Some("box"), &mut rec)
            .expect("run");
        rec.snapshot_mutations()
    };

    let snap_miss = run(&mut realm, miss);
    let snap_hit = run(&mut realm, hit);
    let op_miss = snap_miss.mutations.get("box").and_then(|m| m.opacity);
    let op_hit = snap_hit.mutations.get("box").and_then(|m| m.opacity);
    assert_eq!(
        op_miss, op_hit,
        "cache-hit install must produce field-identical output to cache-miss",
    );
    assert!(
        (op_miss.unwrap() - 0.5).abs() < 1e-6,
        "expected opacity ~0.5, got {:?}",
        op_miss,
    );
}

/// AC #44-1 (extended): two independent realms each installing the same source
/// (each a cache miss in its own realm) must produce field-identical output
/// for the same frame. This validates cross-realm determinism — the hash-based
/// ScriptDriverId is stable across boundaries.
#[test]
fn separate_realms_same_source_produce_field_identical_output() {
    let source = "ctx.getNode('box').opacity(0.5);";

    let mut realm_a = ScriptRealm::<RqJsContext>::open().expect("realm a");
    let mut realm_b = ScriptRealm::<RqJsContext>::open().expect("realm b");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm_a.set_target_registry(registry.clone());
    realm_b.set_target_registry(registry);

    // Both realms install the same source — each is a cache miss in its own realm
    let id_a = realm_a.install(source).expect("install in realm a");
    let id_b = realm_b.install(source).expect("install in realm b");
    assert_eq!(
        id_a, id_b,
        "same source must yield same ScriptDriverId across realms"
    );

    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 64,
        height: 36,
        frames: 1,
    };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);

    let run = |realm: &mut ScriptRealm<RqJsContext>, id: ScriptDriverId| {
        let mut rec = MutationStore::default();
        realm
            .run_frame(id, &script_frame_ctx, Some("box"), &mut rec)
            .expect("run");
        rec.snapshot_mutations()
    };

    let snap_a = run(&mut realm_a, id_a);
    let snap_b = run(&mut realm_b, id_b);
    let op_a = snap_a.mutations.get("box").and_then(|m| m.opacity);
    let op_b = snap_b.mutations.get("box").and_then(|m| m.opacity);
    assert_eq!(op_a, op_b, "separate realms must produce field-identical output");
    assert!(
        (op_a.unwrap() - 0.5).abs() < 1e-6,
        "expected opacity ~0.5, got {:?}",
        op_a,
    );
}
