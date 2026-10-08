//! 整合测试：core 中需要 host 真实 ScriptDriver/QuickJS 的 resolve 用例。
//! 从 `core/element/resolve.rs` 搬迁，避免 core 内出现 `host-default` feature gate。

#![cfg(test)]

use opencat_core::frame_ctx::ScriptFrameCtx;
use opencat_core::parse::easing::Easing;
use opencat_core::parse::primitives::{SrtEntry, caption, div, text};
use opencat_core::parse::time::{FrameState, frame_state_for_root};
use opencat_core::parse::transition::{slide, timeline};
use opencat_core::resolve::resolve::{resolve_ui_tree, resolve_ui_tree_with_script_cache};
use opencat_core::resolve::tree::ElementKind;
use opencat_core::probe::catalog::PreparedResourceCatalog;

use opencat_core::frame_ctx::FrameCtx;

use opencat_core::script::{ScriptHost, ScriptRealm};
use crate::js_context::RqJsContext;

#[test]
fn node_script_only_affects_its_own_subtree() {
    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 320,
        height: 180,
        frames: 1,
    };
    let mut assets = PreparedResourceCatalog::default();

    let scene = div()
        .id("root")
        .child(
            div()
                .id("animated")
                .script_source(r#"ctx.getNode("title").opacity(0.25);"#)
                .expect("script should compile")
                .child(text("A").id("title")),
        )
        .child(div().id("static").child(text("B").id("title")));

    let mut script_runtime = ScriptRealm::<RqJsContext>::open().expect("script realm");
    let resolved = resolve_ui_tree(
        &scene.into(),
        &frame_ctx,
        &mut assets,
        None,
        &mut script_runtime,
    )
    .expect("tree should resolve");

    assert_eq!(resolved.children[0].children[0].style.visual.opacity, 0.25);
    assert_eq!(resolved.children[1].children[0].style.visual.opacity, 1.0);
}

