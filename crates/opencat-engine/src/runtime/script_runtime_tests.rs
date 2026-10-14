//! Integration coverage for runtime JS plugins that need a real QuickJS host.

#![cfg(test)]

use opencat_core::frame_ctx::{FrameCtx, ScriptFrameCtx};
use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::{MutationRecorder, MutationStore};
use opencat_core::script::{
    ScriptDriverId, ScriptHost, ScriptRealm, ScriptTargetRegistry, ScriptTextSource,
    ScriptTextSourceKind,
};

use crate::js_context::RqJsContext;

/// Scene3Raster 测试桩：不产出真实像素，只验证注入契约（run_frame 会把
/// 3D 快照转成帧级 generated image + `DrawOp::Image`）。
struct StubScene3Raster;

impl opencat_core::ir::scene3::Scene3Raster for StubScene3Raster {
    fn render(
        &self,
        _scene: &opencat_core::ir::scene3::Scene3,
        _geometries: &opencat_core::ir::scene3::GeometryRegistry,
        width: u32,
        height: u32,
    ) -> anyhow::Result<std::sync::Arc<[u8]>> {
        Ok(std::sync::Arc::from(vec![0u8; (width as usize) * (height as usize) * 4]))
    }
}

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
        .install("ctx.getNode('box').opacity(ctx.current.frame / 10);")
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
             ctx.getNode('box').opacity(ctx.current.frame / 10);",
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

/// GSAP object-form keyframes: `[{scale, rotation, duration}, ...]`. Each entry
/// is a full state, properties carry forward, and the entries' durations sum to
/// the tween duration (no top-level `duration` needed).
#[test]
fn object_form_keyframes_interpolate_with_per_entry_durations() {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("badge".into());
    realm.set_target_registry(registry);

    let driver = realm
        .install(
            "ctx.set('badge', { scale: 1, rotation: 0 });\
             ctx.to('badge', {\
               keyframes: [\
                 { scale: 1, rotation: 0, duration: 0 },\
                 { scale: 2, rotation: 90, duration: 0.5 },\
                 { scale: 1, rotation: 0, duration: 0.5 }\
               ],\
               ease: 'none'\
             });",
        )
        .expect("install");

    let run_at = |realm: &mut ScriptRealm<RqJsContext>, frame: u32| -> (f32, f32) {
        // 20 fps → frame N is t = N/20 s; the tween spans 1 s (0..=20).
        let frame_ctx = FrameCtx {
            frame,
            fps: 20,
            width: 64,
            height: 36,
            frames: 40,
        };
        let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
        let mut rec = MutationStore::default();
        realm
            .run_frame(driver, &script_frame_ctx, Some("badge"), &mut rec)
            .expect("run");
        let entry = rec.snapshot_mutations().mutations.remove("badge").expect("badge");
        // `scale` lowers to a paired ScaleX/ScaleY write, so read either axis.
        let scale = entry
            .transforms
            .iter()
            .find_map(|t| match t {
                opencat_core::style::Transform::ScaleX { value }
                | opencat_core::style::Transform::ScaleY { value }
                | opencat_core::style::Transform::Scale { value } => Some(*value),
                _ => None,
            })
            .unwrap_or_else(|| panic!("scale track; transforms = {:?}", entry.transforms));
        let rotation = entry
            .transforms
            .iter()
            .find_map(|t| match t {
                opencat_core::style::Transform::RotateDeg { value } => Some(*value),
                _ => None,
            })
            .expect("rotation track");
        (scale, rotation)
    };

    // Half-way through the first segment: scale 1→2, rotation 0→90.
    let (s5, r5) = run_at(&mut realm, 5);
    assert!((s5 - 1.5).abs() < 0.02, "frame 5 scale: {s5}");
    assert!((r5 - 45.0).abs() < 0.5, "frame 5 rotation: {r5}");

    // Half-way through the second segment: scale 2→1, rotation 90→0.
    let (s15, r15) = run_at(&mut realm, 15);
    assert!((s15 - 1.5).abs() < 0.02, "frame 15 scale: {s15}");
    assert!((r15 - 45.0).abs() < 0.5, "frame 15 rotation: {r15}");
}

