use crate::{
    FrameCtx,
    nodes::div,
    style::NodeStyle,
    transitions::TransitionKind,
    view::{Node, NodeKind},
};

#[derive(Clone)]
pub struct TimelineNode {
    segments: Vec<TimelineSegment>,
    duration_in_frames: u32,
    pub(crate) style: NodeStyle,
}

impl TimelineNode {
    pub(crate) fn new(segments: Vec<TimelineSegment>, duration_in_frames: u32) -> Self {
        Self {
            segments,
            duration_in_frames,
            style: NodeStyle::default(),
        }
    }

    pub(crate) fn segments(&self) -> &[TimelineSegment] {
        &self.segments
    }

    pub(crate) fn duration_in_frames(&self) -> u32 {
        self.duration_in_frames
    }

    pub(crate) fn style_ref(&self) -> &NodeStyle {
        &self.style
    }
}

#[derive(Clone)]
pub(crate) enum TimelineSegment {
    Scene {
        start_frame: u32,
        duration_in_frames: u32,
        scene: Node,
    },
    Transition {
        start_frame: u32,
        duration_in_frames: u32,
        from: Node,
        to: Node,
        kind: TransitionKind,
    },
}

#[derive(Clone)]
pub(crate) enum FrameState {
    Scene {
        scene: Node,
    },
    Transition {
        from: Node,
        to: Node,
        progress: f32,
        kind: TransitionKind,
    },
}

pub(crate) fn frame_state_for_root(root: &Node, ctx: &FrameCtx) -> FrameState {
    match root.kind() {
        NodeKind::Component(component) => frame_state_for_root(&component.render(ctx), ctx),
        NodeKind::Timeline(timeline) => frame_state_for_timeline(timeline, ctx),
        _ => FrameState::Scene {
            scene: root.clone(),
        },
    }
}

fn frame_state_for_timeline(timeline: &TimelineNode, ctx: &FrameCtx) -> FrameState {
    if timeline.segments().is_empty() {
        return FrameState::Scene {
            scene: div().id("__empty_timeline_scene").into(),
        };
    }

    let frame = if timeline.duration_in_frames() == 0 {
        0
    } else {
        ctx.frame.min(timeline.duration_in_frames() - 1)
    };

    for segment in timeline.segments() {
        match segment {
            TimelineSegment::Scene {
                start_frame,
                duration_in_frames,
                scene,
            } => {
                if frame < start_frame.saturating_add(*duration_in_frames) {
                    return FrameState::Scene {
                        scene: scene.clone(),
                    };
                }
            }
            TimelineSegment::Transition {
                start_frame,
                duration_in_frames,
                from,
                to,
                kind,
            } => {
                if frame < start_frame.saturating_add(*duration_in_frames) {
                    return FrameState::Transition {
                        from: from.clone(),
                        to: to.clone(),
                        progress: transition_progress(
                            frame.saturating_sub(*start_frame),
                            *duration_in_frames,
                        ),
                        kind: *kind,
                    };
                }
            }
        }
    }

    match timeline.segments().last() {
        Some(TimelineSegment::Scene { scene, .. }) => FrameState::Scene {
            scene: scene.clone(),
        },
        Some(TimelineSegment::Transition { to, .. }) => FrameState::Scene { scene: to.clone() },
        None => FrameState::Scene {
            scene: div().id("__empty_timeline_scene").into(),
        },
    }
}

fn transition_progress(frame: u32, duration_in_frames: u32) -> f32 {
    if duration_in_frames == 0 {
        return 1.0;
    }

    (frame as f32 / duration_in_frames as f32).clamp(0.0, 1.0)
}
