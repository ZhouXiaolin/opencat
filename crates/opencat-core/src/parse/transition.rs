use crate::parse::{
    easing::Easing,
    node::Node,
    time::{TimelineNode, TimelineSegment},
};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TransitionKind {
    Slide(SlideDirection),
    LightLeak(LightLeakTransition),
    Gl(GlTransition),
    Fade,
    Wipe(WipeDirection),
    ClockWipe,
    Iris,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlideDirection {
    FromLeft,
    FromRight,
    FromTop,
    FromBottom,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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
    Sequence { duration_secs: f64, node: Node },
    Transition(Transition),
}

#[derive(Clone)]
pub struct Transition {
    presentation: Presentation,
    easing: Easing,
    duration_secs: f64,
}

#[derive(Clone)]
enum Presentation {
    Slide(SlideDirection),
    LightLeak(LightLeakTransition),
    Gl(GlTransition),
    Fade,
    Wipe(WipeDirection),
    ClockWipe,
    Iris,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightLeakTransition {
    pub seed: f32,
    pub hue_shift: f32,
    pub mask_scale: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlTransition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sksl: Option<String>,
}

impl GlTransition {
    /// Fill the `sksl` field using the GLSL-to-SKSL conversion in core.
    pub fn fill_sksl(&mut self) {
        if self.sksl.is_none() {
            self.sksl = crate::parse::gl_transition::gl_transition_sksl(&self.name).ok();
        }
    }
}

#[derive(Clone, Copy)]
pub struct LightLeakBuilder {
    seed: f32,
    hue_shift: f32,
    mask_scale: f32,
}

#[derive(Clone)]
pub struct GlTransitionBuilder {
    name: String,
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

impl Timeline {
    pub fn sequence(mut self, duration_secs: f64, node: Node) -> Self {
        self.items.push(TimelineItem::Sequence {
            duration_secs: sanitize_duration_secs(duration_secs),
            node,
        });
        self
    }

    pub fn transition(mut self, transition: Transition) -> Self {
        self.items.push(TimelineItem::Transition(transition));
        self
    }

    pub fn duration_secs(&self) -> f64 {
        self.items
            .iter()
            .map(|item| match item {
                TimelineItem::Sequence { duration_secs, .. } => *duration_secs,
                TimelineItem::Transition(transition) => transition.duration_secs(),
            })
            .sum()
    }

    fn into_timeline(self) -> TimelineNode {
        let duration_secs = self.duration_secs();
        let items = self.items;
        let mut segments = Vec::new();
        let mut cursor_secs = 0.0;

        for index in 0..items.len() {
            let TimelineItem::Sequence {
                duration_secs,
                node,
            } = &items[index]
            else {
                continue;
            };

            segments.push(TimelineSegment::Scene {
                start_secs: cursor_secs,
                duration_secs: *duration_secs,
                scene: node.clone(),
            });
            cursor_secs += *duration_secs;

            if let (
                Some(TimelineItem::Transition(transition)),
                Some(TimelineItem::Sequence {
                    duration_secs: next_duration_secs,
                    node: next_node,
                    ..
                }),
            ) = (items.get(index + 1), items.get(index + 2))
            {
                let transition_duration = transition.duration_secs();
                segments.push(TimelineSegment::Transition {
                    start_secs: cursor_secs,
                    duration_secs: transition_duration,
                    from: node.clone(),
                    to: next_node.clone(),
                    from_duration_secs: *duration_secs,
                    to_duration_secs: *next_duration_secs,
                    kind: transition.kind(),
                    easing: transition.easing,
                });
                cursor_secs += transition_duration;
            }
        }

        TimelineNode::new(segments, duration_secs)
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
    pub(crate) fn duration_secs(&self) -> f64 {
        self.duration_secs
    }

    fn kind(&self) -> TransitionKind {
        match &self.presentation {
            Presentation::Slide(dir) => TransitionKind::Slide(*dir),
            Presentation::LightLeak(params) => TransitionKind::LightLeak(*params),
            Presentation::Gl(effect) => TransitionKind::Gl(effect.clone()),
            Presentation::Fade => TransitionKind::Fade,
            Presentation::Wipe(dir) => TransitionKind::Wipe(*dir),
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

    pub fn timing(self, easing: Easing, duration_secs: f64) -> Transition {
        Transition {
            presentation: Presentation::Slide(self.direction),
            easing,
            duration_secs: sanitize_duration_secs(duration_secs),
        }
    }
}

impl FadeBuilder {
    pub fn timing(self, easing: Easing, duration_secs: f64) -> Transition {
        Transition {
            presentation: Presentation::Fade,
            easing,
            duration_secs: sanitize_duration_secs(duration_secs),
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

    pub fn timing(self, easing: Easing, duration_secs: f64) -> Transition {
        Transition {
            presentation: Presentation::Wipe(self.direction),
            easing,
            duration_secs: sanitize_duration_secs(duration_secs),
        }
    }
}

impl ClockWipeBuilder {
    pub fn timing(self, easing: Easing, duration_secs: f64) -> Transition {
        Transition {
            presentation: Presentation::ClockWipe,
            easing,
            duration_secs: sanitize_duration_secs(duration_secs),
        }
    }
}

impl IrisBuilder {
    pub fn timing(self, easing: Easing, duration_secs: f64) -> Transition {
        Transition {
            presentation: Presentation::Iris,
            easing,
            duration_secs: sanitize_duration_secs(duration_secs),
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

    pub fn timing(self, easing: Easing, duration_secs: f64) -> Transition {
        Transition {
            presentation: Presentation::LightLeak(LightLeakTransition {
                seed: self.seed,
                hue_shift: self.hue_shift,
                mask_scale: self.mask_scale,
            }),
            easing,
            duration_secs: sanitize_duration_secs(duration_secs),
        }
    }
}

impl GlTransitionBuilder {
    pub fn timing(self, easing: Easing, duration_secs: f64) -> Transition {
        Transition {
            presentation: Presentation::Gl(GlTransition {
                name: self.name,
                sksl: None,
            }),
            easing,
            duration_secs: sanitize_duration_secs(duration_secs),
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

pub fn gl_transition(name: impl Into<String>) -> GlTransitionBuilder {
    GlTransitionBuilder { name: name.into() }
}

pub fn timeline() -> Timeline {
    Timeline { items: Vec::new() }
}

fn sanitize_duration_secs(duration_secs: f64) -> f64 {
    if duration_secs.is_finite() && duration_secs > 0.0 {
        duration_secs
    } else {
        0.0
    }
}