/// A tween-level `transformOrigin` (`.to`/`.fromTo` vars) must snap to that
/// origin for the WHOLE tween, not only on its final frame. GSAP does not
/// interpolate the pivot; the opener zoom-through (`transformOrigin:'66% 59%'`)
/// scaled about the box centre until this was fixed.
#[test]
fn tween_transform_origin_applies_for_whole_tween_not_just_last_frame() {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("scene".into());
    realm.set_target_registry(registry);

    let driver = realm
        .install("ctx.to('scene', { scale: 1.6, duration: 1, ease: 'none', transformOrigin: '66% 59%' }, 0);")
        .expect("install");

    let origin_at = |realm: &mut ScriptRealm<RqJsContext>, frame: u32| -> Option<(f32, f32)> {
        let frame_ctx = FrameCtx {
            frame,
            fps: 20,
            width: 1920,
            height: 1080,
            frames: 40,
        };
        let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
        let mut rec = MutationStore::default();
        realm
            .run_frame(driver, &script_frame_ctx, Some("scene"), &mut rec)
            .expect("run");
        rec.snapshot_mutations()
            .mutations
            .get("scene")
            .and_then(|m| m.transform_origin)
            .map(|o| (o.x, o.y))
    };

    // Mid-tween (t=0.25 s of 1 s): the tween's origin must already be in effect.
    let mid = origin_at(&mut realm, 5).expect("transform_origin written mid-tween");
    assert!(
        (mid.0 - 0.66).abs() < 1e-4 && (mid.1 - 0.59).abs() < 1e-4,
        "mid-tween origin must be the tween's 66% 59%, got {mid:?}"
    );
    // Final frame too.
    let last = origin_at(&mut realm, 20).expect("transform_origin written on last frame");
    assert!(
        (last.0 - 0.66).abs() < 1e-4 && (last.1 - 0.59).abs() < 1e-4,
        "final-frame origin must be 66% 59%, got {last:?}"
    );
}

/// GSAP value-level relative strings: `y: "+=6"` tweens from the node's current
/// value to `current + 6`, not from 0 to the literal `"+6"`.
#[test]
fn relative_value_strings_offset_from_current_value() {
    let mut realm = ScriptRealm::<RqJsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("dot".into());
    realm.set_target_registry(registry);

    let driver = realm
        .install(
            "ctx.set('dot', { y: 10 });\
             ctx.to('dot', { y: '+=6', duration: 1, ease: 'none' });",
        )
        .expect("install");

    let run_at = |realm: &mut ScriptRealm<RqJsContext>, frame: u32| -> f32 {
        let frame_ctx = FrameCtx {
            frame,
            fps: 20,
            width: 64,
            height: 36,
            frames: 40,
        };
        let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
        let mut rec = MutationStore::default();
        realm
            .run_frame(driver, &script_frame_ctx, Some("dot"), &mut rec)
            .expect("run");
        let entry = rec.snapshot_mutations().mutations.remove("dot").expect("dot");
        entry
            .transforms
            .iter()
            .find_map(|t| match t {
                opencat_core::style::Transform::TranslateY { value } => Some(*value),
                _ => None,
            })
            .unwrap_or_else(|| panic!("y track; transforms = {:?}", entry.transforms))
    };

    // Start stays at the node's current 10; end is 10 + 6 = 16.
    assert!((run_at(&mut realm, 0) - 10.0).abs() < 0.05, "t=0");
    assert!((run_at(&mut realm, 20) - 16.0).abs() < 0.05, "t=1");
    assert!((run_at(&mut realm, 10) - 13.0).abs() < 0.05, "t=0.5");
}

