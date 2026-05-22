use std::collections::HashMap;

use crate::style::{
    BorderRadius, BorderStyle, BoxShadow, ColorToken, DropShadow, FlexDirection, FontWeight,
    InsetShadow, JustifyContent, LengthPercentageAuto, ObjectFit, Position, TextAlign, Transform,
};

// ── Node style mutations ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextUnitGranularity {
    Grapheme,
    Word,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextUnitOverride {
    pub opacity: Option<f32>,
    pub translate_x: Option<f32>,
    pub translate_y: Option<f32>,
    pub scale: Option<f32>,
    pub rotation_deg: Option<f32>,
    pub color: Option<ColorToken>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextUnitOverrideBatch {
    pub granularity: TextUnitGranularity,
    pub overrides: Vec<TextUnitOverride>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStyleMutations {
    pub position: Option<Position>,
    pub inset_left: Option<f32>,
    pub inset_top: Option<f32>,
    pub inset_right: Option<f32>,
    pub inset_bottom: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub padding: Option<f32>,
    pub padding_x: Option<f32>,
    pub padding_y: Option<f32>,
    pub margin: Option<f32>,
    pub margin_x: Option<f32>,
    pub margin_y: Option<f32>,
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<crate::style::AlignItems>,
    pub gap: Option<f32>,
    pub flex_grow: Option<f32>,
    pub opacity: Option<f32>,
    pub bg_color: Option<ColorToken>,
    pub fill_color: Option<ColorToken>,
    pub stroke_color: Option<ColorToken>,
    pub stroke_width: Option<f32>,
    pub stroke_dasharray: Option<f32>,
    pub stroke_dashoffset: Option<f32>,
    pub border_radius: Option<f32>,
    pub border_width: Option<f32>,
    pub border_top_width: Option<f32>,
    pub border_right_width: Option<f32>,
    pub border_bottom_width: Option<f32>,
    pub border_left_width: Option<f32>,
    pub border_color: Option<ColorToken>,
    pub border_style: Option<BorderStyle>,
    pub object_fit: Option<ObjectFit>,
    pub transforms: Vec<Transform>,
    pub text_color: Option<ColorToken>,
    pub text_px: Option<f32>,
    pub font_weight: Option<FontWeight>,
    pub letter_spacing: Option<f32>,
    pub text_align: Option<TextAlign>,
    pub line_height: Option<f32>,
    pub box_shadow: Option<BoxShadow>,
    pub box_shadow_color: Option<ColorToken>,
    pub inset_shadow: Option<InsetShadow>,
    pub inset_shadow_color: Option<ColorToken>,
    pub drop_shadow: Option<DropShadow>,
    pub drop_shadow_color: Option<ColorToken>,
    pub text_content: Option<String>,
    pub text_unit_overrides: Option<TextUnitOverrideBatch>,
    pub svg_path: Option<String>,
}

impl NodeStyleMutations {
    pub fn apply_to(&self, style: &mut crate::style::NodeStyle) {
        if let Some(v) = self.position {
            style.position = Some(v);
        }
        if let Some(v) = self.inset_left {
            style.inset_left = Some(LengthPercentageAuto::length(v));
        }
        if let Some(v) = self.inset_top {
            style.inset_top = Some(LengthPercentageAuto::length(v));
        }
        if let Some(v) = self.inset_right {
            style.inset_right = Some(LengthPercentageAuto::length(v));
        }
        if let Some(v) = self.inset_bottom {
            style.inset_bottom = Some(LengthPercentageAuto::length(v));
        }
        if let Some(v) = self.width {
            style.width = Some(v);
            style.width_full = false;
        }
        if let Some(v) = self.height {
            style.height = Some(v);
            style.height_full = false;
        }
        if let Some(v) = self.padding {
            style.padding = Some(v);
        }
        if let Some(v) = self.padding_x {
            style.padding_x = Some(v);
        }
        if let Some(v) = self.padding_y {
            style.padding_y = Some(v);
        }
        if let Some(v) = self.margin {
            style.margin = Some(LengthPercentageAuto::Length(v));
        }
        if let Some(v) = self.margin_x {
            style.margin_x = Some(LengthPercentageAuto::Length(v));
        }
        if let Some(v) = self.margin_y {
            style.margin_y = Some(LengthPercentageAuto::Length(v));
        }
        if let Some(v) = self.flex_direction {
            style.flex_direction = Some(v);
        }
        if let Some(v) = self.justify_content {
            style.justify_content = Some(v);
        }
        if let Some(v) = self.align_items {
            style.align_items = Some(v);
        }
        if let Some(v) = self.gap {
            style.gap = Some(v);
        }
        if let Some(v) = self.flex_grow {
            style.flex_grow = Some(v);
        }
        if let Some(v) = self.opacity {
            style.opacity = Some(v.clamp(0.0, 1.0));
        }
        if let Some(v) = self.bg_color {
            style.bg_color = Some(v);
        }
        if let Some(v) = self.fill_color {
            style.fill_color = Some(v);
        }
        if let Some(v) = self.stroke_color {
            style.stroke_color = Some(v);
        }
        if let Some(v) = self.stroke_width {
            style.stroke_width = Some(v);
        }
        if let Some(v) = self.stroke_dasharray {
            style.stroke_dasharray = Some(v);
        }
        if let Some(v) = self.stroke_dashoffset {
            style.stroke_dashoffset = Some(v);
        }
        if let Some(v) = self.border_radius {
            style.border_radius = Some(BorderRadius::uniform(v));
        }
        if let Some(v) = self.border_width {
            style.border_width = Some(v);
        }
        if let Some(v) = self.border_top_width {
            style.border_top_width = Some(v);
        }
        if let Some(v) = self.border_right_width {
            style.border_right_width = Some(v);
        }
        if let Some(v) = self.border_bottom_width {
            style.border_bottom_width = Some(v);
        }
        if let Some(v) = self.border_left_width {
            style.border_left_width = Some(v);
        }
        if let Some(v) = self.border_color {
            style.border_color = Some(v);
        }
        if let Some(v) = self.border_style {
            style.border_style = Some(v);
        }
        if let Some(v) = self.object_fit {
            style.object_fit = Some(v);
        }
        if !self.transforms.is_empty() {
            style.transforms.extend(self.transforms.iter().cloned());
        }
        if let Some(v) = self.text_color {
            style.text_color = Some(v);
        }
        if let Some(v) = self.text_px {
            style.text_px = Some(v);
        }
        if let Some(v) = self.font_weight {
            style.font_weight = Some(v);
        }
        if let Some(v) = self.letter_spacing {
            style.letter_spacing = Some(v);
        }
        if let Some(v) = self.text_align {
            style.text_align = Some(v);
        }
        if let Some(v) = self.line_height {
            style.line_height = Some(v);
        }
        if let Some(v) = self.box_shadow {
            style.box_shadow = Some(v);
        }
        if let Some(v) = self.box_shadow_color {
            style.box_shadow_color = Some(v);
        }
        if let Some(v) = self.inset_shadow {
            style.inset_shadow = Some(v);
        }
        if let Some(v) = self.inset_shadow_color {
            style.inset_shadow_color = Some(v);
        }
        if let Some(v) = self.drop_shadow {
            style.drop_shadow = Some(v);
        }
        if let Some(v) = self.drop_shadow_color {
            style.drop_shadow_color = Some(v);
        }
        if let Some(v) = &self.svg_path {
            style.svg_path = Some(v.clone());
        }
    }
}

// ── Canvas mutations ──────────────────────────────────────────────

use crate::draw::op::DrawOp;

#[derive(Debug, Clone, Default)]
pub struct CanvasMutations {
    pub commands: Vec<DrawOp>,
}

// ── Style mutations collection ────────────────────────────────────

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleMutations {
    pub mutations: HashMap<String, NodeStyleMutations>,
    #[serde(skip)]
    pub canvas_mutations: HashMap<String, CanvasMutations>,
}

impl StyleMutations {
    pub fn get(&self, id: &str) -> Option<&NodeStyleMutations> {
        self.mutations.get(id)
    }

    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty() && self.canvas_mutations.is_empty()
    }

    pub fn apply_to_node(&self, node_style: &mut crate::style::NodeStyle, id: &str) {
        if let Some(mutation) = self.mutations.get(id) {
            mutation.apply_to(node_style);
        }
    }

    pub fn get_canvas(&self, id: &str) -> Option<&CanvasMutations> {
        self.canvas_mutations.get(id)
    }

    pub fn apply_to_canvas(&self, commands: &mut Vec<DrawOp>, id: &str) {
        if let Some(mutation) = self.canvas_mutations.get(id) {
            commands.extend(mutation.commands.iter().cloned());
        }
    }

    pub fn text_content_for(&self, id: &str) -> Option<&str> {
        self.mutations
            .get(id)
            .and_then(|m| m.text_content.as_deref())
    }

    pub fn apply_to_recorder(&self, recorder: &mut dyn crate::script::recorder::MutationRecorder) {
        for (node_id, node_mutations) in &self.mutations {
            apply_node_to_recorder(recorder, node_id, node_mutations);
        }
        for (canvas_id, canvas_mutations) in &self.canvas_mutations {
            for cmd in &canvas_mutations.commands {
                recorder.record_draw_op(canvas_id, cmd.clone());
            }
        }
    }
}

pub fn apply_node_to_recorder(
    recorder: &mut dyn crate::script::recorder::MutationRecorder,
    id: &str,
    m: &NodeStyleMutations,
) {
    if let Some(v) = m.opacity {
        recorder.record_opacity(id, v);
    }
    if let Some(v) = m.inset_left {
        recorder.record_left(id, v);
    }
    if let Some(v) = m.inset_top {
        recorder.record_top(id, v);
    }
    if let Some(v) = m.inset_right {
        recorder.record_right(id, v);
    }
    if let Some(v) = m.inset_bottom {
        recorder.record_bottom(id, v);
    }
    if let Some(v) = m.width {
        recorder.record_width(id, v);
    }
    if let Some(v) = m.height {
        recorder.record_height(id, v);
    }
    if let Some(v) = m.padding {
        recorder.record_padding(id, v);
    }
    if let Some(v) = m.padding_x {
        recorder.record_padding_x(id, v);
    }
    if let Some(v) = m.padding_y {
        recorder.record_padding_y(id, v);
    }
    if let Some(v) = m.margin {
        recorder.record_margin(id, v);
    }
    if let Some(v) = m.margin_x {
        recorder.record_margin_x(id, v);
    }
    if let Some(v) = m.margin_y {
        recorder.record_margin_y(id, v);
    }
    if let Some(v) = m.gap {
        recorder.record_gap(id, v);
    }
    if let Some(v) = m.flex_grow {
        recorder.record_flex_grow(id, v);
    }
    if let Some(v) = m.border_radius {
        recorder.record_border_radius(id, v);
    }
    if let Some(v) = m.border_width {
        recorder.record_border_width(id, v);
    }
    if let Some(v) = m.border_top_width {
        recorder.record_border_top_width(id, v);
    }
    if let Some(v) = m.border_right_width {
        recorder.record_border_right_width(id, v);
    }
    if let Some(v) = m.border_bottom_width {
        recorder.record_border_bottom_width(id, v);
    }
    if let Some(v) = m.border_left_width {
        recorder.record_border_left_width(id, v);
    }
    if let Some(v) = m.stroke_width {
        recorder.record_stroke_width(id, v);
    }
    if let Some(v) = m.stroke_dasharray {
        recorder.record_stroke_dasharray(id, v);
    }
    if let Some(v) = m.stroke_dashoffset {
        recorder.record_stroke_dashoffset(id, v);
    }
    if let Some(v) = m.text_px {
        recorder.record_text_size(id, v);
    }
    if let Some(v) = m.letter_spacing {
        recorder.record_letter_spacing(id, v);
    }
    if let Some(v) = m.line_height {
        recorder.record_line_height(id, v);
    }
    if let Some(pos) = m.position {
        recorder.record_position(id, pos);
    }
    if let Some(fd) = m.flex_direction {
        recorder.record_flex_direction(id, fd);
    }
    if let Some(jc) = m.justify_content {
        recorder.record_justify_content(id, jc);
    }
    if let Some(ai) = m.align_items {
        recorder.record_align_items(id, ai);
    }
    if let Some(of) = m.object_fit {
        recorder.record_object_fit(id, of);
    }
    if let Some(ta) = m.text_align {
        recorder.record_text_align(id, ta);
    }
    if let Some(bs) = m.border_style {
        recorder.record_border_style(id, bs);
    }
    if let Some(w) = m.font_weight {
        recorder.record_font_weight(id, w);
    }
    if let Some(sh) = m.box_shadow {
        recorder.record_box_shadow(id, sh);
    }
    if let Some(sh) = m.inset_shadow {
        recorder.record_inset_shadow(id, sh);
    }
    if let Some(sh) = m.drop_shadow {
        recorder.record_drop_shadow(id, sh);
    }
    if let Some(color) = m.bg_color {
        recorder.record_bg_color(id, color);
    }
    if let Some(color) = m.fill_color {
        recorder.record_fill_color(id, color);
    }
    if let Some(color) = m.stroke_color {
        recorder.record_stroke_color(id, color);
    }
    if let Some(color) = m.border_color {
        recorder.record_border_color(id, color);
    }
    if let Some(color) = m.text_color {
        recorder.record_text_color(id, color);
    }
    if let Some(color) = m.box_shadow_color {
        recorder.record_box_shadow_color(id, color);
    }
    if let Some(color) = m.inset_shadow_color {
        recorder.record_inset_shadow_color(id, color);
    }
    if let Some(color) = m.drop_shadow_color {
        recorder.record_drop_shadow_color(id, color);
    }
    for t in &m.transforms {
        match *t {
            Transform::Translate { x, y } => recorder.record_translate(id, x, y),
            Transform::TranslateX { value } => recorder.record_translate_x(id, value),
            Transform::TranslateY { value } => recorder.record_translate_y(id, value),
            Transform::Scale { value } => recorder.record_scale(id, value),
            Transform::ScaleX { value } => recorder.record_scale_x(id, value),
            Transform::ScaleY { value } => recorder.record_scale_y(id, value),
            Transform::RotateDeg { value } => recorder.record_rotate(id, value),
            Transform::SkewXDeg { value } => recorder.record_skew_x(id, value),
            Transform::SkewYDeg { value } => recorder.record_skew_y(id, value),
            Transform::SkewDeg { x, y } => recorder.record_skew(id, x, y),
        }
    }
    if let Some(ref text) = m.text_content {
        recorder.record_text_content(id, text.clone());
    }
    if let Some(ref overrides_batch) = m.text_unit_overrides {
        let granularity = overrides_batch.granularity;
        for (index, override_val) in overrides_batch.overrides.iter().enumerate() {
            let values = crate::script::recorder::TextUnitValues {
                opacity: override_val.opacity,
                translate_x: override_val.translate_x,
                translate_y: override_val.translate_y,
                scale: override_val.scale,
                rotation_deg: override_val.rotation_deg,
                color: override_val.color,
            };
            recorder.record_text_unit_override(id, granularity, index, values);
        }
    }
    if let Some(ref data) = m.svg_path {
        recorder.record_svg_path(id, data.clone());
    }
}

// ── Name → enum parsing helpers (for script/JS bridge) ────────────

pub fn line_cap_from_name(name: &str) -> Option<crate::draw::op::LineCap> {
    match name {
        "butt" => Some(crate::draw::op::LineCap::Butt),
        "round" => Some(crate::draw::op::LineCap::Round),
        "square" => Some(crate::draw::op::LineCap::Square),
        _ => None,
    }
}

pub fn line_join_from_name(name: &str) -> Option<crate::draw::op::LineJoin> {
    match name {
        "miter" => Some(crate::draw::op::LineJoin::Miter),
        "round" => Some(crate::draw::op::LineJoin::Round),
        "bevel" => Some(crate::draw::op::LineJoin::Bevel),
        _ => None,
    }
}

pub fn point_mode_from_name(name: &str) -> Option<crate::draw::op::PointMode> {
    match name {
        "points" => Some(crate::draw::op::PointMode::Points),
        "lines" => Some(crate::draw::op::PointMode::Lines),
        "polygon" => Some(crate::draw::op::PointMode::Polygon),
        _ => None,
    }
}

pub fn font_edging_from_name(name: &str) -> Option<String> {
    match name {
        "alias" | "antiAlias" | "subpixelAntiAlias" => Some(name.to_string()),
        _ => None,
    }
}

// ── Color parsing ─────────────────────────────────────────────────

pub fn script_color_from_value(value: &str) -> Option<crate::draw::op::ColorU8> {
    let color = crate::style::color_token_from_script_name(value).map(|color| color.rgba());
    if let Some((r, g, b, a)) = color {
        return Some(crate::draw::op::ColorU8 { r, g, b, a });
    }

    if let Some(color) = parse_rgb_function(value) {
        return Some(color);
    }

    let hex = value.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = parse_hex_nibble(hex.as_bytes()[0])?;
            let g = parse_hex_nibble(hex.as_bytes()[1])?;
            let b = parse_hex_nibble(hex.as_bytes()[2])?;
            (r * 17, g * 17, b * 17, 255)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };

    Some(crate::draw::op::ColorU8 { r, g, b, a })
}

