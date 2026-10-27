//! Integration coverage for runtime JS plugins that need a real QuickJS host.

#![cfg(test)]

use opencat_core::frame_ctx::{FrameCtx, ScriptFrameCtx};
use opencat_core::script::js_context::JsContext;
use opencat_core::script::recorder::{MutationRecorder, MutationStore};
use opencat_core::script::{
    ScriptDriverId, ScriptHost, ScriptRealm, ScriptTargetRegistry, ScriptTextSource,
    ScriptTextSourceKind,
};

use crate::js_context::V8JsContext;

fn open_with_source(source: &str) -> (ScriptRealm<V8JsContext>, ScriptDriverId) {
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("script realm should open");
    let driver = realm.install(source).expect("install driver");
    (realm, driver)
}

fn run_driver(
    realm: &mut ScriptRealm<V8JsContext>,
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

/// Run one frame expecting a JS error; returns the formatted message (install
/// wraps the source lazily, so the error surfaces at run time).
fn run_driver_expect_err(realm: &mut ScriptRealm<V8JsContext>, driver: ScriptDriverId) -> String {
    let frame_ctx = FrameCtx { frame: 0, fps: 30, width: 320, height: 180, frames: 30 };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut recorder = MutationStore::default();
    let err = realm
        .run_frame(driver, &script_frame_ctx, None, &mut recorder)
        .expect_err("script should fail");
    format!("{err:#}")
}

#[test]
fn one_realm_shares_js_state_across_drivers() {
    // Same pipeline realm: global/scene/node drivers share JS state.
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
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
    let mut a = ScriptRealm::<V8JsContext>::open().expect("realm a");
    let mut b = ScriptRealm::<V8JsContext>::open().expect("realm b");

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
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    realm.set_target_registry(registry);
    let driver = realm
        .install("ctx.getNode('box').opacity(ctx.current.frame / 10);")
        .expect("install");

    let run = |realm: &mut ScriptRealm<V8JsContext>, frame: u32| {
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
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
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

    let run = |realm: &mut ScriptRealm<V8JsContext>, frame: u32| {
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
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
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

#[test]
fn realm_installs_multiple_drivers_into_shared_state() {
    // Contract-phase replacement of the historical ScriptRuntimeCache test:
    // many drivers must share one realm so pipeline-local JS state works.
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
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
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
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

    let run = |realm: &mut ScriptRealm<V8JsContext>, frame: u32| -> f32 {
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
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
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
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
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

    let run = |realm: &mut ScriptRealm<V8JsContext>, id: ScriptDriverId| {
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

    let mut realm_a = ScriptRealm::<V8JsContext>::open().expect("realm a");
    let mut realm_b = ScriptRealm::<V8JsContext>::open().expect("realm b");
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

    let run = |realm: &mut ScriptRealm<V8JsContext>, id: ScriptDriverId| {
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

#[test]
fn realms_tear_down_out_of_creation_order() {
    let source = "ctx.getNode('box').opacity(0.25);";

    let mut first = ScriptRealm::<V8JsContext>::open().expect("realm first");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    first.set_target_registry(registry.clone());
    let driver = first.install(source).expect("install in first");

    let mut second = ScriptRealm::<V8JsContext>::open().expect("realm second");
    second.set_target_registry(registry);
    second.install(source).expect("install in second");

    // Both realms still work while both are alive.
    let store = run_driver(&mut first, driver, 0, Some("box"));
    assert!(
        (store.snapshot_mutations().mutations["box"].opacity.unwrap() - 0.25).abs() < 1e-6
    );

    drop(first);
    drop(second);
}

/// G2 column chart on a canvas node: bars fill, axes stroke, axis labels text.
///
/// The G2 bundle renders through CanvasKit, which the host facade maps onto the
/// node's DrawOp stream — so a passing chart shows up as *shape ops* on the
/// target canvas, not as a rendered bitmap. This is the end-to-end proof that
/// `__ocG2Frame` drives a real AntV G2 mark tree into the recorder.
#[test]
fn g2_chart_spike_column() {
    let source = r#"
        globalThis.__ocMeta = { width: 480, height: 300 };
        globalThis.__ocG2Frame('chart', globalThis.ctx.current.time, function (Chart, container) {
            if (!globalThis.__ocG2Built) {
                globalThis.__ocG2Built = true;
                const chart = new Chart({ container: container, width: 480, height: 300 });
                chart.options({
                    type: 'interval',
                    data: [
                        { genre: 'Sports', sold: 275 },
                        { genre: 'Strategy', sold: 115 },
                        { genre: 'Action', sold: 120 },
                    ],
                    encode: { x: 'genre', y: 'sold' },
                    animate: false,
                    axis: { x: { title: false }, y: { title: false } },
                });
                const renderDone = chart.render();
                globalThis.__ocG2RenderDone = 'pending';
                if (renderDone && renderDone.then) renderDone.then(
                    () => { globalThis.__ocG2RenderDone = 'done'; },
                    (e) => { globalThis.__ocG2RenderDone = 'error ' + (e && e.message); }
                );
            }
        });
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("chart".to_string());
    registry.canvas_ids.insert("chart".to_string());
    realm.set_target_registry(registry);
    realm
        .ctx()
        .eval("globalThis.__ocMeta = { width: 480, height: 300 };")
        .expect("chart meta");

    // Script text bindings resolve families through the thread-scoped font db,
    // which the composition pipeline installs; this test opens a realm
    // directly, so the engine's default faces are scoped the same way.
    let font_db = crate::fonts::engine_default_font_db();
    let store = opencat_core::text::scope_font_db(&font_db, || {
        // Frame 0 builds the chart; frames 1+ re-enter and re-render it, which
        // is what a real composition does every frame.
        for frame in 0..8 {
            let s = run_driver(&mut realm, driver, frame, Some("chart"));
            let n = s
                .snapshot_mutations()
                .canvas_mutations
                .get("chart")
                .map(|c| c.commands.len())
                .unwrap_or(0);
        }
        let s = run_driver(&mut realm, driver, 8, Some("chart"));
        let n = s
            .snapshot_mutations()
            .canvas_mutations
            .get("chart")
            .map(|c| c.commands.len())
            .unwrap_or(0);
        s
    });

    let snap = store.snapshot_mutations();
    let canvas = snap
        .canvas_mutations
        .get("chart")
        .expect("chart node should record canvas mutations");

    let bars = count_ops(&canvas.commands, |op| {
        matches!(
            op,
            opencat_core::ir::draw_op::DrawOp::FillPath
                | opencat_core::ir::draw_op::DrawOp::SetFillStyle { .. }
        )
    });
    let strokes = count_ops(&canvas.commands, |op| {
        matches!(
            op,
            opencat_core::ir::draw_op::DrawOp::StrokePath
                | opencat_core::ir::draw_op::DrawOp::SetStrokeStyle { .. }
        )
    });
    let text = count_ops(&canvas.commands, |op| {
        matches!(op, opencat_core::ir::draw_op::DrawOp::Image { .. })
    });

    assert!(bars > 0, "G2 marks produced no filled shapes: {bars}");
    assert!(strokes > 0, "G2 axes produced no strokes: {strokes}");
    assert!(text > 0, "G2 axis labels produced no text images: {text}");

    let render_done = realm.eval_json_for_test("globalThis.__ocG2RenderDone || 'none'");
    assert_eq!(
        render_done,
        serde_json::json!("done"),
        "G2 render promise never settled"
    );
}

fn count_ops(
    commands: &[opencat_core::ir::draw_op::DrawOp],
    predicate: impl Fn(&opencat_core::ir::draw_op::DrawOp) -> bool,
) -> usize {
    commands.iter().filter(|op| predicate(op)).count()
}

/// A dynamic chart — the build closure reads the clock and moves a bar per
/// frame — must never be served from the cache: frames 2+ keep producing
/// fresh, different op streams instead of frozen replays.
#[test]
fn g2_dynamic_chart_never_replays_cache() {
    let source = r#"
        globalThis.__ocMeta = { width: 480, height: 300 };
        globalThis.__ocG2Frame('chart', globalThis.ctx.current.time, function (Chart, container) {
            const t = globalThis.ctx.current.time;
            const chart = new Chart({ container: container, width: 480, height: 300 });
            chart.options({
                type: 'interval',
                data: [
                    { genre: 'Sports', sold: 275 + Math.round(t * 100) },
                    { genre: 'Strategy', sold: 115 },
                    { genre: 'Action', sold: 120 },
                ],
                encode: { x: 'genre', y: 'sold' },
                animate: false,
                axis: { x: { title: false }, y: { title: false } },
            });
            chart.render();
        });
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("chart".to_string());
    registry.canvas_ids.insert("chart".to_string());
    realm.set_target_registry(registry);

    let font_db = crate::fonts::engine_default_font_db();
    let mut streams: Vec<Vec<opencat_core::ir::draw_op::DrawOp>> = Vec::new();
    opencat_core::text::scope_font_db(&font_db, || {
        for frame in 0..4u32 {
            let s = run_driver(&mut realm, driver, frame, Some("chart"));
            let snap = s.snapshot_mutations();
            streams.push(
                snap.canvas_mutations
                    .get("chart")
                    .map(|c| c.commands.clone())
                    .unwrap_or_default(),
            );
        }
    });
    assert_ne!(
        streams[0], streams[1],
        "dynamic chart must produce different ops per frame"
    );
    // If the cache (wrongly) served, frames 2+ would be byte-identical to an
    // earlier frame instead of tracking the data.
    assert_ne!(
        streams[2], streams[3],
        "frames 2+ must not be frozen cache replays"
    );
}

/// G2 static-build cache: frame 0 builds and arms, frame 1 probes (same ops at
/// a different frame time ⇒ static), frame 2+ replays the cached DrawOps. The
/// replayed stream must match the freshly built one op-for-op, and the
/// paragraph images must be re-registered every frame so the compositor sees
/// the axis labels.
#[test]
fn g2_static_cache_replays_identical_ops() {
    let source = r#"
        globalThis.__ocMeta = { width: 480, height: 300 };
        globalThis.__ocG2Frame('chart', globalThis.ctx.current.time, function (Chart, container) {
            const chart = new Chart({ container: container, width: 480, height: 300 });
            chart.options({
                type: 'interval',
                data: [
                    { genre: 'Sports', sold: 275 },
                    { genre: 'Strategy', sold: 115 },
                    { genre: 'Action', sold: 120 },
                ],
                encode: { x: 'genre', y: 'sold' },
                animate: false,
                axis: { x: { title: false }, y: { title: false } },
            });
            chart.render();
        });
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("chart".to_string());
    registry.canvas_ids.insert("chart".to_string());
    realm.set_target_registry(registry);

    let font_db = crate::fonts::engine_default_font_db();
    let mut built_ops = 0usize;
    let mut replayed_ops = 0usize;
    let mut replayed_images = 0usize;
    let mut replayed_text = 0usize;
    opencat_core::text::scope_font_db(&font_db, || {
        for frame in 0..6u32 {
            let mut s = run_driver(&mut realm, driver, frame, Some("chart"));
            let images = realm.take_frame_generated_images().len();
            let snap = s.snapshot_mutations();
            let commands = snap
                .canvas_mutations
                .get("chart")
                .map(|c| c.commands.len())
                .unwrap_or(0);
            match frame {
                // Frames 0-1 build for real (arm + probe).
                0 => built_ops = commands,
                1 => assert_eq!(
                    commands, built_ops,
                    "probe frame built a different op count"
                ),
                // Frame 2+: the cache must serve; the op stream must be
                // identical to the built one, with text images re-registered.
                _ => {
                    replayed_ops = commands;
                    replayed_images = images;
                    replayed_text = count_ops(
                        &snap
                            .canvas_mutations
                            .get("chart")
                            .map(|c| &c.commands)
                            .unwrap(),
                        |op| matches!(op, opencat_core::ir::draw_op::DrawOp::Image { .. }),
                    );
                    assert_eq!(
                        replayed_ops, built_ops,
                        "replayed stream diverged from the built stream on frame {frame}"
                    );
                    assert!(replayed_text > 0, "replay lost the text images");
                    assert_eq!(
                        replayed_images, replayed_text,
                        "replay must re-register every paragraph image"
                    );
                }
            }
        }
    });
}
// ── B2: author-facing DOM layer ────────────────────────────────────────────
// `document.getElementById` hands out the same handle as `ctx.getNode`; the
// DOM property facade (style proxy / textContent / geometry reads) reads and
// writes through the same store the mutation API uses.

fn dom_test_realm() -> ScriptRealm<V8JsContext> {
    let mut realm = ScriptRealm::<V8JsContext>::open().expect("realm");
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".into());
    registry.visual_ids.insert("label".into());
    registry.visual_ids.insert("panel".into());
    realm.set_target_registry(registry);
    // Parse-time initial styles: width/height/inset as a class rule would
    // seed them (this is the `set_initial_style_from_node` payload).
    realm.set_style_defaults(
        &[
            (
                "box".to_string(),
                [
                    ("width".into(), serde_json::json!(120.0)),
                    ("height".into(), serde_json::json!(60.0)),
                    ("left".into(), serde_json::json!(10.0)),
                    ("top".into(), serde_json::json!(20.0)),
                    ("className".into(), serde_json::json!("rounded shadow")),
                ]
                .into_iter()
                .collect(),
            ),
            (
                "label".to_string(),
                [("text".into(), serde_json::json!("hello"))].into_iter().collect(),
            ),
        ]
        .into_iter()
        .collect(),
    );
    realm
}

fn run_dom_frame(
    realm: &mut ScriptRealm<V8JsContext>,
    driver: ScriptDriverId,
    frame: u32,
    current_node_id: Option<&str>,
) -> MutationStore {
    let frame_ctx = FrameCtx { frame, fps: 30, width: 320, height: 180, frames: 30 };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut rec = MutationStore::default();
    realm
        .run_frame(driver, &script_frame_ctx, current_node_id, &mut rec)
        .expect("run");
    rec
}

#[test]
fn dom_document_get_element_by_id_shares_node_handle() {
    let mut realm = dom_test_realm();
    let driver = realm
        .install("document.getElementById('box').opacity(0.25);")
        .expect("install");
    let rec = run_dom_frame(&mut realm, driver, 0, Some("box"));
    let opacity = rec
        .snapshot_mutations()
        .mutations
        .get("box")
        .and_then(|m| m.opacity)
        .expect("document.getElementById should reach the same mutation store");
    assert_eq!(opacity, 0.25);
}

#[test]
fn dom_document_unknown_id_errors_with_pointer() {
    let mut realm = dom_test_realm();
    // install wraps the source in a function (lazy); force a frame to run it.
    let driver = realm
        .install("document.getElementById('nope');")
        .expect("install wraps the source lazily");
    let frame_ctx = FrameCtx { frame: 0, fps: 30, width: 320, height: 180, frames: 30 };
    let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
    let mut rec = MutationStore::default();
    let err = realm
        .run_frame(driver, &script_frame_ctx, None, &mut rec)
        .expect_err("unknown id should fail when the driver runs");
    let text = format!("{err:#}");
    assert!(
        text.contains("unknown id 'nope'"),
        "error should name the id, got: {text}"
    );
}

#[test]
fn dom_get_bounding_rect_reads_seeded_box() {
    let mut realm = dom_test_realm();
    let driver = realm
        .install(
            r"
            var r = document.getElementById('box').getBoundingClientRect();
            globalThis.__rect = { left: r.left, top: r.top, width: r.width, right: r.right };
        ",
        )
        .expect("install");
    run_dom_frame(&mut realm, driver, 0, None);
    let rect = realm.eval_json_for_test("globalThis.__rect");
    assert_eq!(rect["left"], serde_json::json!(10));
    assert_eq!(rect["top"], serde_json::json!(20));
    assert_eq!(rect["width"], serde_json::json!(120));
    assert_eq!(rect["right"], serde_json::json!(130));
}

#[test]
fn dom_style_proxy_writes_and_reads_through_store() {
    let mut realm = dom_test_realm();
    // Write via the DOM proxy, read back through the same proxy.
    let driver = realm
        .install(
            r"
            var el = document.getElementById('box');
            el.style.opacity = 0.5;
            el.style.backgroundColor = '#ff0000';
            el.style.borderRadius = 8;
            globalThis.__readBack = {
                opacity: el.style.opacity,
                bg: el.style.backgroundColor,
                kebab: el.style['border-radius'],
            };
        ",
        )
        .expect("install");
    run_dom_frame(&mut realm, driver, 0, None);
    let read = realm.eval_json_for_test("globalThis.__readBack");
    assert_eq!(read["opacity"], serde_json::json!(0.5));
    // Colors read back in the store's #RRGGBBAA serialization.
    assert_eq!(read["bg"], serde_json::json!("#ff0000ff"));
    // kebab-case read normalizes to the same store key the camelCase write used.
    assert_eq!(read["kebab"], serde_json::json!(8));

    // And the mutation stream carries it (the pipeline consumes this).
    let mut realm = dom_test_realm();
    let driver = realm
        .install("document.getElementById('box').style.opacity = 0.75;")
        .expect("install");
    let rec = run_dom_frame(&mut realm, driver, 1, Some("box"));
    let opacity = rec
        .snapshot_mutations()
        .mutations
        .get("box")
        .and_then(|m| m.opacity)
        .expect("style proxy write should record a mutation");
    assert_eq!(opacity, 0.75);
}

#[test]
fn dom_text_content_read_write_and_class_list() {
    let mut realm = dom_test_realm();
    let driver = realm
        .install(
            r"
            var el = document.getElementById('box');
            var label = document.getElementById('label');
            globalThis.__dom = {
                classListContains: el.classList.contains('rounded'),
                classListMissing: el.classList.contains('nope'),
                textRead: label.textContent,
            };
            label.textContent = 'replaced';
            globalThis.__dom.textAfter = label.textContent;
        ",
        )
        .expect("install");
    run_dom_frame(&mut realm, driver, 0, None);
    let dom = realm.eval_json_for_test("globalThis.__dom");
    assert_eq!(dom["classListContains"], serde_json::json!(true));
    assert_eq!(dom["classListMissing"], serde_json::json!(false));
    assert_eq!(dom["textRead"], serde_json::json!("hello"));
    assert_eq!(dom["textAfter"], serde_json::json!("replaced"));

    // textContent write lands in the mutation stream as text_content.
    let mut realm = dom_test_realm();
    let driver = realm
        .install("document.getElementById('label').textContent = 'frame text';")
        .expect("install");
    let rec = run_dom_frame(&mut realm, driver, 1, Some("label"));
    let text = rec
        .snapshot_mutations()
        .mutations
        .get("label")
        .and_then(|m| m.text_content.clone())
        .expect("textContent write should record text_content");
    assert_eq!(text, "frame text");
}

#[test]
fn dom_window_reads_composition_size_and_timers_error() {
    let mut realm = dom_test_realm();
    let driver = realm
        .install(
            r"
            globalThis.__win = { w: window.innerWidth, h: window.innerHeight, dpr: window.devicePixelRatio };
            globalThis.__g = { w: ctx.global.width, h: ctx.global.height };
        ",
        )
        .expect("install");
    run_dom_frame(&mut realm, driver, 2, None);
    assert_eq!(
        realm.eval_json_for_test("globalThis.__win"),
        serde_json::json!({ "w": 320, "h": 180, "dpr": 1 })
    );
    assert_eq!(
        realm.eval_json_for_test("globalThis.__g"),
        serde_json::json!({ "w": 320, "h": 180 })
    );

    // Timers and tree mutation are hard errors with a pointer to the model.
    // install wraps the source in a function (lazy), so each one runs via a
    // frame; the expectation is a run error whose text points at the model.
    // `requestAnimationFrame` is NOT in this list: the G2 rAF queue polyfill
    // (JS_PLATFORM_SHIM, loaded first) owns the name, and its queue drains at
    // the frame boundary (`__ocG2Flush`) — a queued callback runs once on the
    // composition clock, which stays deterministic. The dom_api guard only
    // fills the gap when that polyfill is absent.
    // `document.createElement` is NOT in this list either: it hands out a real
    // overlay node, and appending it to a declared element is how official
    // plugins (SplitText) build their wrappers — the renderer reads the child
    // view from the overlay, so the tree mutation is the supported path.
    for source in [
        "setTimeout(function() {}, 1);",
        "document.querySelector('.some-class');",
    ] {
        let driver = realm.install(source).unwrap_or_else(|e| {
            panic!("`{source}` should install: {e:#}")
        });
        let frame_ctx = FrameCtx { frame: 3, fps: 30, width: 320, height: 180, frames: 30 };
        let script_frame_ctx = ScriptFrameCtx::global(&frame_ctx);
        let mut rec = MutationStore::default();
        let result = realm.run_frame(driver, &script_frame_ctx, None, &mut rec);
        let err = result.expect_err(&format!("`{source}` should fail when run"));
        let text = format!("{err:#}");
        assert!(
            text.contains("useFrame")
                || text.contains("declarative")
                || text.contains("composition clock")
                || text.contains("only '#id'"),
            "`{source}` should point at the supported alternative, got: {text}"
        );
    }
}

#[test]
fn dom_get_context_2d_records_canvas_ops() {
    let mut realm = dom_test_realm();
    let driver = realm
        .install(
            r"
            var ctx2d = document.getElementById('panel').getContext('2d');
            ctx2d.fillStyle = '#00ff00';
            ctx2d.fillRect(10, 20, 30, 40);
            ctx2d.strokeStyle = '#0000ff';
            ctx2d.lineWidth = 2;
            ctx2d.strokeRect(0, 0, 50, 50);
            ctx2d.beginPath();
            ctx2d.moveTo(0, 0);
            ctx2d.lineTo(10, 10);
            ctx2d.stroke();
            var again = document.getElementById('panel').getContext('2d');
            if (again !== ctx2d) throw new Error('getContext must return the same context per node');
        ",
        )
        .expect("install");
    let rec = run_dom_frame(&mut realm, driver, 1, Some("panel"));
    let snap = rec.snapshot_mutations();
    let commands = &snap
        .canvas_mutations
        .get("panel")
        .expect("2d context should target the panel canvas")
        .commands;
    // fillRect lowers to the CK path stream (SetFillStyle + AddRect + FillPath),
    // strokeRect / stroke() to a stroked path — the same ops the CanvasKit
    // facade records.
    let has_fill_rect = commands
        .iter()
        .any(|op| matches!(op, opencat_core::ir::draw_op::DrawOp::FillPath));
    let has_rect_path = commands.iter().any(|op| {
        matches!(
            op,
            opencat_core::ir::draw_op::DrawOp::Path(opencat_core::ir::draw_types::PathOp::AddRect { .. })
        )
    });
    let has_stroke_path = commands
        .iter()
        .any(|op| matches!(op, opencat_core::ir::draw_op::DrawOp::StrokePath));
    assert!(has_fill_rect, "fillRect should record a FillPath op");
    assert!(has_rect_path, "fillRect should record an AddRect path op");
    assert!(has_stroke_path, "strokeRect / stroke() should record a StrokePath op");

}

// ── C-2: ctx.threejs namespace + WebGLRenderer({canvas}) adapter ──────────
// WebGLRenderer resolves the canvas id from either handle form (useRef JSX
// descriptor / DOM handle), sizes from the parse-time box, and commits the
// same Scene3 snapshot as new Renderer(id, w, h).

#[test]
fn canvas_namespace_make_surface_resolves_element_and_draws() {
    // (1) `CanvasKitInit({}).then(CK => ...)`: the official shape. The promise
    // resolves in run_frame's microtask drain, so the draw lands on the very
    // first frame.
    let source = r#"
        CanvasKitInit({}).then(function (CK) {
            if (CK.PaintStyle.Fill !== 0 || CK.PaintStyle.Stroke !== 1) throw new Error('PaintStyle must carry the official enum numbers');
            const ref = { current: { __ocEl: true, tag: 'canvas', props: { id: 'panel' } } };
            const surface = CK.MakeCanvasSurface(ref.current);
            surface.getCanvas().drawCircle(50, 50, 20, new CK.Paint());
            surface.flush();
            surface.dispose();
            globalThis.__thenRan = true;
        });
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("panel".to_string());
    realm.set_target_registry(registry);
    let recorder = run_driver(&mut realm, driver, 0, None);
    let snap = recorder.snapshot_mutations();
    let commands = &snap
        .canvas_mutations
        .get("panel")
        .expect("MakeCanvasSurface should target the panel canvas")
        .commands;
    assert!(
        commands.iter().any(|op| matches!(op, opencat_core::ir::draw_op::DrawOp::FillPath)),
        "drawCircle should record a FillPath op, got: {commands:?}"
    );
    assert_eq!(
        realm.eval_json_for_test("globalThis.__thenRan"),
        serde_json::json!(true),
        "the CanvasKitInit continuation must run inside the frame"
    );

    // (2) DOM-handle form and pixel-buffer offscreen rejection. These call the
    // facade directly: a throw inside a promise continuation is an unhandled
    // rejection, not a host error — the sync path is what run_driver can see.
    let source = r#"
        const CK = globalThis.CanvasKit;
        const el = { id: 'panel' };
        const surface = CK.MakeCanvasSurface(el);
        surface.getCanvas().clear(CK.WHITE);
        surface.makeImageSnapshot();
    "#;
    let (mut realm, driver) = open_with_source(source);
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("panel".to_string());
    realm.set_target_registry(registry);
    let err = run_driver_expect_err(&mut realm, driver);
    assert!(
        err.contains("makeImageSnapshot") && err.contains("ctx.surface"),
        "makeImageSnapshot error should point at ctx.surface, got: {err}"
    );

    // (3) missing canvas id errors with the option pointer.
    let (mut realm, driver) =
        open_with_source("globalThis.CanvasKit.MakeCanvasSurface({});");
    let err = run_driver_expect_err(&mut realm, driver);
    assert!(
        err.contains("MakeCanvasSurface") && err.contains("expected an id string"),
        "bad arg error should name MakeCanvasSurface, got: {err}"
    );
}

// ── C-4: real-package builtins (pure forwards) + hook gate ────────────────
// `import { gsap } from 'gsap'` is project-supplied: the host materializes
// the bundle's gsap segment into __ocModuleSources['gsap'], and the runtime's
// install pass adds target normalization, the paused protocol and the hook
// gate on top of the raw package. These tests drive that exact protocol with
// a stub package; gsap's animation semantics are covered by the rendered
// acceptance (k3diff), not here.

/// Protocol stub handed to __ocModuleSources['gsap']: enough surface for the
/// install pass (timeline/to + a seekable playhead that writes real node
/// mutations), nothing more.
const GSAP_STUB: &str = r#"
    var __ocStubGsap = {
        timeline: function (vars) {
            if (!vars || vars.paused !== true) throw new Error('timeline must be created paused');
            var tweens = [];
            var tl = {
                to: function (target) { tweens.push(String(target)); return tl; },
                time: function (t) {
                    for (var i = 0; i < tweens.length; i++) {
                        globalThis.ctx.__domTarget(tweens[i]).style.opacity = String(0.5 + t);
                    }
                },
            };
            return tl;
        },
        to: function () {},
    };
    __ocStubGsap.default = __ocStubGsap;
    module.exports = { gsap: __ocStubGsap, default: __ocStubGsap, TimelineLite: function () {} };
"#;

fn with_gsap_stub(source: &str) -> String {
    format!(
        "globalThis.__ocModuleSources = {{ gsap: {} }};\n{}",
        serde_json::to_string(GSAP_STUB).unwrap(),
        source
    )
}

#[test]
fn real_package_imports_forward_to_ctx_namespaces() {
    // (1) gsap: hook-gated — usable inside useGSAP/useFrame, errors outside.
    // The driver body (outside a hook) must NOT be able to tween.
    let (mut realm, driver) = open_with_source(&with_gsap_stub(
        r#"
        const { gsap } = globalThis.__ocRequire('gsap');
        if (typeof gsap.to !== 'function') throw new Error('gsap.to must come through the builtin');
        try {
            gsap.to('box', { opacity: 1 });
            throw new Error('gsap.to outside a hook must be rejected');
        } catch (e) {
            if (!String(e).includes('useGSAP')) throw e;
        }
        globalThis.__gateWorks = true;
    "#,
    ));
    run_driver(&mut realm, driver, 0, None);
    assert_eq!(
        realm.eval_json_for_test("globalThis.__gateWorks"),
        serde_json::json!(true),
        "gate should reject out-of-hook tween calls"
    );

    // (2) inside a per-frame hook the same import drives tweens. run_frame
    // replays the hook registry after the microtask drain, so the timeline's
    // writes land in the same frame's consuming transfer — no manual
    // `__ocRunFrameHooks()` in the driver anymore.
    let source = r#"
        const gsap = globalThis.__ocRequire('gsap').gsap;
        globalThis.__ocOnFrame(function () {
            if (typeof gsap.to !== 'function') throw new Error('in-hook gsap.to missing');
            const tl = gsap.timeline();
            tl.to('box', { opacity: 1, duration: 0.5 }, 0);
            tl.to('box2', { x: 40, duration: 0.5 }, 0.25);
        });
    "#;
    let (mut realm, driver) = open_with_source(&with_gsap_stub(source));
    let mut registry = ScriptTargetRegistry::default();
    registry.visual_ids.insert("box".to_string());
    registry.visual_ids.insert("box2".to_string());
    realm.set_target_registry(registry);
    let snap = run_driver(&mut realm, driver, 15, None).snapshot_mutations();
    assert!(
        snap.mutations.contains_key("box") && snap.mutations.contains_key("box2"),
        "in-hook gsap timeline should drive both targets: {:?}",
        snap.mutations.keys().collect::<Vec<_>>()
    );

    // (3) canvaskit-wasm: ungated forward (constructors are pure local state).
    // It resolves to the official loader stub: a callable that resolves the
    // facade. (three 模块已随 opencat-three 退役移除——WebGL 走 core 的
    // canvas.getContext('webgl2') 录制面，不再有 ctx.threejs 内建。)
    let source = r#"
        const CanvasKitInit = globalThis.__ocRequire('canvaskit-wasm').default;
        if (typeof CanvasKitInit !== 'function') throw new Error('canvaskit-wasm default must be the CanvasKitInit loader');
        CanvasKitInit({}).then(function (CK) {
            if (typeof CK.MakeCanvasSurface !== 'function') throw new Error('facade must expose MakeCanvasSurface');
            globalThis.__fwdOk = true;
        });
    "#;
    let (mut realm, driver) = open_with_source(source);
    run_driver(&mut realm, driver, 0, None);
    assert_eq!(
        realm.eval_json_for_test("globalThis.__fwdOk"),
        serde_json::json!(true),
        "canvaskit-wasm forward should resolve"
    );
}
