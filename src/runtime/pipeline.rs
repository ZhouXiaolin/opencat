use std::time::Instant;

use anyhow::Result;
use skia_safe::Canvas;

use crate::{
    display::{
        analysis::display_list_contains_video,
        build::{build_display_list_from_tree, build_display_tree},
        list::DisplayList,
        tree::DisplayTree,
    },
    element::resolve::resolve_ui_tree_with_script_cache,
    frame_ctx::{FrameCtx, ScriptFrameCtx},
    runtime::{
        policy::{
            cache::SceneSlot,
            snapshot::{SceneSnapshotRuntime, plan_for_scene, render_scene_slot},
        },
        preflight::ensure_assets_preloaded,
        profile::{BackendProfile, FrameProfile, SceneBuildStats},
        session::RenderSession,
    },
    scene::{
        composition::Composition,
        node::Node,
        script::StyleMutations,
        time::{FrameState, frame_state_for_root},
        transition::TransitionKind,
    },
};

pub(crate) fn render_frame_on_canvas(
    composition: &Composition,
    frame_index: u32,
    session: &mut RenderSession,
    canvas: &Canvas,
) -> Result<()> {
    ensure_assets_preloaded(composition, session)?;

    let mut frame_profile = FrameProfile::default();
    let frame_ctx = FrameCtx {
        frame: frame_index,
        fps: composition.fps,
        width: composition.width,
        height: composition.height,
        frames: composition.frames,
    };

    let script_started = Instant::now();
    let mutations: Option<StyleMutations> = None;
    frame_profile.script_ms = script_started.elapsed().as_secs_f64() * 1000.0;

    let root = composition.root_node(&frame_ctx);
    let frame_state_started = Instant::now();
    let frame_state = frame_state_for_root(&root, &frame_ctx);
    frame_profile.frame_state_ms = frame_state_started.elapsed().as_secs_f64() * 1000.0;

    match frame_state {
        FrameState::Scene {
            scene,
            script_frame_ctx,
        } => {
            let (display_tree, display_list, scene_stats) = build_scene_display_list_with_slot(
                &scene,
                &frame_ctx,
                &script_frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::Scene,
            )?;
            frame_profile.merge_scene_stats(&scene_stats);
            let snapshot_plan = plan_for_scene(&scene_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            let render_engine = session.render_engine_handle();
            {
                let mut snapshot_runtime = SceneSnapshotRuntime {
                    assets: &session.assets,
                    scene_snapshots: &mut session.scene_snapshots,
                    backend_resources: &session.backend_resources,
                    media_ctx: &mut session.media_ctx,
                    frame_ctx: &frame_ctx,
                    backend_profile: &mut backend_profile,
                    render_engine,
                    width: composition.width,
                    height: composition.height,
                };
                render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::Scene,
                    &display_tree,
                    &display_list,
                    snapshot_plan,
                    false,
                    Some(canvas),
                )?;
            }

            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.merge_backend_profile(&backend_profile);
        }
        FrameState::Transition {
            from,
            to,
            from_script_frame_ctx,
            to_script_frame_ctx,
            progress,
            kind,
        } => {
            let (from_tree, from_display, from_stats) = build_scene_display_list_with_slot(
                &from,
                &frame_ctx,
                &from_script_frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionFrom,
            )?;
            let (to_tree, to_display, to_stats) = build_scene_display_list_with_slot(
                &to,
                &frame_ctx,
                &to_script_frame_ctx,
                session,
                mutations.as_ref(),
                SceneSlot::TransitionTo,
            )?;
            frame_profile.merge_scene_stats(&from_stats);
            frame_profile.merge_scene_stats(&to_stats);
            let from_plan = plan_for_scene(&from_stats);
            let to_plan = plan_for_scene(&to_stats);

            let backend_started = Instant::now();
            let mut backend_profile = BackendProfile::default();
            let render_engine = session.render_engine_handle();
            let (from_snapshot, to_snapshot) = {
                let mut snapshot_runtime = SceneSnapshotRuntime {
                    assets: &session.assets,
                    scene_snapshots: &mut session.scene_snapshots,
                    backend_resources: &session.backend_resources,
                    media_ctx: &mut session.media_ctx,
                    frame_ctx: &frame_ctx,
                    backend_profile: &mut backend_profile,
                    render_engine,
                    width: composition.width,
                    height: composition.height,
                };
                let from_snapshot = render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::TransitionFrom,
                    &from_tree,
                    &from_display,
                    from_plan,
                    true,
                    None,
                )?
                .expect("transition source scene snapshot should exist");
                let to_snapshot = render_scene_slot(
                    &mut snapshot_runtime,
                    SceneSlot::TransitionTo,
                    &to_tree,
                    &to_display,
                    to_plan,
                    true,
                    None,
                )?
                .expect("transition target scene snapshot should exist");
                (from_snapshot, to_snapshot)
            };
            frame_profile.backend_ms = backend_started.elapsed().as_secs_f64() * 1000.0;

            let transition_started = Instant::now();
            session.render_engine_handle().draw_transition(
                canvas,
                &from_snapshot,
                &to_snapshot,
                progress,
                kind,
                composition.width,
                composition.height,
                Some(&mut backend_profile),
            )?;
            let transition_ms = transition_started.elapsed().as_secs_f64() * 1000.0;
            frame_profile.transition_ms = transition_ms;
            frame_profile.merge_backend_profile(&backend_profile);
            match kind {
                TransitionKind::Slide(_) => {
                    frame_profile.slide_transition_ms = transition_ms;
                    frame_profile.slide_transition_frames = 1;
                }
                TransitionKind::LightLeak(_) => {
                    frame_profile.light_leak_transition_ms = transition_ms;
                    frame_profile.light_leak_transition_frames = 1;
                }
                _ => {}
            }
        }
    }

    session.profiler.push(frame_profile);
    Ok(())
}

pub(crate) fn build_scene_display_list_with_slot(
    scene: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    session: &mut RenderSession,
    mutations: Option<&StyleMutations>,
    slot: SceneSlot,
) -> Result<(DisplayTree, DisplayList, SceneBuildStats)> {
    let mut stats = SceneBuildStats::default();

    let resolve_started = Instant::now();
    let element_root = resolve_ui_tree_with_script_cache(
        scene,
        frame_ctx,
        script_frame_ctx,
        &mut session.media_ctx,
        &mut session.assets,
        mutations,
        &mut session.script_runtime,
    )?;
    stats.resolve_ms = resolve_started.elapsed().as_secs_f64() * 1000.0;

    let layout_started = Instant::now();
    let text_engine = session.text_engine_handle();
    let (layout_tree, layout_pass) = session
        .layout_session_mut(slot)
        .compute_layout_with_text_engine(&element_root, frame_ctx, text_engine.as_ref())?;
    stats.layout_ms = layout_started.elapsed().as_secs_f64() * 1000.0;
    stats.layout_pass = layout_pass;

    let display_started = Instant::now();
    let display_tree = build_display_tree(&element_root, &layout_tree)?;
    let display_list = build_display_list_from_tree(&display_tree);
    stats.display_ms = display_started.elapsed().as_secs_f64() * 1000.0;
    stats.contains_video = display_list_contains_video(&display_list, &session.assets);

    Ok((display_tree, display_list, stats))
}
