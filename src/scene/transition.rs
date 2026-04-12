use crate::scene::{
    easing::{Easing, SpringConfig},
    node::Node,
    time::{TimelineNode, TimelineSegment},
};

#[derive(Clone, Copy, Debug)]
pub enum TransitionKind {
    Slide(SlideDirection),
    LightLeak(LightLeakTransition),
    Fade,
    Wipe(WipeDirection),
    ClockWipe,
    Iris,
}

#[derive(Clone, Copy, Debug)]
pub enum SlideDirection {
    FromLeft,
    FromRight,
    FromTop,
    FromBottom,
}

#[derive(Clone, Copy, Debug)]
pub enum WipeDirection {
    FromLeft,
    FromTopLeft,
    FromTop,
    FromTopRight,
    FromRight,
    FromBottomRight,
    FromBottom,
    FromBottomLeft,
}

#[derive(Clone)]
pub struct Timeline {
    items: Vec<TimelineItem>,
}

#[derive(Clone)]
enum TimelineItem {
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
    Slide(SlideDirection),
    LightLeak(LightLeakTransition),
    Fade,
    Wipe(WipeDirection),
    ClockWipe,
    Iris,
}

#[derive(Clone, Copy, Debug)]
pub struct LightLeakTransition {
    pub seed: f32,
    pub hue_shift: f32,
    pub mask_scale: f32,
}

#[derive(Clone, Copy)]
pub struct LightLeakBuilder {
    seed: f32,
    hue_shift: f32,
    mask_scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum Timing {
    Linear {
        duration_in_frames: u32,
    },
    Spring {
        config: SpringConfig,
        duration_in_frames: u32,
    },
}

impl Timing {
    pub(crate) fn easing(&self) -> Easing {
        match self {
            Timing::Linear { .. } => Easing::Linear,
            Timing::Spring { config, .. } => Easing::Spring(*config),
        }
    }

    pub(crate) fn duration_in_frames(&self) -> u32 {
        match self {
            Timing::Linear { duration_in_frames } => *duration_in_frames,
            Timing::Spring {
                duration_in_frames, ..
            } => *duration_in_frames,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SlideBuilder {
    direction: SlideDirection,
}

#[derive(Clone, Copy)]
pub struct FadeBuilder;

#[derive(Clone, Copy)]
pub struct WipeBuilder {
    direction: WipeDirection,
}

#[derive(Clone, Copy)]
pub struct ClockWipeBuilder;

#[derive(Clone, Copy)]
pub struct IrisBuilder;

#[derive(Clone, Copy)]
pub struct LinearTimingBuilder;

#[derive(Clone, Copy)]
pub struct SpringTimingBuilder {
    config: SpringConfig,
}

impl Timeline {
    pub fn sequence(mut self, duration_in_frames: u32, node: Node) -> Self {
        self.items.push(TimelineItem::Sequence {
            duration_in_frames,
            node,
        });
        self
    }

    pub fn transition(mut self, transition: Transition) -> Self {
        self.items.push(TimelineItem::Transition(transition));
        self
    }

    pub fn duration_in_frames(&self) -> u32 {
        self.items
            .iter()
            .map(|item| match item {
                TimelineItem::Sequence {
                    duration_in_frames, ..
                } => *duration_in_frames,
                TimelineItem::Transition(transition) => transition.duration_in_frames(),
            })
            .sum()
    }

    fn into_timeline(self) -> TimelineNode {
        let duration_in_frames = self.duration_in_frames();
        let items = self.items;
        let mut segments = Vec::new();
        let mut cursor = 0;

        for index in 0..items.len() {
            let TimelineItem::Sequence {
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
                Some(TimelineItem::Transition(transition)),
                Some(TimelineItem::Sequence {
                    duration_in_frames: next_duration_in_frames,
                    node: next_node,
                    ..
                }),
            ) = (items.get(index + 1), items.get(index + 2))
            {
                let transition_duration = transition.duration_in_frames();
                segments.push(TimelineSegment::Transition {
                    start_frame: cursor,
                    duration_in_frames: transition_duration,
                    from: node.clone(),
                    to: next_node.clone(),
                    from_duration_in_frames: *duration_in_frames,
                    to_duration_in_frames: *next_duration_in_frames,
                    kind: transition.kind(),
                    timing: transition.timing,
                });
                cursor += transition_duration;
            }
        }

        TimelineNode::new(segments, duration_in_frames)
    }
}

impl Default for Timeline {
    fn default() -> Self {
        timeline()
    }
}

impl From<Timeline> for Node {
    fn from(series: Timeline) -> Self {
        Node::from(series.into_timeline())
    }
}

impl Transition {
    fn duration_in_frames(&self) -> u32 {
        self.timing.duration_in_frames()
    }

    fn kind(&self) -> TransitionKind {
        match self.presentation {
            Presentation::Slide(dir) => TransitionKind::Slide(dir),
            Presentation::LightLeak(params) => TransitionKind::LightLeak(params),
            Presentation::Fade => TransitionKind::Fade,
            Presentation::Wipe(dir) => TransitionKind::Wipe(dir),
            Presentation::ClockWipe => TransitionKind::ClockWipe,
            Presentation::Iris => TransitionKind::Iris,
        }
    }
}

impl SlideBuilder {
    pub fn from_left(self) -> Self {
        self.direction(SlideDirection::FromLeft)
    }

    pub fn from_right(self) -> Self {
        self.direction(SlideDirection::FromRight)
    }

    pub fn from_top(self) -> Self {
        self.direction(SlideDirection::FromTop)
    }

    pub fn from_bottom(self) -> Self {
        self.direction(SlideDirection::FromBottom)
    }

    fn direction(mut self, direction: SlideDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::Slide(self.direction),
            timing,
        }
    }
}

impl FadeBuilder {
    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::Fade,
            timing,
        }
    }
}

impl WipeBuilder {
    pub fn from_left(self) -> Self {
        self.direction(WipeDirection::FromLeft)
    }

