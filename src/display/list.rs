use crate::{
    assets::AssetId,
    layout::tree::LayoutRect,
    style::{BackgroundFill, ColorToken, ComputedTextStyle, ObjectFit, ShadowStyle},
};

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
    ApplyTransform { transform: DisplayTransform },
    Draw { item: DisplayItem },
}

#[derive(Clone, Debug)]
pub struct DisplayLayer {
    pub bounds: LayoutRect,
    pub opacity: f32,
}

#[derive(Clone, Debug)]
pub struct DisplayTransform {
    pub translation_x: f32,
    pub translation_y: f32,
    pub bounds: LayoutRect,
    pub transforms: Vec<crate::style::Transform>,
}

#[derive(Clone, Debug)]
pub enum DisplayItem {
    Rect(RectDisplayItem),
    Text(TextDisplayItem),
    Bitmap(BitmapDisplayItem),
    Lucide(LucideDisplayItem),
}

#[derive(Clone, Debug)]
pub struct RectDisplayItem {
    pub bounds: LayoutRect,
    pub paint: RectPaintStyle,
}

#[derive(Clone, Debug)]
pub struct TextDisplayItem {
    pub bounds: LayoutRect,
    pub text: String,
    pub style: ComputedTextStyle,
    pub allow_wrap: bool,
}

#[derive(Clone, Debug)]
pub struct BitmapDisplayItem {
    pub bounds: LayoutRect,
    pub asset_id: AssetId,
    pub width: u32,
    pub height: u32,
    pub object_fit: ObjectFit,
    pub paint: BitmapPaintStyle,
}

#[derive(Clone, Debug)]
pub struct RectPaintStyle {
    pub background: Option<BackgroundFill>,
    pub border_radius: f32,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub shadow: Option<ShadowStyle>,
}

#[derive(Clone, Debug)]
pub struct BitmapPaintStyle {
    pub background: Option<BackgroundFill>,
    pub border_radius: f32,
    pub border_width: Option<f32>,
    pub border_color: Option<ColorToken>,
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
    pub bounds: LayoutRect,
    pub icon: String,
    pub paint: LucidePaintStyle,
}
