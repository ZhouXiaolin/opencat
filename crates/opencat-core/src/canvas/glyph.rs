#[derive(Clone, Debug)]
pub struct GlyphRunSpec<'a> {
    pub font_id: u32,
    pub font_size: f32,
    pub font_scale_x: f32,
    pub font_skew_x: f32,
    pub edging: FontEdging,
    pub subpixel: bool,
    pub glyph_ids: &'a [u16],
    pub positions: &'a [f32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontEdging {
    Alias,
    AntiAlias,
    SubpixelAntiAlias,
}