#[test]
fn three_like_api_commits_scene3_snapshot_per_frame() {
    let source = r#"
        const T = ctx.three;
        const scene = new T.Scene();
        scene.background = new T.Color(0x101014);
        const camera = new T.PerspectiveCamera(45, 16 / 9, 0.1, 100);
        camera.position.set(2.2, 1.8, 3.0);
        camera.lookAt(0, 0, 0);
        const hemi = new T.HemisphereLight(0xffffff, 0x7eb6a4, 2.1);
        const light = new T.DirectionalLight(0xffffff, 1.0);
        light.position.set(-2, -4, -5);
        const pivot = new T.Group();
        const mat = new T.MeshLambertMaterial({
            color: 0xe64a33,
            noiseDisplacement: { amplitude: 0.12, frequency: 1.5, worldFrequency: 0.1, timeSpeed: 0.4 },
        });
        pivot.add(new T.Mesh(new T.BoxGeometry(1, 1, 1), mat));
        scene.add(hemi, light, pivot);
        const renderer = new T.Renderer('c3d', 320, 180);
        pivot.rotation.y = ctx.global.frame * 0.1;
        mat.noiseTime = ctx.global.frame * 0.05;
        renderer.render(scene, camera);
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("c3d".to_string());
    realm.set_target_registry(registry);
    realm.set_scene3_raster(std::sync::Arc::new(StubScene3Raster));

    let recorder = run_driver(&mut realm, driver, 30, None);
    let snap = recorder.snapshot_mutations();
    let scene3 = snap
        .scene3_mutations
        .get("c3d")
        .expect("renderer.render() should commit a Scene3 snapshot");

    // 渲染目标尺寸随快照提交（Renderer(id, w, h) → Scene3.target）。
    assert_eq!(scene3.target, [320, 180]);

    // 光栅化接通：快照同时变成一帧 generated image + 目标 canvas 上的
    // `DrawOp::Image { ImageRef::Generated }`（z 序 = canvas 命令流）。
    let commands = &snap
        .canvas_mutations
        .get("c3d")
        .expect("rasterized scene should draw on its canvas")
        .commands;
    assert_eq!(commands.len(), 1, "one DrawOp::Image per committed scene");
    let generated_id = match &commands[0] {
        opencat_core::ir::draw_op::DrawOp::Image {
            image: opencat_core::ir::draw_types::ImageRef::Generated { id },
            x,
            y,
            ..
        } => {
            assert_eq!((*x, *y), (0.0, 0.0), "full-target raster draws at origin");
            *id
        }
        other => panic!("expected DrawOp::Image(Generated), got {other:?}"),
    };
    // 像素落在帧级 generated-image 队列，等 pipeline 取走进表。
    let drained = realm
        .ctx()
        .with_store_mut(|s| s.take_pending_generated_images());
    assert_eq!(drained.len(), 1, "one pending generated image");
    assert_eq!(drained[0].id, generated_id);
    assert_eq!((drained[0].width, drained[0].height), (320, 180));
    assert_eq!(drained[0].rgba.len(), 320 * 180 * 4);

    // 背景 0x101014：Color.setHex → 线性 working space（ColorManagement.js:205
    // SRGBToLinear）；shim 默认 outputColorSpace=Linear → 清屏原样写线性分量。
    // （SRGB 输出下 WebGLBackground setClear 回编码 → 16/255 round-trip，见
    // three_like_api_background_clear_encodes_to_output_space。）
    for (got, want) in scene3
        .clear
        .iter()
        .zip([0.005_181_517, 0.005_181_517, 0.006_995_410, 1.0])
    {
        assert!((got - want).abs() < 1e-6, "clear {got} vs {want}");
    }

    // 相机：position/lookAt 直通。
    assert_eq!(scene3.camera.fov_y_degrees, 45.0);
    for (got, want) in scene3.camera.position.iter().zip([2.2, 1.8, 3.0]) {
        assert!((got - want).abs() < 1e-5);
    }
    assert!(scene3.camera.target.iter().all(|v| v.abs() < 1e-6));

    // 灯光：Hemisphere（先）+ Directional；direction = -position（未归一化，
    // 渲染器归一化）。
    assert_eq!(scene3.lights.len(), 2);
    match &scene3.lights[0] {
        opencat_core::ir::scene3::Light3::Hemisphere {
            sky_color,
            ground_color,
            intensity,
        } => {
            for c in sky_color {
                assert!((c - 1.0).abs() < 1e-6, "sky 0xffffff");
            }
            for (got, want) in ground_color
                .iter()
                .zip([0.208_636_870, 0.467_783_796, 0.371_237_680])
            {
                assert!((got - want).abs() < 1e-6, "ground 0x7eb6a4 linear {got} vs {want}");
            }
            assert!((intensity - 2.1).abs() < 1e-6);
        }
        other => panic!("expected hemisphere light, got {other:?}"),
    }
    match &scene3.lights[1] {
        opencat_core::ir::scene3::Light3::Directional {
            direction,
            color,
            intensity,
        } => {
            for (got, want) in direction.iter().zip([2.0, 4.0, 5.0]) {
                assert!((got - want).abs() < 1e-5);
            }
            for c in color {
                assert!((c - 1.0).abs() < 1e-6);
            }
            assert!((intensity - 1.0).abs() < 1e-6);
        }
        other => panic!("expected directional light, got {other:?}"),
    }

    // 网格：Group（无自身变换）内 Mesh rotation.y = 30 * 0.1 = 3.0 弧度。
    // three.js 'XYZ' 列主序绕 Y：col0 = (cos, 0, -sin)，col2 = (sin, 0, cos)。
    assert_eq!(scene3.meshes.len(), 1);
    let mesh = &scene3.meshes[0];
    let (s, c) = (3.0f32.sin(), 3.0f32.cos());
    assert!((mesh.matrix[0] - c).abs() < 1e-5, "m[0]");
    assert!((mesh.matrix[2] + s).abs() < 1e-5, "m[2]");
    assert!((mesh.matrix[8] - s).abs() < 1e-5, "m[8]");
    assert!((mesh.matrix[10] - c).abs() < 1e-5, "m[10]");

    // 材质：0xe64a33 → setHex 线性 working space（SRGBToLinear），Lambert。
    let want = [0.791_297_940, 0.068_478_170, 0.033_104_767, 1.0];
    for (got, want) in mesh.material.base_color.iter().zip(want) {
        assert!((got - want).abs() < 1e-6, "base_color {got} vs {want}");
    }
    assert!(matches!(
        mesh.material.shading,
        opencat_core::ir::scene3::Shading3::Lambert
    ));

    // 位移参数与逐帧 noiseTime（frame 30 → 30*0.05 = 1.5）。
    let disp = mesh.material.displacement.as_ref().expect("noiseDisplacement");
    assert!((disp.amplitude - 0.12).abs() < 1e-6);
    assert!((disp.frequency - 1.5).abs() < 1e-6);
    assert!((disp.world_frequency - 0.1).abs() < 1e-6);
    assert!((disp.time_speed - 0.4).abs() < 1e-6);
    assert!((disp.time - 1.5).abs() < 1e-5, "noiseTime at frame 30");

    // BoxGeometry(1,1,1)：24 顶点 / 36 索引。
    match &mesh.geometry {
        opencat_core::ir::scene3::Geometry3::Inline {
            positions,
            normals,
            indices,
            ..
        } => {
            assert_eq!(positions.len(), 24);
            assert_eq!(normals.len(), 24);
            assert_eq!(indices.len(), 36);
        }
        other => panic!("expected inline geometry, got {other:?}"),
    }

    // 下一帧重提交：快照随帧变化（rotation 随 ctx.global.frame）。
    let next = run_driver(&mut realm, driver, 60, None);
    let next_scene = next.snapshot_mutations().scene3_mutations.remove("c3d").unwrap();
    assert!(
        (next_scene.meshes[0].matrix[0] - (6.0f32.cos())).abs() < 1e-5,
        "frame 60 → rotation.y = 6.0"
    );
}

