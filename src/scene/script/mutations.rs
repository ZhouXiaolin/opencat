use std::collections::HashMap;

use crate::style::{
    BorderRadius, BorderStyle, BoxShadow, ColorToken, DropShadow, FlexDirection, FontWeight,
    InsetShadow, JustifyContent, LengthPercentageAuto, ObjectFit, Position, TextAlign, Transform,
};

// ── Node style mutations ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextUnitGranularity {
    Grapheme,
    Word,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextUnitOverride {
    pub opacity: Option<f32>,
    pub translate_x: Option<f32>,
    pub translate_y: Option<f32>,
    pub scale: Option<f32>,
    pub rotation_deg: Option<f32>,
    pub color: Option<ColorToken>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextUnitOverrideBatch {
    pub granularity: TextUnitGranularity,
    pub overrides: Vec<TextUnitOverride>,
}

#[derive(Debug, Clone, Default)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptPointMode {
    Points,
    Lines,
    Polygon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptFontEdging {
    Alias,
    AntiAlias,
    SubpixelAntiAlias,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasCommand {
    Save,
    SaveLayer {
        alpha: f32,
        bounds: Option<[f32; 4]>,
    },
    Restore,
    RestoreToCount {
        count: i32,
    },
    SetFillStyle {
        color: ScriptColor,
    },
    SetStrokeStyle {
        color: ScriptColor,
    },
    SetLineWidth {
        width: f32,
    },
    SetLineCap {
        cap: ScriptLineCap,
    },
    SetLineJoin {
        join: ScriptLineJoin,
    },
    SetLineDash {
        intervals: Vec<f32>,
        phase: f32,
    },
    ClearLineDash,
    SetGlobalAlpha {
        alpha: f32,
    },
    SetAntiAlias {
        enabled: bool,
    },
    Translate {
        x: f32,
        y: f32,
    },
    Scale {
        x: f32,
        y: f32,
    },
    Rotate {
        degrees: f32,
    },
    ClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        anti_alias: bool,
    },
    Clear {
        color: Option<ScriptColor>,
    },
    DrawPaint {
        color: ScriptColor,
        anti_alias: bool,
    },
    DrawText {
        text: String,
        x: f32,
        y: f32,
        color: ScriptColor,
        anti_alias: bool,
        stroke: bool,
        stroke_width: f32,
        font_size: f32,
        font_scale_x: f32,
        font_skew_x: f32,
        font_subpixel: bool,
        font_edging: ScriptFontEdging,
    },
    FillRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
    },
    FillRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    StrokeRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
        stroke_width: f32,
    },
    StrokeRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    DrawLine {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    FillCircle {
        cx: f32,
        cy: f32,
        radius: f32,
    },
    StrokeCircle {
        cx: f32,
        cy: f32,
        radius: f32,
    },
    BeginPath,
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    QuadTo {
        cx: f32,
        cy: f32,
        x: f32,
        y: f32,
    },
    CubicTo {
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    },
    ClosePath,
    AddRectPath {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    AddRRectPath {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    AddOvalPath {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    AddArcPath {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        start_angle: f32,
        sweep_angle: f32,
    },
    FillPath,
    StrokePath,
    DrawImage {
        asset_id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        src_rect: Option<[f32; 4]>,
        alpha: f32,
        anti_alias: bool,
        object_fit: ObjectFit,
    },
    DrawArc {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        start_angle: f32,
        sweep_angle: f32,
        use_center: bool,
    },
    StrokeArc {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        start_angle: f32,
        sweep_angle: f32,
    },
    FillOval {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    StrokeOval {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    ClipPath {
        anti_alias: bool,
    },
    ClipRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        anti_alias: bool,
    },
    DrawPoints {
        mode: ScriptPointMode,
        points: Vec<f32>,
    },
    FillDRRect {
        outer_x: f32,
        outer_y: f32,
        outer_width: f32,
        outer_height: f32,
        outer_radius: f32,
        inner_x: f32,
        inner_y: f32,
        inner_width: f32,
        inner_height: f32,
        inner_radius: f32,
    },
    StrokeDRRect {
        outer_x: f32,
        outer_y: f32,
        outer_width: f32,
        outer_height: f32,
        outer_radius: f32,
        inner_x: f32,
        inner_y: f32,
        inner_width: f32,
        inner_height: f32,
        inner_radius: f32,
    },
    Skew {
        sx: f32,
        sy: f32,
    },
    DrawImageSimple {
        asset_id: String,
        x: f32,
        y: f32,
        alpha: f32,
        anti_alias: bool,
    },
    Concat {
        matrix: [f32; 9],
    },
}

impl std::hash::Hash for CanvasCommand {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            CanvasCommand::Save => {
                0_u8.hash(state);
            }
            CanvasCommand::SaveLayer { alpha, bounds } => {
                45_u8.hash(state);
                alpha.to_bits().hash(state);
                bounds.map(|rect| rect.map(f32::to_bits)).hash(state);
            }
            CanvasCommand::Restore => {
                1_u8.hash(state);
            }
            CanvasCommand::RestoreToCount { count } => {
                43_u8.hash(state);
                count.hash(state);
            }
            CanvasCommand::SetFillStyle { color } => {
                2_u8.hash(state);
                color.hash(state);
            }
            CanvasCommand::SetStrokeStyle { color } => {
                3_u8.hash(state);
                color.hash(state);
            }
            CanvasCommand::SetLineWidth { width } => {
                4_u8.hash(state);
                width.to_bits().hash(state);
            }
            CanvasCommand::SetLineCap { cap } => {
                5_u8.hash(state);
                cap.hash(state);
            }
            CanvasCommand::SetLineJoin { join } => {
                6_u8.hash(state);
                join.hash(state);
            }
            CanvasCommand::SetLineDash { intervals, phase } => {
                7_u8.hash(state);
                intervals
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>()
                    .hash(state);
                phase.to_bits().hash(state);
            }
            CanvasCommand::ClearLineDash => {
                8_u8.hash(state);
            }
            CanvasCommand::SetGlobalAlpha { alpha } => {
                9_u8.hash(state);
                alpha.to_bits().hash(state);
            }
            CanvasCommand::SetAntiAlias { enabled } => {
                44_u8.hash(state);
                enabled.hash(state);
            }
            CanvasCommand::Translate { x, y } => {
                10_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            CanvasCommand::Scale { x, y } => {
                11_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            CanvasCommand::Rotate { degrees } => {
                12_u8.hash(state);
                degrees.to_bits().hash(state);
            }
            CanvasCommand::ClipRect {
                x,
                y,
                width,
                height,
                anti_alias,
            } => {
                13_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                anti_alias.hash(state);
            }
            CanvasCommand::Clear { color } => {
                14_u8.hash(state);
                color.hash(state);
            }
            CanvasCommand::DrawPaint { color, anti_alias } => {
                46_u8.hash(state);
                color.hash(state);
                anti_alias.hash(state);
            }
            CanvasCommand::DrawText {
                text,
                x,
                y,
                color,
                anti_alias,
                stroke,
                stroke_width,
                font_size,
                font_scale_x,
                font_skew_x,
                font_subpixel,
                font_edging,
            } => {
                51_u8.hash(state);
                text.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                color.hash(state);
                anti_alias.hash(state);
                stroke.hash(state);
                stroke_width.to_bits().hash(state);
                font_size.to_bits().hash(state);
                font_scale_x.to_bits().hash(state);
                font_skew_x.to_bits().hash(state);
                font_subpixel.hash(state);
                font_edging.hash(state);
            }
            CanvasCommand::FillRect {
                x,
                y,
                width,
                height,
                color,
            } => {
                15_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                color.hash(state);
            }
            CanvasCommand::FillRRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                16_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                radius.to_bits().hash(state);
            }
            CanvasCommand::StrokeRect {
                x,
                y,
                width,
                height,
                color,
                stroke_width,
            } => {
                17_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                color.hash(state);
                stroke_width.to_bits().hash(state);
            }
            CanvasCommand::StrokeRRect {
                x,
                y,
                width,
                height,
                radius,
            } => {
                18_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                radius.to_bits().hash(state);
            }
            CanvasCommand::DrawLine { x0, y0, x1, y1 } => {
                19_u8.hash(state);
                x0.to_bits().hash(state);
                y0.to_bits().hash(state);
                x1.to_bits().hash(state);
                y1.to_bits().hash(state);
            }
            CanvasCommand::FillCircle { cx, cy, radius } => {
                20_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                radius.to_bits().hash(state);
            }
            CanvasCommand::StrokeCircle { cx, cy, radius } => {
                21_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                radius.to_bits().hash(state);
            }
            CanvasCommand::BeginPath => {
                22_u8.hash(state);
            }
            CanvasCommand::MoveTo { x, y } => {
                23_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            CanvasCommand::LineTo { x, y } => {
                24_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            CanvasCommand::QuadTo { cx, cy, x, y } => {
                25_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            CanvasCommand::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } => {
                26_u8.hash(state);
                c1x.to_bits().hash(state);
                c1y.to_bits().hash(state);
                c2x.to_bits().hash(state);
                c2y.to_bits().hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            CanvasCommand::ClosePath => {
                27_u8.hash(state);
            }
            CanvasCommand::AddRectPath {
                x,
                y,
                width,
                height,
            } => {
                47_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
            }
            CanvasCommand::AddRRectPath {
                x,
                y,
                width,
                height,
                radius,
            } => {
                48_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                radius.to_bits().hash(state);
            }
            CanvasCommand::AddOvalPath {
                x,
                y,
                width,
                height,
            } => {
                49_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
            }
            CanvasCommand::AddArcPath {
                x,
                y,
                width,
                height,
                start_angle,
                sweep_angle,
            } => {
                50_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                start_angle.to_bits().hash(state);
                sweep_angle.to_bits().hash(state);
            }
            CanvasCommand::FillPath => {
                28_u8.hash(state);
            }
            CanvasCommand::StrokePath => {
                29_u8.hash(state);
            }
            CanvasCommand::DrawImage {
                asset_id,
                x,
                y,
                width,
                height,
                src_rect,
                alpha,
                anti_alias,
                object_fit,
            } => {
                30_u8.hash(state);
                asset_id.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                src_rect.map(|rect| rect.map(f32::to_bits)).hash(state);
                alpha.to_bits().hash(state);
                anti_alias.hash(state);
                object_fit.hash(state);
            }
            CanvasCommand::DrawArc {
                cx,
                cy,
                rx,
                ry,
                start_angle,
                sweep_angle,
                use_center,
            } => {
                31_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                rx.to_bits().hash(state);
                ry.to_bits().hash(state);
                start_angle.to_bits().hash(state);
                sweep_angle.to_bits().hash(state);
                use_center.hash(state);
            }
            CanvasCommand::StrokeArc {
                cx,
                cy,
                rx,
                ry,
                start_angle,
                sweep_angle,
            } => {
                32_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                rx.to_bits().hash(state);
                ry.to_bits().hash(state);
                start_angle.to_bits().hash(state);
                sweep_angle.to_bits().hash(state);
            }
            CanvasCommand::FillOval { cx, cy, rx, ry } => {
                33_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                rx.to_bits().hash(state);
                ry.to_bits().hash(state);
            }
            CanvasCommand::StrokeOval { cx, cy, rx, ry } => {
                34_u8.hash(state);
                cx.to_bits().hash(state);
                cy.to_bits().hash(state);
                rx.to_bits().hash(state);
                ry.to_bits().hash(state);
            }
            CanvasCommand::ClipPath { anti_alias } => {
                35_u8.hash(state);
                anti_alias.hash(state);
            }
            CanvasCommand::ClipRRect {
                x,
                y,
                width,
                height,
                radius,
                anti_alias,
            } => {
                36_u8.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                width.to_bits().hash(state);
                height.to_bits().hash(state);
                radius.to_bits().hash(state);
                anti_alias.hash(state);
            }
            CanvasCommand::DrawPoints { mode, points } => {
                37_u8.hash(state);
                mode.hash(state);
                points
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>()
                    .hash(state);
            }
            CanvasCommand::FillDRRect {
                outer_x,
                outer_y,
                outer_width,
                outer_height,
                outer_radius,
                inner_x,
                inner_y,
                inner_width,
                inner_height,
                inner_radius,
            } => {
                38_u8.hash(state);
                outer_x.to_bits().hash(state);
                outer_y.to_bits().hash(state);
                outer_width.to_bits().hash(state);
                outer_height.to_bits().hash(state);
                outer_radius.to_bits().hash(state);
                inner_x.to_bits().hash(state);
                inner_y.to_bits().hash(state);
                inner_width.to_bits().hash(state);
                inner_height.to_bits().hash(state);
                inner_radius.to_bits().hash(state);
            }
            CanvasCommand::StrokeDRRect {
                outer_x,
                outer_y,
                outer_width,
                outer_height,
                outer_radius,
                inner_x,
                inner_y,
                inner_width,
                inner_height,
                inner_radius,
            } => {
                39_u8.hash(state);
                outer_x.to_bits().hash(state);
                outer_y.to_bits().hash(state);
                outer_width.to_bits().hash(state);
                outer_height.to_bits().hash(state);
                outer_radius.to_bits().hash(state);
                inner_x.to_bits().hash(state);
                inner_y.to_bits().hash(state);
                inner_width.to_bits().hash(state);
                inner_height.to_bits().hash(state);
                inner_radius.to_bits().hash(state);
            }
            CanvasCommand::Skew { sx, sy } => {
                40_u8.hash(state);
                sx.to_bits().hash(state);
                sy.to_bits().hash(state);
            }
            CanvasCommand::DrawImageSimple {
                asset_id,
                x,
                y,
                alpha,
                anti_alias,
            } => {
                41_u8.hash(state);
                asset_id.hash(state);
                x.to_bits().hash(state);
                y.to_bits().hash(state);
                alpha.to_bits().hash(state);
                anti_alias.hash(state);
            }
            CanvasCommand::Concat { matrix } => {
                42_u8.hash(state);
                matrix.map(f32::to_bits).hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanvasMutations {
    pub commands: Vec<CanvasCommand>,
}

// ── Style mutations collection ────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct StyleMutations {
    pub mutations: HashMap<String, NodeStyleMutations>,
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

    pub fn apply_to_canvas(&self, commands: &mut Vec<CanvasCommand>, id: &str) {
        if let Some(mutation) = self.canvas_mutations.get(id) {
            commands.extend(mutation.commands.iter().cloned());
        }
    }

    pub fn text_content_for(&self, id: &str) -> Option<&str> {
        self.mutations
            .get(id)
            .and_then(|m| m.text_content.as_deref())
    }
}

// ── Text source ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptTextSource {
    pub text: String,
    pub kind: ScriptTextSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptTextSourceKind {
    TextNode,
    Caption,
}

// ── Script driver ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ScriptDriver {
    pub(crate) source: String,
}

impl ScriptDriver {
    pub fn from_source(source: &str) -> anyhow::Result<Self> {
        Ok(Self {
            source: source.to_string(),
        })
    }

    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Self::from_source(&source)
    }

    pub(crate) fn cache_key(&self) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.source.hash(&mut hasher);
        hasher.finish()
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}
