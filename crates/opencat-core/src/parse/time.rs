use crate::{
    frame_ctx::ScriptFrameCtx,
    frame_ctx::{FrameCtx, duration_secs_to_frames, frames_to_duration_secs},
    parse::{
        easing::Easing,
        node::{Node, NodeKind},
        primitives::div,
        transition::TransitionKind,
    },
    style::NodeStyle,
};

#[derive(Clone)]
pub struct TimelineNode {
    segments: Vec<TimelineSegment>,
    duration_secs: f64,
    pub style: NodeStyle,
}

impl TimelineNode {
    pub fn new(segments: Vec<TimelineSegment>, duration_secs: f64) -> Self {
        Self {
            segments,
            duration_secs,
            style: NodeStyle::default(),
        }
    }

    pub fn segments(&self) -> &[TimelineSegment] {
        &self.segments
    }

    pub fn duration_secs(&self) -> f64 {
        self.duration_secs
    }

    pub fn duration_in_frames(&self, ctx: &FrameCtx) -> u32 {
        self.segments
            .iter()
            .map(|segment| duration_secs_to_frames(segment.duration_secs(), ctx.fps))
            .sum()
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

#[derive(Clone)]
pub enum TimelineSegment {
    Scene {
        start_secs: f64,
        duration_secs: f64,
        scene: Node,
    },
    Transition {
        start_secs: f64,
        duration_secs: f64,
        from: Node,
        to: Node,
        from_duration_secs: f64,
        to_duration_secs: f64,
        kind: TransitionKind,
        easing: Easing,
    },
}

impl TimelineSegment {
    pub fn start_secs(&self) -> f64 {
        match self {
            TimelineSegment::Scene { start_secs, .. }
            | TimelineSegment::Transition { start_secs, .. } => *start_secs,
        }
    }

    pub fn duration_secs(&self) -> f64 {
        match self {
            TimelineSegment::Scene { duration_secs, .. }
            | TimelineSegment::Transition { duration_secs, .. } => *duration_secs,
        }
    }
}

#[derive(Clone)]
pub enum FrameState {
    Scene {
        scene: Node,
        script_frame_ctx: ScriptFrameCtx,
    },
    Transition {
        from: Node,
        to: Node,
        from_script_frame_ctx: ScriptFrameCtx,
        to_script_frame_ctx: ScriptFrameCtx,
        progress: f32,
        kind: TransitionKind,
    },
}

pub fn frame_state_for_root(root: &Node, ctx: &FrameCtx) -> FrameState {
    match root.kind() {
        NodeKind::Timeline(timeline) => frame_state_for_timeline(timeline, ctx),
        NodeKind::Div(_) => FrameState::Scene {
            scene: root.clone(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        },
        _ => FrameState::Scene {
            scene: root.clone(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        },
    }
}

fn frame_state_for_timeline(timeline: &TimelineNode, ctx: &FrameCtx) -> FrameState {
    if timeline.segments().is_empty() {
        return FrameState::Scene {
            scene: div().id("__empty_timeline_scene").into(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        };
    }

    let duration_in_frames = timeline.duration_in_frames(ctx);
    let frame = if duration_in_frames == 0 {
        0
    } else {
        ctx.frame.min(duration_in_frames - 1)
    };

    let mut cursor_frame: u32 = 0;
    for segment in timeline.segments() {
        let segment_frames = duration_secs_to_frames(segment.duration_secs(), ctx.fps);
        match segment {
            TimelineSegment::Scene {
                duration_secs: _,
                scene,
                ..
            } => {
                if frame < cursor_frame.saturating_add(segment_frames) {
                    return FrameState::Scene {
                        scene: scene.clone(),
                        script_frame_ctx: ScriptFrameCtx::for_segment(
                            ctx,
                            cursor_frame,
                            segment_frames,
                        ),
                    };
                }
            }
            TimelineSegment::Transition {
                from,
                to,
                from_duration_secs,
                to_duration_secs,
                kind,
                easing,
                ..
            } => {
                if frame < cursor_frame.saturating_add(segment_frames) {
                    let from_duration_frames =
                        duration_secs_to_frames(*from_duration_secs, ctx.fps);
                    let to_duration_frames = duration_secs_to_frames(*to_duration_secs, ctx.fps);
                    return FrameState::Transition {
                        from: from.clone(),
                        to: to.clone(),
                        from_script_frame_ctx: frozen_script_frame_ctx(
                            ctx,
                            from_duration_frames.saturating_sub(1),
                            from_duration_frames,
                        ),
                        to_script_frame_ctx: frozen_script_frame_ctx(ctx, 0, to_duration_frames),
                        progress: transition_progress(
                            frame.saturating_sub(cursor_frame),
                            segment_frames,
                            easing,
                        ),
                        kind: kind.clone(),
                    };
                }
            }
        }
        cursor_frame = cursor_frame.saturating_add(segment_frames);
    }

    match timeline.segments().last() {
        Some(TimelineSegment::Scene { scene, .. }) => FrameState::Scene {
            scene: scene.clone(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        },
        Some(TimelineSegment::Transition {
            to,
            to_duration_secs,
            ..
        }) => FrameState::Scene {
            scene: to.clone(),
            script_frame_ctx: frozen_script_frame_ctx(
                ctx,
                0,
                duration_secs_to_frames(*to_duration_secs, ctx.fps),
            ),
        },
        None => FrameState::Scene {
            scene: div().id("__empty_timeline_scene").into(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        },
    }
}

fn frozen_script_frame_ctx(
    ctx: &FrameCtx,
    current_frame: u32,
    scene_frames: u32,
) -> ScriptFrameCtx {
    ScriptFrameCtx {
        frame: ctx.frame,
        fps: ctx.fps,
        total_frames: ctx.frames,
        current_frame: current_frame.min(scene_frames.saturating_sub(1)),
        scene_frames,
        time_secs: ctx.time_secs(),
        total_duration_secs: ctx.duration_secs(),
        current_time_secs: frames_to_duration_secs(
            current_frame.min(scene_frames.saturating_sub(1)),
            ctx.fps,
        ),
        scene_duration_secs: frames_to_duration_secs(scene_frames, ctx.fps),
    }
}

fn transition_progress(frame: u32, duration_in_frames: u32, easing: &Easing) -> f32 {
    if duration_in_frames == 0 {
        return 1.0;
    }

    let t = (frame as f32 / duration_in_frames as f32).clamp(0.0, 1.0);
    easing.apply(t).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::FrameState;
    use crate::{
        frame_ctx::FrameCtx,
        parse::{
            easing::Easing,
            node::NodeKind,
            primitives::{caption, div},
            transition::{slide, timeline},
        },
    };

    #[test]
    fn frame_state_uses_scene_local_progress_inside_timeline() {
        let root = timeline()
            .sequence(10.0 / 30.0, div().id("scene-a").into())
            .transition(slide().timing(Easing::Linear, 5.0 / 30.0))
            .sequence(20.0 / 30.0, div().id("scene-b").into())
            .into();
        let frame_ctx = FrameCtx {
            frame: 18,
            fps: 30,
            width: 320,
            height: 180,
            frames: 120,
        };

        let FrameState::Scene {
            script_frame_ctx, ..
        } = super::frame_state_for_root(&root, &frame_ctx)
        else {
            panic!("expected scene frame");
        };

        assert_eq!(script_frame_ctx.frame, 18);
        assert_eq!(script_frame_ctx.total_frames, 120);
        assert_eq!(script_frame_ctx.current_frame, 3);
        assert_eq!(script_frame_ctx.scene_frames, 20);
    }

    #[test]
    fn frame_state_freezes_scene_script_clocks_during_transition() {
        let root = timeline()
            .sequence(10.0 / 30.0, div().id("scene-a").into())
            .transition(slide().timing(Easing::Linear, 6.0 / 30.0))
            .sequence(20.0 / 30.0, div().id("scene-b").into())
            .into();
        let frame_ctx = FrameCtx {
            frame: 13,
            fps: 30,
            width: 320,
            height: 180,
            frames: 120,
        };

        let FrameState::Transition {
            from_script_frame_ctx,
            to_script_frame_ctx,
            ..
        } = super::frame_state_for_root(&root, &frame_ctx)
        else {
            panic!("expected transition frame");
        };

        assert_eq!(from_script_frame_ctx.frame, 13);
        assert_eq!(from_script_frame_ctx.total_frames, 120);
        assert_eq!(from_script_frame_ctx.current_frame, 9);
        assert_eq!(from_script_frame_ctx.scene_frames, 10);
        assert_eq!(to_script_frame_ctx.frame, 13);
        assert_eq!(to_script_frame_ctx.total_frames, 120);
        assert_eq!(to_script_frame_ctx.current_frame, 0);
        assert_eq!(to_script_frame_ctx.scene_frames, 20);
    }

    #[test]
    fn frame_state_handles_div_root_with_timeline_and_caption_siblings() {
        let root = div()
            .id("root")
            .child(
                timeline()
                    .sequence(10.0 / 30.0, div().id("scene-a").into())
                    .transition(slide().timing(Easing::Linear, 5.0 / 30.0))
                    .sequence(10.0 / 30.0, div().id("scene-b").into()),
            )
            .child(caption().id("subs").path("sub.srt").entries(vec![]));

        let frame_ctx = FrameCtx {
            frame: 12,
            fps: 30,
            width: 320,
            height: 180,
            frames: 25,
        };

        let state = super::frame_state_for_root(&root.into(), &frame_ctx);
        let FrameState::Scene { scene, .. } = state else {
            panic!("root div should still resolve as scene");
        };
        let NodeKind::Div(scene_div) = scene.kind() else {
            panic!("scene should remain a div");
        };
        assert_eq!(scene_div.children_ref().len(), 2);
    }

    #[test]
    fn frame_state_keeps_single_timeline_child_root_as_scene() {
        let root = div().id("root").child(
            timeline()
                .sequence(10.0 / 30.0, div().id("scene-a").into())
                .transition(slide().timing(Easing::Linear, 5.0 / 30.0))
                .sequence(10.0 / 30.0, div().id("scene-b").into()),
        );

        let frame_ctx = FrameCtx {
            frame: 12,
            fps: 30,
            width: 320,
            height: 180,
            frames: 25,
        };

        let state = super::frame_state_for_root(&root.into(), &frame_ctx);
        let FrameState::Scene { scene, .. } = state else {
            panic!("single timeline child root should remain a scene");
        };
        let NodeKind::Div(scene_div) = scene.kind() else {
            panic!("scene should remain a div");
        };
        assert_eq!(scene_div.children_ref().len(), 1);
    }
}
