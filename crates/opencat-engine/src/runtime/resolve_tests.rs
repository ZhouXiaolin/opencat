//! 整合测试：core 中需要 host 真实 ScriptDriver/QuickJS 的 resolve 用例。
//! 从 `core/element/resolve.rs` 搬迁，避免 core 内出现 `host-default` feature gate。

#![cfg(test)]

use opencat_core::element::resolve::{resolve_ui_tree, resolve_ui_tree_with_script_cache};
use opencat_core::element::tree::ElementKind;
use opencat_core::frame_ctx::ScriptFrameCtx;
use opencat_core::resource::asset_catalog::AssetCatalog;
use opencat_core::scene::easing::Easing;
use crate::runtime::path_bounds::SkiaPathBounds;
use opencat_core::scene::primitives::{SrtEntry, caption, div, text};
use opencat_core::scene::time::{FrameState, frame_state_for_root};
use opencat_core::scene::transition::{slide, timeline};

use crate::FrameCtx;

use crate::script::ScriptRuntimeCache;

#[test]
fn node_script_only_affects_its_own_subtree() {
    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 320,
        height: 180,
        frames: 1,
    };
    let mut assets = AssetCatalog::new();

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

    let mut script_runtime = ScriptRuntimeCache::default();
    let resolved = resolve_ui_tree(&scene.into(), &frame_ctx, &mut assets, None, &mut script_runtime)
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
    let mut assets = AssetCatalog::new();
    let mut script_runtime = ScriptRuntimeCache::default();

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
        .sequence(10, from_scene.into())
        .transition(slide().timing(Easing::Linear, 10))
        .sequence(10, to_scene.into())
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
        &SkiaPathBounds,
    )
    .expect("from scene should resolve");
    let to_resolved = resolve_ui_tree_with_script_cache(
        &to,
        &frame_ctx,
        &to_script_frame_ctx,
        &mut assets,
        None,
        &mut script_runtime,
        &SkiaPathBounds,
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
    let mut assets = AssetCatalog::new();
    let mut script_runtime = ScriptRuntimeCache::default();

    let scene = div()
        .id("scene-b")
        .script_source(
            r#"ctx.getNode("title").opacity(ctx.currentFrame === 4 && ctx.sceneFrames === 10 ? 0.6 : 0.1);"#,
        )
        .expect("script should compile")
        .child(text("B").id("title"));
    let root = timeline()
        .sequence(
            10,
            div().id("scene-a").child(text("A").id("a-title")).into(),
        )
        .transition(slide().timing(Easing::Linear, 5))
        .sequence(10, scene.into())
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
        &SkiaPathBounds,
    )
    .expect("scene should resolve");

    assert_eq!(
        script_frame_ctx,
        ScriptFrameCtx::for_segment(&frame_ctx, 15, 10)
    );
    assert_eq!(resolved.children[0].style.visual.opacity, 0.6);
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
    let mut assets = AssetCatalog::new();

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

    let mut script_runtime = ScriptRuntimeCache::default();
    let resolved = resolve_ui_tree(&root.into(), &frame_ctx, &mut assets, None, &mut script_runtime)
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
        .sequence(10, div().id("scene-a").child(text("A").id("t")).into())
        .sequence(10, div().id("scene-b").child(caption_node).into())
        .into();
    let frame_ctx = FrameCtx {
        frame: 17,
        fps: 30,
        width: 320,
        height: 180,
        frames: 20,
    };
    let mut assets = AssetCatalog::new();
    let mut runtime = ScriptRuntimeCache::default();

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
        &SkiaPathBounds,
    )
    .expect("caption tree should resolve");

    assert!(format!("{tree:?}").contains("Local B"));
}
