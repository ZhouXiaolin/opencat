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

    /// Absolute frame index each segment starts on, plus one final entry for
    /// the frame the timeline ends on. Entry `i` is the start of segment `i`;
    /// entry `len` is the end of the last segment.
    ///
    /// A boundary is `ceil(running_secs * fps)` of the seconds cursor — NOT the
    /// sum of per-segment `ceil(duration * fps)`. Those differ whenever a
    /// duration lands between frames, and the difference compounds: each such
    /// boundary drifts one frame later, so a long timeline ends up seconds
    /// behind where the author placed it.
    pub fn segment_frame_bounds(&self, ctx: &FrameCtx) -> Vec<u32> {
        let mut bounds = Vec::with_capacity(self.segments.len() + 1);
        let mut cursor_secs = 0.0;
        for segment in &self.segments {
            bounds.push(duration_secs_to_frames(cursor_secs, ctx.fps));
            cursor_secs += segment.duration_secs();
        }
        bounds.push(duration_secs_to_frames(cursor_secs, ctx.fps));
        bounds
    }

    pub fn duration_in_frames(&self, ctx: &FrameCtx) -> u32 {
        self.segment_frame_bounds(ctx).last().copied().unwrap_or(0)
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    /// Walk every scene / transition endpoint and replace it via `f`.
    ///
    /// Used by host-driven script injection, which must recurse into the
    /// scene nodes a timeline owns. Timing/metadata are preserved; only the
    /// owned [`Node`]s are rewritten. `f` may return `Err` to abort the walk.
    pub(crate) fn map_scene_nodes(
        &mut self,
        mut f: impl FnMut(Node) -> anyhow::Result<Node>,
    ) -> anyhow::Result<()> {
        for segment in &mut self.segments {
            match segment {
                TimelineSegment::Scene { scene, .. } => {
                    *scene = f(std::mem::replace(scene, clone_placeholder()))?;
                }
                TimelineSegment::Transition { from, to, .. } => {
                    *from = f(std::mem::replace(from, clone_placeholder()))?;
                    *to = f(std::mem::replace(to, clone_placeholder()))?;
                }
            }
        }
        Ok(())
    }
}

/// Cheap throwaway node used only to swap a real node out of a segment field so
/// it can be moved into `f` without contending the borrow of `self.segments`.
fn clone_placeholder() -> Node {
    div().into()
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
    frame_state_for_root_at(root, ctx, TimelineOrigin::default())
}

/// Where a timeline starts on the composition clock.
///
/// A top-level timeline starts at zero. A timeline nested inside a scene starts
/// where that scene does: its own segments are then selected by local time, and
/// each of its scenes runs a clock from its own segment start. That is what
/// makes `ctx.current.time` a component's own lifetime however deeply it is
/// nested, instead of the composition's elapsed time.
#[derive(Debug, Clone, Copy, Default)]
pub struct TimelineOrigin {
    pub frame: u32,
    pub secs: f64,
}

impl TimelineOrigin {
    /// The origin of a timeline that fills the scene `script_frame_ctx` describes.
    pub fn of_scene(script_frame_ctx: &ScriptFrameCtx) -> Self {
        Self {
            frame: script_frame_ctx.scene_start_frame,
            secs: script_frame_ctx.scene_start_secs,
        }
    }
}