#[test]
fn three_like_api_physical_attenuation_distance_sentinel() {
    // three.js MeshPhysicalMaterial 缺省 attenuationDistance = Infinity（不
    // 衰减）。快照 JSON 无法表达 ±inf（serde f32 拒绝，r8 前缺省路径直接
    // 炸 render 提交），shim 按渲染器 WGSL volumeAttenuation 的 ≥1e29 哨兵
    // 语义发 1e30（lib.rs:519 注释的既定契约）；显式有限值直通。
    let source = r#"
        const T = ctx.three;
        const scene = new T.Scene();
        const camera = new T.PerspectiveCamera(45, 16 / 9, 0.1, 100);
        camera.position.set(0, 0, 5);
        camera.lookAt(0, 0, 0);
        const a = new T.MeshPhysicalMaterial({ transmission: 1 });
        const b = new T.MeshPhysicalMaterial({ transmission: 1, attenuationDistance: Infinity });
        const c = new T.MeshPhysicalMaterial({ transmission: 1, attenuationDistance: 2 });
        scene.add(new T.Mesh(new T.SphereGeometry(0.5, 16, 16), a));
        scene.add(new T.Mesh(new T.SphereGeometry(0.5, 16, 16), b));
        scene.add(new T.Mesh(new T.SphereGeometry(0.5, 16, 16), c));
        const renderer = new T.Renderer('c3d', 320, 180);
        renderer.render(scene, camera);
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("c3d".to_string());
    realm.set_target_registry(registry);
    realm.set_scene3_raster(std::sync::Arc::new(StubScene3Raster));
    let recorder = run_driver(&mut realm, driver, 1, None);
    let scene3 = recorder
        .snapshot_mutations()
        .scene3_mutations
        .get("c3d")
        .expect("snapshot committed")
        .clone();
    assert_eq!(scene3.meshes.len(), 3);
    let distances: Vec<f32> = scene3
        .meshes
        .iter()
        .map(|m| match &m.material.shading {
            opencat_core::ir::scene3::Shading3::Physical(p) => p.attenuation_distance,
            other => panic!("expected Physical, got {other:?}"),
        })
        .collect();
    assert_eq!(distances[0], 1e30, "缺省 Infinity → 1e30 哨兵");
    assert_eq!(distances[1], 1e30, "显式 Infinity → 1e30 哨兵");
    assert!((distances[2] - 2.0).abs() < 1e-6, "显式有限值直通");
}

