use crate::{
    resource::assets::AssetId,
    resource::media::VideoFrameTiming,
    core::scene::script::{CanvasCommand, TextUnitOverrideBatch},
    core::scene::transition::TransitionKind,
    core::style::{
        BackgroundFill, BorderRadius, BorderStyle, BoxShadow, ColorToken, ComputedTextStyle,
        DropShadow, InsetShadow, ObjectFit,
    },
};

#[derive(Clone, Copy, Debug)]
pub struct DisplayRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct DisplayClip {
    pub bounds: DisplayRect,
    pub border_radius: BorderRadius,
}

#[derive(Clone, Debug)]
pub struct DisplayTransform {
    pub translation_x: f32,
    pub translation_y: f32,
    pub bounds: DisplayRect,
    pub transforms: Vec<crate::core::style::Transform>,
}

#[derive(Clone, Debug)]
pub enum DisplayItem {
    Rect(RectDisplayItem),
    Timeline(TimelineDisplayItem),
    Text(TextDisplayItem),
    Bitmap(BitmapDisplayItem),
    DrawScript(DrawScriptDisplayItem),
    SvgPath(SvgPathDisplayItem),
}

#[derive(Clone, Debug)]
pub struct RectDisplayItem {
    pub bounds: DisplayRect,
    pub paint: RectPaintStyle,
}

#[derive(Clone, Debug)]
pub struct TextDisplayItem {
    pub bounds: DisplayRect,
    pub text: String,
    pub style: ComputedTextStyle,
    pub allow_wrap: bool,
    pub truncate: bool,
    pub drop_shadow: Option<DropShadow>,
    pub text_unit_overrides: Option<TextUnitOverrideBatch>,
    pub visual_expand_x: f32,
    pub visual_expand_y: f32,
}

#[derive(Clone, Debug)]
pub struct TimelineDisplayItem {
    pub bounds: DisplayRect,
    pub paint: RectPaintStyle,
    pub transition: Option<TimelineTransitionDisplay>,
}

#[derive(Clone, Debug)]
pub struct TimelineTransitionDisplay {
    pub progress: f32,
    pub kind: TransitionKind,
}

#[derive(Clone, Debug)]
pub struct BitmapDisplayItem {
    pub bounds: DisplayRect,
    pub asset_id: AssetId,
    pub width: u32,
    pub height: u32,
    pub video_timing: Option<VideoFrameTiming>,
    pub object_fit: ObjectFit,
    pub paint: BitmapPaintStyle,
}

#[derive(Clone, Debug)]
pub struct DrawScriptDisplayItem {
    pub bounds: DisplayRect,
    pub commands: Vec<CanvasCommand>,
    pub drop_shadow: Option<DropShadow>,
}

#[derive(Clone, Debug)]
pub struct RectPaintStyle {
    pub background: Option<BackgroundFill>,
    pub border_radius: BorderRadius,
    pub border_width: Option<f32>,
    pub border_top_width: Option<f32>,
    pub border_right_width: Option<f32>,
    pub border_bottom_width: Option<f32>,
    pub border_left_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub border_style: Option<BorderStyle>,
    pub blur_sigma: Option<f32>,
    pub box_shadow: Option<BoxShadow>,
    pub inset_shadow: Option<InsetShadow>,
    pub drop_shadow: Option<DropShadow>,
}

#[derive(Clone, Debug)]
pub struct BitmapPaintStyle {
    pub background: Option<BackgroundFill>,
    pub border_radius: BorderRadius,
    pub border_width: Option<f32>,
    pub border_top_width: Option<f32>,
    pub border_right_width: Option<f32>,
    pub border_bottom_width: Option<f32>,
    pub border_left_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub border_style: Option<BorderStyle>,
    pub blur_sigma: Option<f32>,
    pub box_shadow: Option<BoxShadow>,
    pub inset_shadow: Option<InsetShadow>,
    pub drop_shadow: Option<DropShadow>,
}