fn parse_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_rgb_channel(value: &str) -> Option<u8> {
    let channel = value.trim().parse::<f32>().ok()?;
    if !(0.0..=255.0).contains(&channel) {
        return None;
    }
    Some(channel.round() as u8)
}

fn parse_alpha_channel(value: &str) -> Option<u8> {
    let alpha = value.trim().parse::<f32>().ok()?;
    if !(0.0..=1.0).contains(&alpha) {
        return None;
    }
    Some((alpha * 255.0).round() as u8)
}

fn parse_rgb_function(value: &str) -> Option<crate::draw::op::ColorU8> {
    let (is_rgba, body) = if let Some(body) = value
        .strip_prefix("rgba(")
        .and_then(|body| body.strip_suffix(')'))
    {
        (true, body)
    } else {
        let body = value
            .strip_prefix("rgb(")
            .and_then(|body| body.strip_suffix(')'))?;
        (false, body)
    };

    let parts: Vec<_> = body.split(',').map(str::trim).collect();
    if (!is_rgba && parts.len() != 3) || (is_rgba && parts.len() != 4) {
        return None;
    }

    let r = parse_rgb_channel(parts[0])?;
    let g = parse_rgb_channel(parts[1])?;
    let b = parse_rgb_channel(parts[2])?;
    let a = if is_rgba {
        parse_alpha_channel(parts[3])?
    } else {
        255
    };

    Some(crate::draw::op::ColorU8 { r, g, b, a })
}

// ── Coordinate parsing helpers ─────────────────────────────────────

pub fn parse_image_rect_coords(coords: &[f32]) -> Option<[f32; 4]> {
    if coords.len() < 4 {
        return None;
    }
    Some([coords[0], coords[1], coords[2], coords[3]])
}

pub type DrRectCoords = (f32, f32, f32, f32, f32, f32, f32, f32, f32, f32);

pub fn parse_drrect_coords(coords: &[f32]) -> Option<DrRectCoords> {
    if coords.len() < 10 {
        return None;
    }
    Some((
        coords[0], coords[1], coords[2], coords[3], coords[4], coords[5], coords[6], coords[7],
        coords[8], coords[9],
    ))
}
