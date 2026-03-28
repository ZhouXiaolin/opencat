use crate::{FrameCtx, Node, component_node_with_duration, nodes::div};

#[derive(Clone, Copy, Debug)]
pub enum TransitionKind {
    Slide,
    LightLeak(LightLeakTransition),
}

#[derive(Clone)]
pub struct TransitionNode {
    from: Node,
    to: Node,
    progress: f32,
    kind: TransitionKind,
    style: crate::style::NodeStyle,
}

impl TransitionNode {
    pub fn new(from: Node, to: Node, progress: f32, kind: TransitionKind) -> Self {
        Self {
            from,
            to,
            progress,
            kind,
            style: crate::style::NodeStyle::default(),
        }
    }

    pub fn from_node(&self) -> &Node {
        &self.from
    }

    pub fn to_node(&self) -> &Node {
        &self.to
    }

    pub fn params(&self) -> (f32, TransitionKind) {
        (self.progress, self.kind)
    }

    pub fn style_ref(&self) -> &crate::style::NodeStyle {
        &self.style
    }
}

#[derive(Clone)]
pub struct TransitionSeries {
    items: Vec<TransitionSeriesItem>,
}

#[derive(Clone)]
enum TransitionSeriesItem {
    Sequence { duration_in_frames: u32, node: Node },
    Transition(Transition),
}

#[derive(Clone)]
pub struct Transition {
    presentation: Presentation,
    timing: Timing,
}

#[derive(Clone, Copy)]
enum Presentation {
    Slide,
    LightLeak(LightLeakTransition),
}

#[derive(Clone, Copy, Debug)]
pub struct LightLeakTransition {
    pub seed: f32,
    pub retract_seed: f32,
    pub hue_shift: f32,
}

#[derive(Clone, Copy)]
pub struct LightLeakBuilder {
    seed: f32,
    retract_seed: f32,
    hue_shift: f32,
}

#[derive(Clone, Copy)]
pub enum Timing {
    Linear { duration_in_frames: u32 },
}

#[derive(Clone, Copy)]
pub struct SlideBuilder;

#[derive(Clone, Copy)]
pub struct LinearTimingBuilder;

impl TransitionSeries {
    pub fn sequence(mut self, duration_in_frames: u32, node: Node) -> Self {
        self.items.push(TransitionSeriesItem::Sequence {
            duration_in_frames,
            node,
        });
        self
    }

    pub fn transition(mut self, transition: Transition) -> Self {
        self.items
            .push(TransitionSeriesItem::Transition(transition));
        self
    }

    pub fn duration_in_frames(&self) -> u32 {
        self.items
            .iter()
            .map(|item| match item {
                TransitionSeriesItem::Sequence {
                    duration_in_frames, ..
                } => *duration_in_frames,
                TransitionSeriesItem::Transition(transition) => transition.duration_in_frames(),
            })
            .sum()
    }

    fn render(&self, ctx: &FrameCtx) -> Node {
        if self.items.is_empty() {
            return div().into();
        }

        let mut cursor = 0;
        let mut index = 0;

        while index < self.items.len() {
            let Some(current_item) = self.items.get(index) else {
                break;
            };

            match current_item {
                TransitionSeriesItem::Sequence {
                    duration_in_frames,
                    node,
                } => {
                    let segment_end = cursor + duration_in_frames;
                    if ctx.frame < segment_end {
                        return node.clone();
                    }
                    cursor = segment_end;

                    let Some(TransitionSeriesItem::Transition(transition)) =
                        self.items.get(index + 1)
                    else {
                        index += 1;
                        continue;
                    };

                    let Some(TransitionSeriesItem::Sequence {
                        node: next_node, ..
                    }) = self.items.get(index + 2)
                    else {
                        return node.clone();
                    };

                    let transition_end = cursor + transition.duration_in_frames();
                    if ctx.frame < transition_end {
                        let local_frame = ctx.frame - cursor;
                        let progress = transition.progress(local_frame);
                        return transition.presentation.render(
                            ctx,
                            node.clone(),
                            next_node.clone(),
                            progress,
                        );
                    }

                    cursor = transition_end;
                    index += 2;
                }
                TransitionSeriesItem::Transition(_) => {
                    index += 1;
                }
            }
        }

        self.last_sequence_node().unwrap_or_else(|| div().into())
    }

    fn last_sequence_node(&self) -> Option<Node> {
        self.items.iter().rev().find_map(|item| match item {
            TransitionSeriesItem::Sequence { node, .. } => Some(node.clone()),
            TransitionSeriesItem::Transition(_) => None,
        })
    }
}

impl Default for TransitionSeries {
    fn default() -> Self {
        transition_series()
    }
}

impl From<TransitionSeries> for Node {
    fn from(series: TransitionSeries) -> Self {
        let duration_in_frames = series.duration_in_frames();
        component_node_with_duration(move |ctx| series.render(ctx), move || duration_in_frames)
    }
}

impl Transition {
    fn duration_in_frames(&self) -> u32 {
        match self.timing {
            Timing::Linear { duration_in_frames } => duration_in_frames,
        }
    }

    fn progress(&self, frame: u32) -> f32 {
        match self.timing {
            Timing::Linear { duration_in_frames } => {
                if duration_in_frames == 0 {
                    return 1.0;
                }

                (frame as f32 / duration_in_frames as f32).clamp(0.0, 1.0)
            }
        }
    }
}

impl Presentation {
    fn render(self, _ctx: &FrameCtx, from: Node, to: Node, progress: f32) -> Node {
        let kind = match self {
            Presentation::Slide => TransitionKind::Slide,
            Presentation::LightLeak(params) => TransitionKind::LightLeak(params),
        };
        TransitionNode::new(from, to, progress, kind).into()
    }
}

impl SlideBuilder {
    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::Slide,
            timing,
        }
    }
}

impl LightLeakBuilder {
    pub fn seed(mut self, seed: f32) -> Self {
        self.seed = seed;
        self
    }

    pub fn retract_seed(mut self, retract_seed: f32) -> Self {
        self.retract_seed = retract_seed;
        self
    }

    pub fn hue_shift(mut self, hue_shift: f32) -> Self {
        self.hue_shift = hue_shift;
        self
    }

    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::LightLeak(LightLeakTransition {
                seed: self.seed,
                retract_seed: self.retract_seed,
                hue_shift: self.hue_shift,
            }),
            timing,
        }
    }
}

impl LinearTimingBuilder {
    pub fn duration(self, duration_in_frames: u32) -> Timing {
        Timing::Linear { duration_in_frames }
    }
}

pub fn slide() -> SlideBuilder {
    SlideBuilder
}

pub fn light_leak() -> LightLeakBuilder {
    LightLeakBuilder {
        seed: 0.0,
        retract_seed: 1.0,
        hue_shift: 0.0,
    }
}

pub fn linear() -> LinearTimingBuilder {
    LinearTimingBuilder
}

pub fn transition_series() -> TransitionSeries {
    TransitionSeries { items: Vec::new() }
}