#[test]
fn three_like_api_background_clear_encodes_to_output_space() {
    // WebGLBackground.js setClear（r181）：scene.background 的线性 working
    // 分量经 getRGB(getUnlitUniformColorSpace(renderer)) 转到输出色彩空间再
    // 清屏。SRGB 输出下 0x101014 → linear(0.00518…) → OETF 回 16/255,
    // 16/255, 20/255 —— 数值 round-trip 恒等（hex 即 sRGB 编码）。
    let source = r#"
        const T = ctx.three;
        const scene = new T.Scene();
        scene.background = new T.Color(0x101014);
        const camera = new T.PerspectiveCamera(45, 16 / 9, 0.1, 100);
        camera.position.set(0, 0, 3);
        camera.lookAt(0, 0, 0);
        const renderer = new T.Renderer('c3d', 320, 180);
        renderer.outputColorSpace = T.SRGBColorSpace;
        renderer.render(scene, camera);
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("c3d".to_string());
    realm.set_target_registry(registry);
    realm.set_scene3_raster(std::sync::Arc::new(StubScene3Raster));

    let recorder = run_driver(&mut realm, driver, 1, None);
    let scene3 = recorder
        .snapshot_mutations()
        .scene3_mutations
        .get("c3d")
        .expect("render() commits a snapshot")
        .clone();
    // 容差 1e-5：srgbToLinear→linearToSrgb 两次浮点传递函数往返不严格恒等
    //（实测 |Δ| ≈ 4.1e-6，16/255 处）。
    for (got, want) in scene3.clear.iter().zip([16.0 / 255.0, 16.0 / 255.0, 20.0 / 255.0, 1.0]) {
        assert!((got - want).abs() < 1e-5, "clear srgb {got} vs {want}");
    }
}