pub fn frame_state_for_root_at(
    root: &Node,
    ctx: &FrameCtx,
    origin: TimelineOrigin,
) -> FrameState {
    match root.kind() {
        NodeKind::Timeline(timeline) => frame_state_for_timeline(timeline, ctx, origin),
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

fn frame_state_for_timeline(
    timeline: &TimelineNode,
    ctx: &FrameCtx,
    origin: TimelineOrigin,
) -> FrameState {
    let segments = timeline.segments();
    if segments.is_empty() {
        return FrameState::Scene {
            scene: div().id("__empty_timeline_scene").into(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        };
    }

    // Frame index inside this timeline. A nested timeline is offset by the
    // enclosing scene's start, so its segments are picked by local time.
    let local_frame = ctx.frame.saturating_sub(origin.frame);
    let duration_in_frames = timeline.duration_in_frames(ctx);
    let frame = if duration_in_frames == 0 {
        0
    } else {
        local_frame.min(duration_in_frames - 1)
    };

    // Scenes and transitions tile the timeline end to end, in the order the
    // author wrote them. A transition is the OVERLAP between its two scenes, so
    // each scene is on screen for its own segment plus the transition segments
    // on either side. Both neighbours of a transition stay live for its whole
    // span, and each runs its own clock from its own window start — nothing is
    // frozen at a boundary frame.
    //
    // Boundaries come from `segment_frame_bounds`: each is the ceiling of the
    // running seconds cursor, so segment `i` occupies `[bounds[i], bounds[i+1])`
    // and no boundary can drift from where the author placed it.
    let bounds = timeline.segment_frame_bounds(ctx);
    let seg_frames: Vec<u32> = (0..segments.len())
        .map(|index| bounds[index + 1].saturating_sub(bounds[index]))
        .collect();
    let seg_start = &bounds[..segments.len()];

    // Exact (unquantized) start of each segment on the composition clock.
    let mut seg_start_secs = Vec::with_capacity(segments.len());
    let mut cursor_secs = 0.0;
    for segment in segments {
        seg_start_secs.push(cursor_secs);
        cursor_secs += segment.duration_secs();
    }

    // Visible window of the scene at `index`: its segment widened by the
    // transition segments that touch it. Returns `(start_frame, start_secs,
    // frames)` on the COMPOSITION clock — a nested timeline shifts its windows
    // by its origin, so the scene clock it hands a script is still a real
    // composition timestamp. The seconds start is kept exact so the scene clock
    // does not inherit the frame quantization of the boundary.
    let scene_window = |index: usize| -> (u32, f64, u32) {
        let before = if index > 0 && matches!(segments[index - 1], TimelineSegment::Transition { .. })
        {
            (seg_frames[index - 1], segments[index - 1].duration_secs())
        } else {
            (0, 0.0)
        };
        let after = if index + 1 < segments.len()
            && matches!(segments[index + 1], TimelineSegment::Transition { .. })
        {
            (seg_frames[index + 1], segments[index + 1].duration_secs())
        } else {
            (0, 0.0)
        };
        (
            origin.frame + seg_start[index].saturating_sub(before.0),
            (origin.secs + seg_start_secs[index] - before.1).max(0.0),
            before.0 + seg_frames[index] + after.0,
        )
    };

    for (index, segment) in segments.iter().enumerate() {
        if frame >= seg_start[index] + seg_frames[index] {
            continue;
        }
        match segment {
            TimelineSegment::Scene { scene, .. } => {
                let (window_start, window_start_secs, window_frames) = scene_window(index);
                return FrameState::Scene {
                    scene: scene.clone(),
                    script_frame_ctx: ScriptFrameCtx::for_window(
                        ctx,
                        window_start,
                        window_start_secs,
                        window_frames,
                    ),
                };
            }
            TimelineSegment::Transition {
                from,
                to,
                kind,
                easing,
                ..
            } => {
                let (from_start, from_start_secs, from_frames) = scene_window(index - 1);
                let (to_start, to_start_secs, to_frames) = scene_window(index + 1);
                return FrameState::Transition {
                    from: from.clone(),
                    to: to.clone(),
                    from_script_frame_ctx: ScriptFrameCtx::for_window(
                        ctx,
                        from_start,
                        from_start_secs,
                        from_frames,
                    ),
                    to_script_frame_ctx: ScriptFrameCtx::for_window(
                        ctx,
                        to_start,
                        to_start_secs,
                        to_frames,
                    ),
                    progress: transition_progress(
                        frame.saturating_sub(seg_start[index]),
                        seg_frames[index],
                        easing,
                    ),
                    kind: kind.clone(),
                };
            }
        }
    }

    // Past the last segment: hold its scene at the end of its own window.
    match segments.last() {
        Some(TimelineSegment::Scene { scene, .. }) => {
            let (window_start, window_start_secs, window_frames) =
                scene_window(segments.len() - 1);
            FrameState::Scene {
                scene: scene.clone(),
                script_frame_ctx: ScriptFrameCtx::for_window(
                    ctx,
                    window_start,
                    window_start_secs,
                    window_frames,
                ),
            }
        }
        Some(TimelineSegment::Transition { to, .. }) => {
            let (to_start, to_start_secs, to_frames) = scene_window(segments.len() - 1);
            FrameState::Scene {
                scene: to.clone(),
                script_frame_ctx: ScriptFrameCtx::for_window(
                    ctx,
                    to_start,
                    to_start_secs,
                    to_frames,
                ),
            }
        }
        None => FrameState::Scene {
            scene: div().id("__empty_timeline_scene").into(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        },
    }
}

fn transition_progress(frame: u32, duration_in_frames: u32, easing: &Easing) -> f32 {
    if duration_in_frames == 0 {
        return 1.0;
    }

    let t = (frame as f32 / duration_in_frames as f32).clamp(0.0, 1.0);
    easing.apply(t).clamp(0.0, 1.0)
}