#[test]
fn transition_scenes_keep_node_scripts_isolated() {
    let frame_ctx = FrameCtx {
        frame: 10,
        fps: 30,
        width: 320,
        height: 180,
        frames: 30,
    };
    let mut assets = PreparedResourceCatalog::default();
    let mut script_runtime = ScriptRealm::<RqJsContext>::open().expect("script realm");

    let from_scene = div()
        .id("scene-a")
        .script_source(r#"ctx.getNode("title").opacity(0.2);"#)
        .expect("script should compile")
        .child(text("From").id("title"));
    let to_scene = div()
        .id("scene-b")
        .script_source(r#"ctx.getNode("title").opacity(0.8);"#)
        .expect("script should compile")
        .child(text("To").id("title"));
    let root = timeline()
        .sequence(10.0 / 30.0, from_scene.into())
        .transition(slide().timing(Easing::Linear, 10.0 / 30.0))
        .sequence(10.0 / 30.0, to_scene.into())
        .into();

    let FrameState::Transition {
        from,
        to,
        from_script_frame_ctx,
        to_script_frame_ctx,
        ..
    } = frame_state_for_root(&root, &frame_ctx)
    else {
        panic!("expected transition frame");
    };

    let from_resolved = resolve_ui_tree_with_script_cache(
        &from,
        &frame_ctx,
        &from_script_frame_ctx,
        &mut assets,
        None,
        &mut script_runtime,
    )
    .expect("from scene should resolve");
    let to_resolved = resolve_ui_tree_with_script_cache(
        &to,
        &frame_ctx,
        &to_script_frame_ctx,
        &mut assets,
        None,
        &mut script_runtime,
    )
    .expect("to scene should resolve");

    assert_eq!(from_resolved.children[0].style.visual.opacity, 0.2);
    assert_eq!(to_resolved.children[0].style.visual.opacity, 0.8);
}

#[test]
fn timeline_scripts_receive_scene_local_frames() {
    let frame_ctx = FrameCtx {
        frame: 19,
        fps: 30,
        width: 320,
        height: 180,
        frames: 60,
    };
    let mut assets = PreparedResourceCatalog::default();
    let mut script_runtime = ScriptRealm::<RqJsContext>::open().expect("script realm");

    let scene = div()
        .id("scene-b")
        .script_source(
            r#"var local = Math.abs(ctx.currentTime - 4 / 30) < 1e-9;
var duration = Math.abs(ctx.sceneDuration - 10 / 30) < 1e-9;
ctx.getNode("title").opacity(local && duration ? 0.6 : 0.1);"#,
        )
        .expect("script should compile")
        .child(text("B").id("title"));
    let root = timeline()
        .sequence(
            10.0 / 30.0,
            div().id("scene-a").child(text("A").id("a-title")).into(),
        )
        .transition(slide().timing(Easing::Linear, 5.0 / 30.0))
        .sequence(10.0 / 30.0, scene.into())
        .into();

    let FrameState::Scene {
        scene,
        script_frame_ctx,
    } = frame_state_for_root(&root, &frame_ctx)
    else {
        panic!("expected scene frame");
    };

    let resolved = resolve_ui_tree_with_script_cache(
        &scene,
        &frame_ctx,
        &script_frame_ctx,
        &mut assets,
        None,
        &mut script_runtime,
    )
    .expect("scene should resolve");

    assert_eq!(
        script_frame_ctx,
        ScriptFrameCtx::for_segment(&frame_ctx, 15, 10)
    );
    assert_eq!(resolved.children[0].style.visual.opacity, 0.6);
}

#[test]
fn timeline_set_applies_from_position_and_ignores_gsap_control_fields() {
    fn opacity_at(frame: u32) -> f32 {
        let frame_ctx = FrameCtx {
            frame,
            fps: 30,
            width: 320,
            height: 180,
            frames: 60,
        };
        let mut assets = PreparedResourceCatalog::default();
        let mut script_runtime = ScriptRealm::<RqJsContext>::open().expect("script realm");

        let root = div()
            .id("root")
            .script_source(
                r#"
                var tl = ctx.timeline();
                tl.set("title", { opacity: 0.25, visibility: "hidden", overwrite: "auto" }, 0.5);
            "#,
            )
            .expect("script should compile")
            .child(text("A").id("title"));

        let resolved = resolve_ui_tree(
            &root.into(),
            &frame_ctx,
            &mut assets,
            None,
            &mut script_runtime,
        )
        .expect("tree should resolve");

        resolved.children[0].style.visual.opacity
    }

    assert_eq!(opacity_at(14), 1.0);
    assert_eq!(opacity_at(15), 0.25);
}

#[test]
fn timeline_tween_times_are_not_rescaled_to_fit_scene_duration() {
    // Regression: a tween whose end overruns the composition duration must keep
    // its absolute GSAP times. Previously `flush()` scaled every timing by
    // `sceneDuration / totalEnd`, so a single late tween (e.g. a flash ending at
    // 12.16s in a 12.0s comp) played the *whole* timeline ~1.3% fast, shifting
    // every keyframe by one or more frames.
    fn inner_x_at(frame: u32) -> f32 {
        let frame_ctx = FrameCtx {
            frame,
            fps: 30,
            width: 400,
            height: 100,
            frames: 360,
        };
        let mut assets = PreparedResourceCatalog::default();
        let mut script_runtime = ScriptRealm::<RqJsContext>::open().expect("script realm");

        // Linear ramp x: 0 -> 300px over 1.0..3.0s, plus a no-op set ending at
        // 24.0s (= 2× the 12.0s composition duration). Without the rescale, at
        // frame 45 (t=1.5s) progress is (1.5-1.0)/2.0 = 0.25 -> x = 75. With the
        // old rescale (scale = 12.0/24.0 = 0.5) the delay/duration halve and the
        // ramp already completes, so x would read 300 — an unmistakable drift.
        let root = div()
            .id("root")
            .script_source(
                r#"
                var tl = ctx.timeline();
                tl.fromTo("box", { x: 0 }, { x: 300, duration: 2.0, ease: "none" }, 1.0);
                tl.set("box", { opacity: 1 }, 24.0);
            "#,
            )
            .expect("script should compile")
            .child(div().id("box"));

        let resolved = resolve_ui_tree(
            &root.into(),
            &frame_ctx,
            &mut assets,
            None,
            &mut script_runtime,
        )
        .expect("tree should resolve");
        let transforms = &resolved.children[0].style.visual.transforms;
        transforms
            .iter()
            .find_map(|t| match t {
                opencat_core::style::Transform::TranslateX { value } => Some(*value),
                _ => None,
            })
            .unwrap_or(0.0)
    }

    // t = 45/30 = 1.5s -> quarter of the 1.0..3.0 ramp -> x = 300*0.25 = 75.
    let x = inner_x_at(45);
    assert!(
        (x - 75.0).abs() < 1.0,
        "timeline tween was rescaled to fit the scene: x={x} (expected ~75)"
    );
}

#[test]
fn parent_script_can_split_descendant_text_before_child_resolution() {
    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 320,
        height: 180,
        frames: 1,
    };
    let mut assets = PreparedResourceCatalog::default();

    let root = div()
        .id("root")
        .script_source(
            r#"
            var parts = ctx.splitText("title", { type: "chars" });
            parts[0].set({ opacity: 0.2 });
        "#,
        )
        .expect("script should compile")
        .child(text("Hello").id("title"));

    let mut script_runtime = ScriptRealm::<RqJsContext>::open().expect("script realm");
    let resolved = resolve_ui_tree(
        &root.into(),
        &frame_ctx,
        &mut assets,
        None,
        &mut script_runtime,
    )
    .expect("parent script should see descendant text source");

    let ElementKind::Text(text) = &resolved.children[0].kind else {
        panic!("child should resolve to text");
    };
    let batch = text
        .text_unit_overrides
        .as_ref()
        .expect("text unit overrides should exist");
    assert_eq!(batch.overrides[0].opacity, Some(0.2));
}

#[test]
fn resolve_caption_uses_scene_local_time_inside_timeline() {
    let caption_node = caption().id("subs").path("sub.srt").entries(vec![
        SrtEntry {
            index: 1,
            start_frame: 0,
            end_frame: 5,
            text: "Local A".into(),
        },
        SrtEntry {
            index: 2,
            start_frame: 5,
            end_frame: 10,
            text: "Local B".into(),
        },
    ]);
    let root = timeline()
        .sequence(
            10.0 / 30.0,
            div().id("scene-a").child(text("A").id("t")).into(),
        )
        .sequence(10.0 / 30.0, div().id("scene-b").child(caption_node).into())
        .into();
    let frame_ctx = FrameCtx {
        frame: 17,
        fps: 30,
        width: 320,
        height: 180,
        frames: 20,
    };
    let mut assets = PreparedResourceCatalog::default();
    let mut runtime = ScriptRealm::<RqJsContext>::open().expect("script realm");

    let FrameState::Scene {
        scene,
        script_frame_ctx,
    } = frame_state_for_root(&root, &frame_ctx)
    else {
        panic!("expected scene frame");
    };

    let tree = resolve_ui_tree_with_script_cache(
        &scene,
        &frame_ctx,
        &script_frame_ctx,
        &mut assets,
        None,
        &mut runtime,
    )
    .expect("caption tree should resolve");

    assert!(format!("{tree:?}").contains("Local B"));
}