#[derive(Clone, Debug)]
pub struct SvgPathPaintStyle {
    pub fill: Option<BackgroundFill>,
    pub stroke_width: Option<f32>,
    pub stroke_color: Option<ColorToken>,
    pub drop_shadow: Option<DropShadow>,
    pub stroke_dasharray: Option<f32>,
    pub stroke_dashoffset: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct SvgPathDisplayItem {
    pub bounds: DisplayRect,
    pub path_data: Vec<String>,
    pub paint: SvgPathPaintStyle,
    pub view_box: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct PictureSemantics {
    pub record_bounds: DisplayRect,
    pub record_translation_x: f32,
    pub record_translation_y: f32,
    pub draw_translation_x: f32,
    pub draw_translation_y: f32,
}

impl DisplayRect {
    pub fn outset(self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            x: self.x - left,
            y: self.y - top,
            width: self.width + left + right,
            height: self.height + top + bottom,
        }
    }

    pub fn translate(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
    }

    pub fn union(self, other: Self) -> Self {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }
}

impl DisplayItem {
    pub fn bounds(&self) -> DisplayRect {
        match self {
            Self::Rect(rect) => rect.bounds,
            Self::Timeline(timeline) => timeline.bounds,
            Self::Text(text) => text.bounds,
            Self::Bitmap(bitmap) => bitmap.bounds,
            Self::DrawScript(script) => script.bounds,
            Self::SvgPath(svg) => svg.bounds,
        }
    }

    pub fn visual_bounds(&self) -> DisplayRect {
        let bounds = match self {
            Self::Text(text) => text.bounds.outset(
                text.visual_expand_x,
                text.visual_expand_y,
                text.visual_expand_x,
                text.visual_expand_y,
            ),
            _ => self.bounds(),
        };
        let mut visual_bounds = bounds;

        let box_shadow = match self {
            Self::Rect(rect) => rect.paint.box_shadow,
            Self::Timeline(timeline) => timeline.paint.box_shadow,
            Self::Bitmap(bitmap) => bitmap.paint.box_shadow,
            Self::Text(_) | Self::DrawScript(_) | Self::SvgPath(_) => None,
        };
        if let Some(shadow) = box_shadow {
            let (left, top, right, bottom) = shadow.outsets();
            visual_bounds = visual_bounds.union(bounds.outset(left, top, right, bottom));
        }

        let drop_shadow = match self {
            Self::Rect(rect) => rect.paint.drop_shadow,
            Self::Timeline(timeline) => timeline.paint.drop_shadow,
            Self::Text(text) => text.drop_shadow,
            Self::Bitmap(bitmap) => bitmap.paint.drop_shadow,
            Self::DrawScript(script) => script.drop_shadow,
            Self::SvgPath(svg) => svg.paint.drop_shadow,
        };
        if let Some(shadow) = drop_shadow {
            let (left, top, right, bottom) = shadow.outsets();
            visual_bounds = visual_bounds.union(bounds.outset(left, top, right, bottom));
        }

        visual_bounds
    }

    pub fn picture_semantics(&self) -> PictureSemantics {
        let visual_bounds = self.visual_bounds();
        PictureSemantics {
            record_bounds: DisplayRect {
                x: 0.0,
                y: 0.0,
                width: visual_bounds.width.max(1.0),
                height: visual_bounds.height.max(1.0),
            },
            record_translation_x: -visual_bounds.x,
            record_translation_y: -visual_bounds.y,
            draw_translation_x: visual_bounds.x,
            draw_translation_y: visual_bounds.y,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DisplayItem, DisplayRect, TextDisplayItem};
    use crate::{
        core::scene::script::{TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch},
        core::style::ComputedTextStyle,
    };

    #[test]
    fn text_visual_bounds_expand_for_unit_overrides() {
        let item = DisplayItem::Text(TextDisplayItem {
            bounds: DisplayRect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 24.0,
            },
            text: "Hello".into(),
            style: ComputedTextStyle::default(),
            allow_wrap: false,
            truncate: false,
            drop_shadow: None,
            text_unit_overrides: Some(TextUnitOverrideBatch {
                granularity: TextUnitGranularity::Grapheme,
                overrides: vec![TextUnitOverride {
                    translate_y: Some(-12.0),
                    ..Default::default()
                }],
            }),
            visual_expand_x: 0.0,
            visual_expand_y: 12.0,
        });

        let visual = item.visual_bounds();
        assert!(visual.height > item.bounds().height);
        assert!(visual.y < item.bounds().y);
    }
}