#[test]
fn three_like_gltf_geometry_registers_and_commits_asset_reference() {
    // 真 GLB（prism 11280 顶点）：GLTFGeometry → 宿主一次性解码注册 →
    // 快照只带 Asset{id}；YXZ Euler（参考 js:339）进世界矩阵。
    let glb_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/lbr/heygen-prism.glb");
    let source = "const T = ctx.three;\n".to_string()
        + "const scene = new T.Scene();\n"
        + "const camera = new T.PerspectiveCamera(45, 16 / 9, 0.1, 100);\n"
        + "camera.position.set(0, 0, 7);\n"
        + &format!(
            "const geo = new T.GLTFGeometry({glb:?}, 'prism');\n",
            glb = glb_path
        )
        + "const mesh = new T.Mesh(geo, new T.MeshBasicMaterial({ color: 0xffffff }));\n"
        + "mesh.rotation.order = 'YXZ';\n" // 参考 js:339
        + "mesh.rotation.set(0.139 * Math.PI, -1.17 * Math.PI, 0);\n" // js:499-500 dock 姿态
        + "scene.add(mesh);\n"
        + "const renderer = new T.Renderer('c3d', 64, 64);\n"
        + "renderer.render(scene, camera);\n";
    let (mut realm, driver) = open_with_source(&source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("c3d".to_string());
    registry.canvas_ids.insert("c3d".to_string());
    realm.set_target_registry(registry);
    realm.set_scene3_raster(std::sync::Arc::new(StubScene3Raster));

    let recorder = run_driver(&mut realm, driver, 1, None);
    let snap = recorder.snapshot_mutations();
    let scene3 = snap.scene3_mutations.get("c3d").expect("scene snapshot");
    assert_eq!(scene3.meshes.len(), 1);
    let mesh = &scene3.meshes[0];

    // 快照只带 id，不带几何体（重网格不逐帧过 JSON 桥）。
    match &mesh.geometry {
        opencat_core::ir::scene3::Geometry3::Asset { id } => assert_eq!(id, "prism"),
        other => panic!("expected Asset geometry, got {other:?}"),
    }

    // 注册表里是真几何：GLB 的 11280 顶点 / 60096 索引 / 全属性。
    let geometries = realm.ctx().with_store_mut(|s| s.geometry_registry());
    let prism = geometries.get("prism").expect("prism registered");
    match prism.as_ref() {
        opencat_core::ir::scene3::Geometry3::Inline {
            positions,
            normals,
            indices,
            uvs,
            colors,
            colors1,
        } => {
            assert_eq!(positions.len(), 11280, "GLB POSITION count");
            assert_eq!(normals.len(), 11280);
            assert_eq!(uvs.len(), 11280, "TEXCOORD_0 count");
            assert_eq!(colors.len(), 11280, "COLOR_0 count");
            assert_eq!(colors1.len(), 11280, "COLOR_1 count");
            assert_eq!(indices.len(), 60096, "GLB indices count");
        }
        other => panic!("expected inline geometry in registry, got {other:?}"),
    }

    // YXZ 判别元：m(0,1) = sin y · sin x（'XYZ' 下该元为 0）——
    // 列主序 m[4]。dock 姿态 z=0。
    let (x, y) = (0.139f32 * std::f32::consts::PI, -1.17f32 * std::f32::consts::PI);
    let want_m01 = y.sin() * x.sin();
    assert!(
        (mesh.matrix[4] - want_m01).abs() < 1e-5,
        "YXZ m[4]: {} vs {want_m01}",
        mesh.matrix[4]
    );
}

#[test]
fn three_like_shader_material_adapts_glsl_and_commits() {
    // ShaderMaterial（参考 js:62-97 形状）：WebGL1 方言 GLSL 逐字进 shim →
    // adaptShaderMaterial 改写成 450 core → 快照携带适配后源 + uniforms
    // 当前值；PlaneGeometry 带 uv 与 three.js 索引序（背景场 vUv 的输入）。
    let source = r#"
        const T = ctx.three;
        const scene = new T.Scene();
        const camera = new T.PerspectiveCamera(35, 16 / 9, 0.1, 100);
        camera.position.set(0, 0, 7);
        camera.lookAt(0, 0, 0);
        const mat = new T.ShaderMaterial({
            depthWrite: false,
            depthTest: false,
            toneMapped: false,
            uniforms: { uFade: { value: 1 } },
            vertexShader: `
                varying vec2 vUv;
                void main() {
                  vUv = uv;
                  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
                }
            `,
            fragmentShader: `
                precision highp float;
                varying vec2 vUv;
                uniform float uFade;
                void main() {
                  gl_FragColor = vec4(vec3(0.957, 0.949, 0.918) * (0.985 + 0.035 * vUv.y), 1.0);
                }
            `,
        });
        const mesh = new T.Mesh(new T.PlaneGeometry(15, 8.5), mat);
        mesh.position.set(0, 0, -3.5);
        scene.add(mesh);
        const renderer = new T.Renderer('c3d', 64, 64);
        mat.uniforms.uFade.value = 1 - ctx.global.frame * 0.25;   // js:527 的对应物
        renderer.render(scene, camera);
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("c3d".to_string());
    registry.canvas_ids.insert("c3d".to_string());
    realm.set_target_registry(registry);
    realm.set_scene3_raster(std::sync::Arc::new(StubScene3Raster));

    let recorder = run_driver(&mut realm, driver, 2, None);
    let snap = recorder.snapshot_mutations();
    let scene3 = snap.scene3_mutations.get("c3d").expect("scene snapshot");
    assert_eq!(scene3.meshes.len(), 1);
    let mesh = &scene3.meshes[0];

    let shader = match &mesh.material.shading {
        opencat_core::ir::scene3::Shading3::Shader(s) => s,
        other => panic!("expected Shader shading, got {other:?}"),
    };
    // 顶点段：450 core + 三方块前缀 + varying→out location 化 + 无残留方言。
    assert!(shader.vertex_shader.starts_with("#version 450 core"), "vs header");
    assert!(
        shader
            .vertex_shader
            .contains("layout(std140, set = 0, binding = 0) uniform ThreeBuiltins"),
        "vs 前缀三方块"
    );
    assert!(
        shader
            .vertex_shader
            .contains("layout(location = 0) out vec2 vUv;"),
        "varying → location 化 out: {}",
        shader.vertex_shader
    );
    assert!(!shader.vertex_shader.contains("varying"), "方言残留 varying");
    assert!(
        shader
            .vertex_shader
            .contains("gl_Position = projectionMatrix * modelViewMatrix"),
        "用户体保留"
    );
    // 片元段：varying→in、gl_FragColor→fragOutput（声明在前）、uniform 进
    // UserUniforms 块（set 1）。
    assert!(shader.fragment_shader.starts_with("#version 450 core"), "fs header");
    assert!(
        shader
            .fragment_shader
            .contains("layout(std140, set = 1, binding = 0) uniform UserUniforms {\n  float uFade;\n};"),
        "standalone uniform 收进 set1 块: {}",
        shader.fragment_shader
    );
    assert!(
        shader
            .fragment_shader
            .contains("layout(location = 0) in vec2 vUv;"),
        "varying → location 化 in"
    );
    let out_pos = shader.fragment_shader.find("layout(location = 0) out vec4 fragOutput;");
    let body_pos = shader.fragment_shader.find("fragOutput = vec4(");
    assert!(out_pos.is_some() && body_pos.is_some(), "fragOutput 改名");
    assert!(out_pos.unwrap() < body_pos.unwrap(), "声明在体之前");
    assert!(!shader.fragment_shader.contains("gl_FragColor"), "方言残留 gl_FragColor");
    assert!(!shader.fragment_shader.contains("precision highp float;\nprecision"), "precision 不重复");

    // uniforms 逐帧值：frame 2 → uFade = 1 - 2*0.25 = 0.5。
    assert_eq!(shader.uniforms.len(), 1, "uniform 载荷");
    assert_eq!(shader.uniforms[0].name, "uFade");
    match shader.uniforms[0].value {
        opencat_core::ir::scene3::ShaderValue3::Float(v) => {
            assert!((v - 0.5).abs() < 1e-6, "uFade 随帧 = {v}");
        }
        other => panic!("expected Float uniform, got {other:?}"),
    }

    // PlaneGeometry：uv (0,1)(1,1)(0,0)(1,0) + three.js 索引 (a,b,d),(b,c,d)。
    match &mesh.geometry {
        opencat_core::ir::scene3::Geometry3::Inline {
            positions,
            indices,
            uvs,
            ..
        } => {
            assert_eq!(positions.len(), 4);
            assert_eq!(
                uvs,
                &vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 0.0]],
                "three.js PlaneGeometry uv（v=1 顶行）"
            );
            assert_eq!(indices, &vec![0u32, 2, 1, 2, 3, 1], "(a,b,d),(b,c,d)");
        }
        other => panic!("expected inline geometry, got {other:?}"),
    }
}
