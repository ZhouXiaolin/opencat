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
    /// Used by host-driven caption hydration, which must recurse into the
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

#[cfg(test)]
mod tests {
    use super::FrameState;
    use crate::{
        frame_ctx::FrameCtx,
        parse::{
            easing::Easing,
            node::{Node, NodeKind},
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
        // scene-b is on screen for the transition it shares with scene-a plus
        // its own 20 frames: its window starts at frame 10 and runs 25 frames.
        assert_eq!(script_frame_ctx.current_frame, 8);
        assert_eq!(script_frame_ctx.scene_frames, 25);
    }

    #[test]
    fn frame_state_keeps_scene_clocks_live_during_transition() {
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

        // Both scenes are live on the transition frame. `from` runs from frame
        // 0 (its window widened by the following transition, 10 + 6 frames);
        // `to` runs from frame 10 (its own window, 6 + 20 frames). Neither clock
        // is pinned to a boundary frame.
        assert_eq!(from_script_frame_ctx.frame, 13);
        assert_eq!(from_script_frame_ctx.total_frames, 120);
        assert_eq!(from_script_frame_ctx.current_frame, 13);
        assert_eq!(from_script_frame_ctx.scene_frames, 16);
        assert_eq!(to_script_frame_ctx.frame, 13);
        assert_eq!(to_script_frame_ctx.total_frames, 120);
        assert_eq!(to_script_frame_ctx.current_frame, 3);
        assert_eq!(to_script_frame_ctx.scene_frames, 26);
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
    fn nested_timeline_places_its_segments_from_the_enclosing_scene() {
        // Outer timeline: scene "wrap" runs from frame 30 (1s) for 60 frames.
        // Inside it, a nested timeline whose own scenes are 15 and 45 frames.
        let inner: Node = timeline()
            .sequence(15.0 / 30.0, div().id("inner-a").into())
            .sequence(45.0 / 30.0, div().id("inner-b").into())
            .into();
        let outer = timeline()
            .sequence(30.0 / 30.0, div().id("head").into())
            .sequence(60.0 / 30.0, div().id("wrap").child(inner).into())
            .into();

        let frame_ctx = FrameCtx {
            frame: 40,
            fps: 30,
            width: 320,
            height: 180,
            frames: 120,
        };

        // Frame 40 is 10 frames into "wrap" (which starts at frame 30).
        let FrameState::Scene {
            scene,
            script_frame_ctx,
        } = super::frame_state_for_root(&outer, &frame_ctx)
        else {
            panic!("expected the wrap scene");
        };
        assert_eq!(scene.style_ref().id, "wrap");
        assert_eq!(script_frame_ctx.current_frame, 10);
        assert_eq!(script_frame_ctx.scene_start_frame, 30);

        // The nested timeline reads its origin off that scene clock: at local
        // frame 10 it is still inside "inner-a" (0-15), not "inner-b".
        let origin = super::TimelineOrigin::of_scene(&script_frame_ctx);
        assert_eq!(origin.frame, 30);
        assert_eq!(origin.secs, 1.0);

        // The resolver reaches the nested timeline by walking into the scene
        // div, so resolve the div first and then the timeline it owns.
        let NodeKind::Div(wrap) = scene.kind() else {
            panic!("wrap should be a div");
        };
        let inner_tl = wrap.children_ref().first().expect("inner timeline");

        let FrameState::Scene { scene, .. } =
            super::frame_state_for_root_at(inner_tl, &frame_ctx, origin)
        else {
            panic!("expected a nested scene");
        };
        assert_eq!(scene.style_ref().id, "inner-a");
    }

    #[test]
    fn nested_timeline_scene_clock_is_relative_to_its_own_segment() {
        let inner: Node = timeline()
            .sequence(15.0 / 30.0, div().id("inner-a").into())
            .sequence(45.0 / 30.0, div().id("inner-b").into())
            .into();
        let outer = timeline()
            .sequence(30.0 / 30.0, div().id("head").into())
            .sequence(60.0 / 30.0, div().id("wrap").child(inner).into())
            .into();

        // Frame 50 is 20 frames into "wrap", i.e. 5 frames into "inner-b"
        // (which starts 15 frames into the nested timeline).
        let frame_ctx = FrameCtx {
            frame: 50,
            fps: 30,
            width: 320,
            height: 180,
            frames: 120,
        };

        let FrameState::Scene {
            scene,
            script_frame_ctx,
        } = super::frame_state_for_root(&outer, &frame_ctx)
        else {
            panic!("expected the wrap scene");
        };
        let origin = super::TimelineOrigin::of_scene(&script_frame_ctx);

        let NodeKind::Div(wrap) = scene.kind() else {
            panic!("wrap should be a div");
        };
        let inner_tl = wrap.children_ref().first().expect("inner timeline");

        let FrameState::Scene {
            scene,
            script_frame_ctx: inner_ctx,
        } = super::frame_state_for_root_at(inner_tl, &frame_ctx, origin)
        else {
            panic!("expected a nested scene");
        };

        assert_eq!(scene.style_ref().id, "inner-b");
        // The nested scene's clock is its own lifetime: it starts at frame 45
        // on the composition clock (30 + 15), so frame 50 reads 5 frames in.
        assert_eq!(inner_ctx.current_frame, 5);
        assert_eq!(inner_ctx.scene_start_frame, 45);
        assert_eq!(inner_ctx.scene_start_secs, 1.5);
    }

    #[test]
    fn top_level_timeline_ignores_the_default_origin() {
        let root = timeline()
            .sequence(15.0 / 30.0, div().id("scene-a").into())
            .sequence(45.0 / 30.0, div().id("scene-b").into())
            .into();
        let frame_ctx = FrameCtx {
            frame: 20,
            fps: 30,
            width: 320,
            height: 180,
            frames: 120,
        };

        // The two-argument entry point is the top-level one: origin zero.
        let FrameState::Scene {
            scene,
            script_frame_ctx,
        } = super::frame_state_for_root(&root, &frame_ctx)
        else {
            panic!("expected a scene");
        };
        assert_eq!(scene.style_ref().id, "scene-b");
        assert_eq!(script_frame_ctx.scene_start_frame, 15);
        assert_eq!(script_frame_ctx.current_frame, 5);
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