    pub fn from_right(self) -> Self {
        self.direction(WipeDirection::FromRight)
    }

    pub fn from_top(self) -> Self {
        self.direction(WipeDirection::FromTop)
    }

    pub fn from_bottom(self) -> Self {
        self.direction(WipeDirection::FromBottom)
    }

    pub fn from_top_left(self) -> Self {
        self.direction(WipeDirection::FromTopLeft)
    }

    pub fn from_top_right(self) -> Self {
        self.direction(WipeDirection::FromTopRight)
    }

    pub fn from_bottom_left(self) -> Self {
        self.direction(WipeDirection::FromBottomLeft)
    }

    pub fn from_bottom_right(self) -> Self {
        self.direction(WipeDirection::FromBottomRight)
    }

    fn direction(mut self, direction: WipeDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::Wipe(self.direction),
            timing,
        }
    }
}

impl ClockWipeBuilder {
    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::ClockWipe,
            timing,
        }
    }
}

impl IrisBuilder {
    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::Iris,
            timing,
        }
    }
}

impl LightLeakBuilder {
    pub fn seed(mut self, seed: f32) -> Self {
        self.seed = seed;
        self
    }

    pub fn hue_shift(mut self, hue_shift: f32) -> Self {
        self.hue_shift = hue_shift;
        self
    }

    pub fn mask_scale(mut self, mask_scale: f32) -> Self {
        self.mask_scale = mask_scale.clamp(0.03125, 1.0);
        self
    }

    pub fn timing(self, timing: Timing) -> Transition {
        Transition {
            presentation: Presentation::LightLeak(LightLeakTransition {
                seed: self.seed,
                hue_shift: self.hue_shift,
                mask_scale: self.mask_scale,
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

impl SpringTimingBuilder {
    pub fn damping(mut self, damping: f32) -> Self {
        self.config.damping = damping;
        self
    }

    pub fn stiffness(mut self, stiffness: f32) -> Self {
        self.config.stiffness = stiffness;
        self
    }

    pub fn mass(mut self, mass: f32) -> Self {
        self.config.mass = mass;
        self
    }

    pub fn duration(self, duration_in_frames: u32) -> Timing {
        Timing::Spring {
            config: self.config,
            duration_in_frames,
        }
    }
}

pub fn slide() -> SlideBuilder {
    SlideBuilder {
        direction: SlideDirection::FromLeft,
    }
}

pub fn fade() -> FadeBuilder {
    FadeBuilder
}

pub fn wipe() -> WipeBuilder {
    WipeBuilder {
        direction: WipeDirection::FromLeft,
    }
}

pub fn clock_wipe() -> ClockWipeBuilder {
    ClockWipeBuilder
}

pub fn iris() -> IrisBuilder {
    IrisBuilder
}

pub fn light_leak() -> LightLeakBuilder {
    LightLeakBuilder {
        seed: 0.0,
        hue_shift: 0.0,
        mask_scale: 0.25,
    }
}

pub fn linear() -> LinearTimingBuilder {
    LinearTimingBuilder
}

pub fn spring() -> SpringTimingBuilder {
    SpringTimingBuilder {
        config: SpringConfig::default(),
    }
}

pub fn timeline() -> Timeline {
    Timeline { items: Vec::new() }
}
