use crate::{
    resource::assets::AssetId,
    scene::script::CanvasCommand,
    style::{BackgroundFill, ColorToken, ComputedTextStyle, ObjectFit, ShadowStyle},
};

#[derive(Clone, Copy, Debug)]
pub struct DisplayRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayList {
    pub commands: Vec<DisplayCommand>,
}

impl DisplayList {
    pub fn push(&mut self, command: DisplayCommand) {
        self.commands.push(command);
    }
}

#[derive(Clone, Debug)]
pub enum DisplayCommand {
    Save,
    Restore,
    SaveLayer { layer: DisplayLayer },
    Clip { clip: DisplayClip },
    ApplyTransform { transform: DisplayTransform },
    Draw { item: DisplayItem },
}

#[derive(Clone, Debug)]
pub struct DisplayLayer {
    pub bounds: DisplayRect,
    pub opacity: f32,
}

#[derive(Clone, Debug)]
pub struct DisplayClip {
    pub bounds: DisplayRect,
    pub border_radius: f32,
}

#[derive(Clone, Debug)]
pub struct DisplayTransform {
    pub translation_x: f32,
    pub translation_y: f32,
    pub bounds: DisplayRect,
    pub transforms: Vec<crate::style::Transform>,
}

#[derive(Clone, Debug)]
pub enum DisplayItem {
    Rect(RectDisplayItem),
    Text(TextDisplayItem),
    Bitmap(BitmapDisplayItem),
    Canvas(CanvasDisplayItem),
    Lucide(LucideDisplayItem),
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
}

#[derive(Clone, Debug)]
pub struct BitmapDisplayItem {
    pub bounds: DisplayRect,
    pub asset_id: AssetId,
    pub width: u32,
    pub height: u32,
    pub object_fit: ObjectFit,
    pub paint: BitmapPaintStyle,
}

#[derive(Clone, Debug)]
pub struct CanvasDisplayItem {
    pub bounds: DisplayRect,
    pub commands: Vec<CanvasCommand>,
}

#[derive(Clone, Debug)]
pub struct RectPaintStyle {
    pub background: Option<BackgroundFill>,
    pub border_radius: f32,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub blur_sigma: Option<f32>,
    pub shadow: Option<ShadowStyle>,
}

#[derive(Clone, Debug)]
pub struct BitmapPaintStyle {
    pub background: Option<BackgroundFill>,
    pub border_radius: f32,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub blur_sigma: Option<f32>,
    pub shadow: Option<ShadowStyle>,
}

#[derive(Clone, Debug)]
pub struct LucidePaintStyle {
    pub foreground: ColorToken,
    pub background: Option<BackgroundFill>,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
}

#[derive(Clone, Debug)]
pub struct LucideDisplayItem {
    pub bounds: DisplayRect,
    pub icon: String,
    pub paint: LucidePaintStyle,
}
