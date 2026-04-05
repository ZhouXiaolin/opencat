use crate::{
    Node,
    timeline::{TimelineNode, TimelineSegment},
};

#[derive(Clone, Copy, Debug)]
pub enum TransitionKind {
    Slide,
    LightLeak(LightLeakTransition),
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

    fn into_timeline(self) -> TimelineNode {
        let duration_in_frames = self.duration_in_frames();
        let items = self.items;
        let mut segments = Vec::new();
        let mut cursor = 0;

        for index in 0..items.len() {
            let TransitionSeriesItem::Sequence {
                duration_in_frames,
                node,
            } = &items[index]
            else {
                continue;
            };

            segments.push(TimelineSegment::Scene {
                start_frame: cursor,
                duration_in_frames: *duration_in_frames,
                scene: node.clone(),
            });
            cursor += *duration_in_frames;

            if let (
                Some(TransitionSeriesItem::Transition(transition)),
                Some(TransitionSeriesItem::Sequence {
                    node: next_node, ..
                }),
            ) = (items.get(index + 1), items.get(index + 2))
            {
                let transition_duration = transition.duration_in_frames();
                segments.push(TimelineSegment::Transition {
                    start_frame: cursor,
                    duration_in_frames: transition_duration,
                    from: node.clone(),
                    to: next_node.clone(),
                    kind: transition.kind(),
                });
                cursor += transition_duration;
            }
        }

        TimelineNode::new(segments, duration_in_frames)
    }
}

impl Default for TransitionSeries {
    fn default() -> Self {
        transition_series()
    }
}

impl From<TransitionSeries> for Node {
    fn from(series: TransitionSeries) -> Self {
        Node::from(series.into_timeline())
    }
}

impl Transition {
    fn duration_in_frames(&self) -> u32 {
        match self.timing {
            Timing::Linear { duration_in_frames } => duration_in_frames,
        }
    }

    fn kind(&self) -> TransitionKind {
        match self.presentation {
            Presentation::Slide => TransitionKind::Slide,
            Presentation::LightLeak(params) => TransitionKind::LightLeak(params),
        }
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
