use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use crate::style::{
    AlignItems, BoxShadow, BoxShadowStyle, ColorToken, DropShadow, DropShadowStyle, FlexDirection,
    FlexWrap, FontWeight, GradientDirection, GridAutoFlow, GridAutoRows, GridPlacement,
    InsetShadow, InsetShadowStyle, JustifyContent, LengthPercentageAuto, NodeStyle, ObjectFit,
    Position, TextAlign, TextTransform, color_token_from_class_suffix,
};

static UNSUPPORTED_TAILWIND_CLASSES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_class_name(class_name: &str) -> NodeStyle {
    parse_class_name_impl(class_name, None)
}

pub(crate) fn parse_class_name_with_context(
    class_name: &str,
    node_id: &str,
    line_number: usize,
) -> NodeStyle {
    parse_class_name_impl(class_name, Some((node_id, line_number)))
}

fn parse_class_name_impl(class_name: &str, context: Option<(&str, usize)>) -> NodeStyle {
    let mut style = NodeStyle {
        auto_size: true,
        ..Default::default()
    };

    if class_name.is_empty() {
        return style;
    }

    let classes: Vec<&str> = class_name.split_whitespace().collect();
    let mut text_size_default_line_height_px = None;
    let mut has_explicit_line_height = false;

    for class in &classes {
        if let Some(after) = class
            .strip_prefix("text-")
            .filter(|_| !class.starts_with("text-["))
        {
            text_size_default_line_height_px =
                parse_tailwind_text_size_default_line_height_px(after);
        }

        if class.starts_with("leading-") {
            has_explicit_line_height = true;
        }

        if !parse_single_class(class, &mut style) {
            if let Some((node_id, line_number)) = context {
                report_unsupported_tailwind_class(class, node_id, line_number);
            }
        }
    }

    if !has_explicit_line_height {
        if let Some(line_height_px) = text_size_default_line_height_px {
            style.line_height_px = Some(line_height_px);
            style.line_height = None;
        }
    }

    style
}

fn report_unsupported_tailwind_class(class: &str, node_id: &str, line_number: usize) {
    let warnings = UNSUPPORTED_TAILWIND_CLASSES.get_or_init(|| Mutex::new(HashSet::new()));
    let mut warnings = warnings
        .lock()
        .expect("unsupported tailwind warning set should not be poisoned");

    if warnings.insert(class.to_string()) {
        eprintln!(
            "Unsupported Tailwind class `{class}` on node `{node_id}` at JSONL line {line_number}; ignoring it."
        );
    }
}

fn parse_single_class(class: &str, style: &mut NodeStyle) -> bool {
    if let Some(action) = exact_class_action(class) {
        apply_exact_class_action(style, action);
        true
    } else {
        parse_arbitrary_class(class, style)
    }
}

fn exact_class_action(class: &str) -> Option<ExactClassAction> {
    EXACT_CLASS_RULES
        .iter()
        .find_map(|(name, action)| (*name == class).then_some(*action))
}

fn apply_exact_class_action(style: &mut NodeStyle, action: ExactClassAction) {
    match action {
        ExactClassAction::Position(value) => style.position = Some(value),
        ExactClassAction::Flex => {
            style.is_flex = true;
            if style.flex_direction.is_none() {
                style.flex_direction = Some(FlexDirection::Row);
            }
        }
        ExactClassAction::Grid => {
            style.is_grid = true;
        }
        ExactClassAction::FlexDirection(value) => {
            style.is_flex = true;
            style.flex_direction = Some(value);
        }
        ExactClassAction::FlexWrap(value) => style.flex_wrap = Some(value),
        ExactClassAction::JustifyContent(value) => style.justify_content = Some(value),
        ExactClassAction::AlignItems(value) => style.align_items = Some(value),
        ExactClassAction::AlignContent(value) => style.align_content = Some(value),
        ExactClassAction::AlignSelf(value) => style.align_self = Some(value),
        ExactClassAction::PlaceContent(value) => {
            style.justify_content = Some(value);
            style.align_content = Some(value);
        }
        ExactClassAction::PlaceSelf(value) => {
            style.align_self = Some(value);
            style.justify_self = Some(value);
        }
        ExactClassAction::JustifyItems(value) => {
            style.justify_items = Some(value);
        }
        ExactClassAction::JustifySelf(value) => {
            style.justify_self = Some(value);
        }
        ExactClassAction::ObjectFit(value) => style.object_fit = Some(value),
        ExactClassAction::FontWeight(value) => style.font_weight = Some(value),
        ExactClassAction::BoxShadow(value) => style.box_shadow = Some(BoxShadow::from_style(value)),
        ExactClassAction::InsetShadow(value) => {
            style.inset_shadow = Some(InsetShadow::from_style(value))
        }
        ExactClassAction::DropShadow(value) => {
            style.drop_shadow = Some(DropShadow::from_style(value))
        }
        ExactClassAction::ClearBoxShadow => style.box_shadow = None,
        ExactClassAction::ClearInsetShadow => style.inset_shadow = None,
        ExactClassAction::ClearDropShadow => style.drop_shadow = None,
        ExactClassAction::BorderRadius(value) => {
            style.border_radius = Some(crate::style::BorderRadius::uniform(value))
        }
        ExactClassAction::BorderWidth(value) => style.border_width = Some(value),
        ExactClassAction::BackdropBlurSigma(value) => style.backdrop_blur_sigma = Some(value),
        ExactClassAction::OverflowHidden => style.overflow_hidden = true,
        ExactClassAction::Truncate => {
            style.overflow_hidden = true;
            style.truncate = true;
        }
        ExactClassAction::LineThrough => style.line_through = true,
        ExactClassAction::Noop => {}
        ExactClassAction::InsetZero => {
            let zero = LengthPercentageAuto::length(0.0);
            style.inset_left = Some(zero);
            style.inset_top = Some(zero);
            style.inset_right = Some(zero);
            style.inset_bottom = Some(zero);
        }
        ExactClassAction::BgGradientDirection(value) => style.bg_gradient_direction = Some(value),
        ExactClassAction::FlexShrink(value) => style.flex_shrink = Some(value),
        ExactClassAction::FlexGrow(value) => style.flex_grow = Some(value),
        ExactClassAction::TextAlign(value) => style.text_align = Some(value),
        ExactClassAction::WidthFull => {
            style.width = None;
            style.width_percent = None;
            style.width_full = true;
        }
        ExactClassAction::HeightFull => {
            style.height = None;
            style.height_full = true;
        }
        ExactClassAction::LineHeight(value) => style.line_height = Some(value),
        ExactClassAction::LetterSpacing(value) => style.letter_spacing = Some(value),
        ExactClassAction::TextTransform(value) => style.text_transform = Some(value),
        ExactClassAction::BlurSigma(value) => style.blur_sigma = Some(value),
        ExactClassAction::GridAutoFlow(value) => style.grid_auto_flow = Some(value),
        ExactClassAction::GridAutoRows(value) => style.grid_auto_rows = Some(value),
        ExactClassAction::AspectRatio(value) => style.aspect_ratio = Some(value),
        ExactClassAction::Order(value) => style.order = Some(value),
        ExactClassAction::ColStartAuto => style.col_start = Some(GridPlacement::Auto),
        ExactClassAction::ColEndAuto => style.col_end = Some(GridPlacement::Auto),
        ExactClassAction::RowStartAuto => style.row_start = Some(GridPlacement::Auto),
        ExactClassAction::RowEndAuto => style.row_end = Some(GridPlacement::Auto),
        ExactClassAction::GridColsNone => style.grid_template_columns = None,
        ExactClassAction::GridRowsNone => style.grid_template_rows = None,
    }
}

fn parse_arbitrary_class(class: &str, style: &mut NodeStyle) -> bool {
    if let Some(border_style) = match class {
        "border-solid" => Some(crate::style::BorderStyle::Solid),
        "border-dashed" => Some(crate::style::BorderStyle::Dashed),
        "border-dotted" => Some(crate::style::BorderStyle::Dotted),
        _ => None,
    } {
        style.border_style = Some(border_style);
        return true;
    }

    if parse_directional_border_width(class, style) {
        return true;
    }

    if class == "fill-none" {
        style.fill_color = Some(ColorToken::Transparent);
        return true;
    }

    if apply_shadow_value_rule(class, "shadow-[", ShadowValueTarget::Box, style)
        || apply_shadow_value_rule(class, "inset-shadow-[", ShadowValueTarget::Inset, style)
        || apply_shadow_value_rule(class, "drop-shadow-[", ShadowValueTarget::Drop, style)
    {
        return true;
    }

    if apply_shadow_color_rule(class, "shadow-", ShadowColorTarget::Box, style)
        || apply_shadow_color_rule(class, "inset-shadow-", ShadowColorTarget::Inset, style)
        || apply_shadow_color_rule(class, "drop-shadow-", ShadowColorTarget::Drop, style)
    {
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "inset-", "-inset-") {
        style.inset_left = Some(value);
        style.inset_top = Some(value);
        style.inset_right = Some(value);
        style.inset_bottom = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "inset-x-", "-inset-x-") {
        style.inset_left = Some(value);
        style.inset_right = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "inset-y-", "-inset-y-") {
        style.inset_top = Some(value);
        style.inset_bottom = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "inset-s-", "-inset-s-") {
        style.inset_left = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "inset-e-", "-inset-e-") {
        style.inset_right = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "inset-bs-", "-inset-bs-") {
        style.inset_top = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "inset-be-", "-inset-be-") {
        style.inset_bottom = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "left-", "-left-") {
        style.inset_left = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "top-", "-top-") {
        style.inset_top = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "right-", "-right-") {
        style.inset_right = Some(value);
        return true;
    }

    if let Some(value) = parse_length_percentage_auto_class(class, "bottom-", "-bottom-") {
        style.inset_bottom = Some(value);
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "m-", "-m-") {
        style.margin = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "mx-", "-mx-") {
        style.margin_x = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "my-", "-my-") {
        style.margin_y = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "mt-", "-mt-") {
        style.margin_top = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "mr-", "-mr-") {
        style.margin_right = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "mb-", "-mb-") {
        style.margin_bottom = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "ml-", "-ml-") {
        style.margin_left = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "ms-", "-ms-") {
        style.margin_left = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "me-", "-me-") {
        style.margin_right = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "mbs-", "-mbs-") {
        style.margin_top = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    if let Some(value) = parse_signed_spacing_scale_class(class, "mbe-", "-mbe-") {
        style.margin_bottom = Some(LengthPercentageAuto::Length(value));
        return true;
    }

    // margin-auto variants: mx-auto, ml-auto, etc.
    match class {
        "m-auto" => {
            style.margin = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "mx-auto" => {
            style.margin_x = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "my-auto" => {
            style.margin_y = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "mt-auto" => {
            style.margin_top = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "mr-auto" => {
            style.margin_right = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "mb-auto" => {
            style.margin_bottom = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "ml-auto" => {
            style.margin_left = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "ms-auto" => {
            style.margin_left = Some(LengthPercentageAuto::Auto);
            return true;
        }
        "me-auto" => {
            style.margin_right = Some(LengthPercentageAuto::Auto);
            return true;
        }
        _ => {}
    }

    // gap-x-N / gap-x-[Npx]
    if let Some(value) = class.strip_prefix("gap-x-") {
        if let Some(n) = parse_bracket_f32(value).or_else(|| parse_tailwind_spacing_token(value)) {
            style.gap_x = Some(n);
            return true;
        }
    }

    // gap-y-N / gap-y-[Npx]
    if let Some(value) = class.strip_prefix("gap-y-") {
        if let Some(n) = parse_bracket_f32(value).or_else(|| parse_tailwind_spacing_token(value)) {
            style.gap_y = Some(n);
            return true;
        }
    }

    // order-N / order-[N]
    if let Some(value) = class.strip_prefix("order-") {
        if let Ok(n) = value.parse::<i32>() {
            style.order = Some(n);
            return true;
        }
        if let Some(value) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            if let Ok(n) = value.parse::<i32>() {
                style.order = Some(n);
                return true;
            }
        }
    }

    // aspect-video → 16/9, aspect-square → 1, aspect-auto, aspect-[W/H]
    if let Some(value) = class.strip_prefix("aspect-") {
        if let Some(ratio) = parse_aspect_ratio(value) {
            style.aspect_ratio = Some(ratio);
            return true;
        }
    }

    // min-h-full → min-height: 100%, min-h-screen → min-height: 100vh, min-h-N, min-h-[Npx]
    if let Some(value) = class.strip_prefix("min-h-") {
        if value == "full" {
            style.min_height = Some(LengthPercentageAuto::Percent(1.0));
            return true;
        }
        if value == "screen" {
            // 100vh — store as a large sentinel; resolved to viewport height at layout time
            style.min_height = Some(LengthPercentageAuto::Percent(-1.0));
            return true;
        }
        if let Some(n) = parse_bracket_f32(value) {
            style.min_height = Some(LengthPercentageAuto::Length(n));
            return true;
        }
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.min_height = Some(LengthPercentageAuto::Length(n));
            return true;
        }
    }

    if parse_grid_axis_shorthand_class(class, "col-", "-col-", "col-span-", style, true) {
        return true;
    }

    if parse_grid_axis_shorthand_class(class, "row-", "-row-", "row-span-", style, false) {
        return true;
    }

    // grid-cols-N
    if let Some(cols_str) = class.strip_prefix("grid-cols-") {
        if let Ok(cols) = cols_str.parse::<u16>() {
            style.is_grid = true;
            style.grid_template_columns = Some(cols);
            return true;
        }
    }

    // grid-rows-N
    if let Some(rows_str) = class.strip_prefix("grid-rows-") {
        if let Ok(rows) = rows_str.parse::<u16>() {
            style.is_grid = true;
            style.grid_template_rows = Some(rows);
            return true;
        }
    }

    // col-start-N / -col-start-N
    if let Some(value) = class.strip_prefix("col-start-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.col_start = Some(GridPlacement::Line(line));
            return true;
        }
    }
    if let Some(value) = class.strip_prefix("-col-start-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.col_start = Some(GridPlacement::Line(-line));
            return true;
        }
    }

    // col-end-N / -col-end-N
    if let Some(value) = class.strip_prefix("col-end-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.col_end = Some(GridPlacement::Line(line));
            return true;
        }
    }
    if let Some(value) = class.strip_prefix("-col-end-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.col_end = Some(GridPlacement::Line(-line));
            return true;
        }
    }

    // row-start-N / -row-start-N
    if let Some(value) = class.strip_prefix("row-start-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.row_start = Some(GridPlacement::Line(line));
            return true;
        }
    }
    if let Some(value) = class.strip_prefix("-row-start-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.row_start = Some(GridPlacement::Line(-line));
            return true;
        }
    }

    // row-end-N / -row-end-N
    if let Some(value) = class.strip_prefix("row-end-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.row_end = Some(GridPlacement::Line(line));
            return true;
        }
    }
    if let Some(value) = class.strip_prefix("-row-end-") {
        if let Some(line) = parse_grid_line_value(value) {
            style.row_end = Some(GridPlacement::Line(-line));
            return true;
        }
    }

    if parse_flex_shorthand_class(class, style) {
        return true;
    }

    if apply_signed_bracket_f32_rule(class, SIGNED_BRACKET_F32_RULES, style) {
        return true;
    }

    if apply_bracket_hex_color_rule(class, "bg-[", ColorTarget::Bg, style)
        || apply_bracket_hex_color_rule(class, "text-[", ColorTarget::Text, style)
        || apply_bracket_hex_color_rule(class, "border-[", ColorTarget::Border, style)
        || apply_bracket_hex_color_rule(class, "fill-[", ColorTarget::Fill, style)
        || apply_bracket_hex_color_rule(class, "stroke-[", ColorTarget::Stroke, style)
    {
        return true;
    }

    if apply_bracket_color_rule(class, "border-color-[", ColorTarget::Border, style) {
        return true;
    }

    if apply_bracket_tracking_rule(class, style) {
        return true;
    }

    if apply_bracket_line_height_rule(class, style) {
        return true;
    }

    if apply_bracket_percent_rule(class, "w-[", style) {
        return true;
    }

    if apply_bracket_f32_rule(class, BRACKET_F32_RULES, style) {
        return true;
    }

    if apply_color_prefix_rule(class, COLOR_PREFIX_RULES, style) {
        return true;
    }

    if apply_spacing_scale_rule(class, SPACING_SCALE_RULES, style) {
        return true;
    }

    if let Some(n) = parse_prefixed_bracket_f32(class, "basis-[") {
        style.flex_basis = Some(LengthPercentageAuto::length(n));
        return true;
    }

    if let Some(value) = class.strip_prefix("basis-") {
        if value == "auto" {
            style.flex_basis = Some(LengthPercentageAuto::auto());
            return true;
        }
        if value == "full" {
            style.flex_basis = Some(LengthPercentageAuto::percent(1.0));
            return true;
        }
        if let Some(percent) = parse_fraction(value) {
            style.flex_basis = Some(LengthPercentageAuto::percent(percent));
            return true;
        }
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.flex_basis = Some(LengthPercentageAuto::length(n));
            return true;
        }
    }

    if let Some(after) = class
        .strip_prefix("text-")
        .filter(|_| !class.starts_with("text-["))
    {
        if let Some(n) = parse_tailwind_text_size(after) {
            style.text_px = Some(n);
            return true;
        }
    }

    if apply_directional_rounded(class, style) {
        return true;
    }

    if let Some(value) = class.strip_prefix("rounded-") {
        if let Some(n) = resolve_rounded_size(value) {
            style.border_radius = Some(crate::style::BorderRadius::uniform(n));
            return true;
        }
    }

    if let Some(n) = class
        .strip_prefix("border-")
        .filter(|_| !class.starts_with("border-color-") && !class.starts_with("border-["))
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.border_width = Some(n);
        return true;
    }

    // Tailwind stroke-width: stroke-0 | stroke-1 | stroke-2; stroke-[n] for arbitrary
    if let Some(n) = parse_prefixed_bracket_f32(class, "stroke-[") {
        style.stroke_width = Some(n);
        return true;
    }
    if let Some(n) = class
        .strip_prefix("stroke-")
        .filter(|_| !class.starts_with("stroke-["))
        .and_then(|value| match value {
            "0" => Some(0.0),
            "1" => Some(1.0),
            "2" => Some(2.0),
            _ => None,
        })
    {
        style.stroke_width = Some(n);
        return true;
    }

    if let Some(n) = class
        .strip_prefix("opacity-")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.opacity = Some((n / 100.0).clamp(0.0, 1.0));
        return true;
    }

    if let Some(n) = class
        .strip_prefix("grow-")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.flex_grow = Some(n);
        return true;
    }

    if class == "grow" {
        style.flex_grow = Some(1.0);
        return true;
    }

    if let Some(n) = class
        .strip_prefix("shrink-")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.flex_shrink = Some(n);
        return true;
    }

    if class == "shrink" {
        style.flex_shrink = Some(1.0);
        return true;
    }

    if let Some(n) = class
        .strip_prefix("z-[")
        .and_then(|value| value.strip_suffix(']'))
        .and_then(|value| value.parse::<i32>().ok())
    {
        style.z_index = Some(n);
        return true;
    }

    if let Some(n) = class
        .strip_prefix("z-")
        .and_then(|value| value.parse::<i32>().ok())
    {
        style.z_index = Some(n);
        return true;
    }

    if let Some(n) = class
        .strip_prefix("tracking-")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.letter_spacing = Some(n);
        return true;
    }

    false
}

#[derive(Copy, Clone)]
enum F32Target {
    Gap,
    GapX,
    GapY,
    MinHeight,
    Width,
    Height,
    MaxWidth,
    InsetLeft,
    InsetTop,
    InsetRight,
    InsetBottom,
    TextPx,
    Padding,
    PaddingX,
    PaddingY,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    Margin,
    MarginX,
    MarginY,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    BorderRadius,
    BorderWidth,
    OpacityClamped,
    BlurSigma,
    FlexGrow,
    FlexShrink,
}

#[derive(Copy, Clone)]
enum ColorTarget {
    Bg,
    Text,
    Border,
    Fill,
    Stroke,
    GradientFrom,
    GradientVia,
    GradientTo,
}

#[derive(Copy, Clone)]
enum ShadowColorTarget {
    Box,
    Inset,
    Drop,
}

#[derive(Copy, Clone)]
enum ShadowValueTarget {
    Box,
    Inset,
    Drop,
}

#[derive(Copy, Clone)]
enum ExactClassAction {
    Position(Position),
    Flex,
    Grid,
    FlexDirection(FlexDirection),
    FlexWrap(FlexWrap),
    JustifyContent(JustifyContent),
    AlignItems(AlignItems),
    AlignContent(JustifyContent),
    AlignSelf(AlignItems),
    PlaceContent(JustifyContent),
    PlaceSelf(AlignItems),
    JustifyItems(AlignItems),
    JustifySelf(AlignItems),
    ObjectFit(ObjectFit),
    FontWeight(FontWeight),
    BoxShadow(BoxShadowStyle),
    InsetShadow(InsetShadowStyle),
    DropShadow(DropShadowStyle),
    ClearBoxShadow,
    ClearInsetShadow,
    ClearDropShadow,
    BorderRadius(f32),
    BorderWidth(f32),
    OverflowHidden,
    Truncate,
    LineThrough,
    Noop,
    InsetZero,
    BgGradientDirection(GradientDirection),
    FlexShrink(f32),
    FlexGrow(f32),
    TextAlign(TextAlign),
    WidthFull,
    HeightFull,
    LineHeight(f32),
    LetterSpacing(f32),
    TextTransform(TextTransform),
    BlurSigma(f32),
    BackdropBlurSigma(f32),
    GridAutoFlow(GridAutoFlow),
    GridAutoRows(GridAutoRows),
    AspectRatio(f32),
    Order(i32),
    ColStartAuto,
    ColEndAuto,
    RowStartAuto,
    RowEndAuto,
    GridColsNone,
    GridRowsNone,
}

include!(concat!(env!("OUT_DIR"), "/tailwind_jsonl_rules.rs"));

fn apply_signed_bracket_f32_rule(
    class: &str,
    rules: &[(&str, &str, F32Target)],
    style: &mut NodeStyle,
) -> bool {
    for (positive_prefix, negative_prefix, target) in rules {
        if let Some(n) = parse_signed_bracket_f32(class, positive_prefix, negative_prefix) {
            apply_f32_target(style, *target, n);
            return true;
        }
    }
    false
}

fn apply_bracket_f32_rule(class: &str, rules: &[(&str, F32Target)], style: &mut NodeStyle) -> bool {
    for (prefix, setter) in rules {
        if let Some(n) = parse_prefixed_bracket_f32(class, prefix) {
            apply_f32_target(style, *setter, n);
            return true;
        }
    }
    false
}

fn apply_spacing_scale_rule(
    class: &str,
    rules: &[(&str, F32Target)],
    style: &mut NodeStyle,
) -> bool {
    for (prefix, target) in rules {
        if let Some(value) = class.strip_prefix(prefix) {
            if let Some(n) = parse_tailwind_spacing_token(value) {
                apply_f32_target(style, *target, n);
                return true;
            }
        }
    }
    false
}

fn apply_color_prefix_rule(
    class: &str,
    rules: &[(&str, ColorTarget)],
    style: &mut NodeStyle,
) -> bool {
    for (prefix, target) in rules {
        if let Some(value) = class.strip_prefix(prefix) {
            if let Some(color) = parse_color_token_with_opacity(value) {
                apply_color_target(style, *target, color);
                return true;
            }
        }
    }
    false
}

fn apply_shadow_color_rule(
    class: &str,
    prefix: &str,
    target: ShadowColorTarget,
    style: &mut NodeStyle,
) -> bool {
    let Some(value) = class.strip_prefix(prefix) else {
        return false;
    };
    let Some(color) = parse_any_color_value(value) else {
        return false;
    };
    match target {
        ShadowColorTarget::Box => style.box_shadow_color = Some(color),
        ShadowColorTarget::Inset => style.inset_shadow_color = Some(color),
        ShadowColorTarget::Drop => style.drop_shadow_color = Some(color),
    }
    true
}

fn apply_shadow_value_rule(
    class: &str,
    prefix: &str,
    target: ShadowValueTarget,
    style: &mut NodeStyle,
) -> bool {
    let Some(value) = class
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };

    match target {
        ShadowValueTarget::Box => parse_box_shadow_value(value)
            .map(|shadow| style.box_shadow = Some(shadow))
            .is_some(),
        ShadowValueTarget::Inset => parse_inset_shadow_value(value)
            .map(|shadow| style.inset_shadow = Some(shadow))
            .is_some(),
        ShadowValueTarget::Drop => parse_drop_shadow_value(value)
            .map(|shadow| style.drop_shadow = Some(shadow))
            .is_some(),
    }
}

fn parse_box_shadow_value(value: &str) -> Option<BoxShadow> {
    let tokens = split_shadow_tokens(value);
    let (length_tokens, color) = split_shadow_tokens_and_color(tokens)?;
    let color = color.unwrap_or(ColorToken::Custom(0, 0, 0, 30));
    match length_tokens.as_slice() {
        [offset_x, offset_y, blur] => Some(BoxShadow {
            offset_x: parse_shadow_length(offset_x)?,
            offset_y: parse_shadow_length(offset_y)?,
            blur_sigma: parse_shadow_blur(blur)?,
            spread: 0.0,
            color,
        }),
        [offset_x, offset_y, blur, spread] => Some(BoxShadow {
            offset_x: parse_shadow_length(offset_x)?,
            offset_y: parse_shadow_length(offset_y)?,
            blur_sigma: parse_shadow_blur(blur)?,
            spread: parse_shadow_length(spread)?,
            color,
        }),
        _ => None,
    }
}

fn parse_inset_shadow_value(value: &str) -> Option<InsetShadow> {
    let tokens = split_shadow_tokens(value);
    let (length_tokens, color) = split_shadow_tokens_and_color(tokens)?;
    let color = color.unwrap_or(ColorToken::Custom(0, 0, 0, 30));
    match length_tokens.as_slice() {
        [offset_x, offset_y, blur] => Some(InsetShadow {
            offset_x: parse_shadow_length(offset_x)?,
            offset_y: parse_shadow_length(offset_y)?,
            blur_sigma: parse_shadow_blur(blur)?,
            spread: 0.0,
            color,
        }),
        [offset_x, offset_y, blur, spread] => Some(InsetShadow {
            offset_x: parse_shadow_length(offset_x)?,
            offset_y: parse_shadow_length(offset_y)?,
            blur_sigma: parse_shadow_blur(blur)?,
            spread: parse_shadow_length(spread)?,
            color,
        }),
        _ => None,
    }
}

fn parse_drop_shadow_value(value: &str) -> Option<DropShadow> {
    let tokens = split_shadow_tokens(value);
    let (length_tokens, color) = split_shadow_tokens_and_color(tokens)?;
    let color = color.unwrap_or(ColorToken::Custom(0, 0, 0, 30));
    match length_tokens.as_slice() {
        [offset_x, offset_y] => Some(DropShadow {
            offset_x: parse_shadow_length(offset_x)?,
            offset_y: parse_shadow_length(offset_y)?,
            blur_sigma: 0.0,
            color,
        }),
        [offset_x, offset_y, blur] => Some(DropShadow {
            offset_x: parse_shadow_length(offset_x)?,
            offset_y: parse_shadow_length(offset_y)?,
            blur_sigma: parse_shadow_blur(blur)?,
            color,
        }),
        _ => None,
    }
}

fn split_shadow_tokens(value: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut depth: usize = 0;

    for (idx, ch) in value.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '_' if depth == 0 => {
                tokens.push(&value[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }

    if start <= value.len() {
        tokens.push(&value[start..]);
    }

    tokens
        .into_iter()
        .filter(|token| !token.is_empty())
        .collect()
}

fn split_shadow_tokens_and_color(tokens: Vec<&str>) -> Option<(Vec<&str>, Option<ColorToken>)> {
    let mut tokens = tokens;
    if tokens.is_empty() {
        return None;
    }
    let color = if let Some(last) = tokens.last().copied() {
        if let Some(color) = parse_any_color_value(last) {
            tokens.pop();
            Some(color)
        } else {
            None
        }
    } else {
        None
    };
    Some((tokens, color))
}

fn parse_shadow_length(token: &str) -> Option<f32> {
    let token = token.trim();
    token
        .strip_suffix("px")
        .unwrap_or(token)
        .parse::<f32>()
        .ok()
}

fn parse_shadow_blur(token: &str) -> Option<f32> {
    parse_shadow_length(token).map(|value| value / 6.0)
}

fn apply_bracket_hex_color_rule(
    class: &str,
    prefix: &str,
    target: ColorTarget,
    style: &mut NodeStyle,
) -> bool {
    let Some(value) = class
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };
    let Some(color) = color_from_hex(value) else {
        return false;
    };
    apply_color_target(style, target, color);
    true
}

fn apply_bracket_color_rule(
    class: &str,
    prefix: &str,
    target: ColorTarget,
    style: &mut NodeStyle,
) -> bool {
    let Some(value) = class
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };
    let Some(color) = parse_any_color_value(value) else {
        return false;
    };
    apply_color_target(style, target, color);
    true
}

fn apply_bracket_tracking_rule(class: &str, style: &mut NodeStyle) -> bool {
    let Some(value) = class
        .strip_prefix("tracking-[")
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };
    if let Some(value) = value
        .strip_suffix("em")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.letter_spacing_em = Some(value);
        return true;
    }
    if let Some(value) = value
        .strip_suffix("px")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.letter_spacing = Some(value);
        return true;
    }
    if let Ok(value) = value.parse::<f32>() {
        style.letter_spacing = Some(value);
        return true;
    }
    false
}

fn apply_bracket_line_height_rule(class: &str, style: &mut NodeStyle) -> bool {
    let Some(value) = class
        .strip_prefix("leading-[")
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };

    if let Some(px) = value
        .strip_suffix("px")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.line_height_px = Some(px);
        style.line_height = None;
        return true;
    }

    if let Ok(scale) = value.parse::<f32>() {
        style.line_height = Some(scale);
        style.line_height_px = None;
        return true;
    }

    false
}

fn parse_prefixed_bracket_f32(class: &str, prefix: &str) -> Option<f32> {
    class.strip_prefix(prefix).and_then(parse_bracket_f32)
}

fn apply_bracket_percent_rule(class: &str, prefix: &str, style: &mut NodeStyle) -> bool {
    let Some(value) = class
        .strip_prefix(prefix)
        .and_then(|v| v.strip_suffix(']'))
        .and_then(|v| v.strip_suffix('%'))
    else {
        return false;
    };
    let Ok(percent) = value.parse::<f32>() else {
        return false;
    };
    // Tailwind 允许任意百分比字面量（含 >100%），但传给 Taffy 前必须是有限值。
    // CSS 浏览器对负 width 的处理是按 0 渲染，这里保持一致以兼容 Tailwind 输入。
    if !percent.is_finite() {
        return false;
    }
    style.width_percent = Some((percent / 100.0).max(0.0));
    style.width = None;
    style.width_full = false;
    true
}

const ROUNDED_SIZE_MAP: &[(&str, f32)] = &[
    ("none", 0.0),
    ("sm", 4.0),
    ("", 8.0),
    ("md", 8.0),
    ("lg", 16.0),
    ("xl", 24.0),
    ("2xl", 32.0),
    ("3xl", 48.0),
    ("full", 9999.0),
];

fn resolve_rounded_size(size_str: &str) -> Option<f32> {
    ROUNDED_SIZE_MAP
        .iter()
        .find(|(name, _)| *name == size_str)
        .map(|(_, v)| *v)
        .or_else(|| parse_bracket_f32(size_str))
        .or_else(|| size_str.parse::<f32>().ok())
}

fn apply_directional_rounded(class: &str, style: &mut NodeStyle) -> bool {
    let directions: &[(&str, [bool; 4])] = &[
        ("rounded-t-", [true, true, false, false]),
        ("rounded-b-", [false, false, true, true]),
        ("rounded-l-", [true, false, false, true]),
        ("rounded-r-", [false, true, true, false]),
        ("rounded-tl-", [true, false, false, false]),
        ("rounded-tr-", [false, true, false, false]),
        ("rounded-bl-", [false, false, false, true]),
        ("rounded-br-", [false, false, true, false]),
    ];

    for (prefix, corners) in directions {
        if let Some(size_str) = class.strip_prefix(prefix) {
            let Some(value) = resolve_rounded_size(size_str) else {
                return false;
            };
            let r = style
                .border_radius
                .get_or_insert_with(crate::style::BorderRadius::default);
            if corners[0] {
                r.top_left = value;
            }
            if corners[1] {
                r.top_right = value;
            }
            if corners[2] {
                r.bottom_right = value;
            }
            if corners[3] {
                r.bottom_left = value;
            }
            return true;
        }
    }
    false
}

fn apply_f32_target(style: &mut NodeStyle, target: F32Target, value: f32) {
    match target {
        F32Target::Gap => style.gap = Some(value),
        F32Target::GapX => style.gap_x = Some(value),
        F32Target::GapY => style.gap_y = Some(value),
        F32Target::MinHeight => style.min_height = Some(LengthPercentageAuto::Length(value)),
        F32Target::Width => {
            style.width = Some(value);
            style.width_percent = None;
            style.width_full = false;
        }
        F32Target::Height => {
            style.height = Some(value);
            style.height_full = false;
        }
        F32Target::MaxWidth => style.max_width = Some(value),
        F32Target::InsetLeft => style.inset_left = Some(LengthPercentageAuto::length(value)),
        F32Target::InsetTop => style.inset_top = Some(LengthPercentageAuto::length(value)),
        F32Target::InsetRight => style.inset_right = Some(LengthPercentageAuto::length(value)),
        F32Target::InsetBottom => style.inset_bottom = Some(LengthPercentageAuto::length(value)),
        F32Target::TextPx => style.text_px = Some(value),
        F32Target::Padding => style.padding = Some(value),
        F32Target::PaddingX => style.padding_x = Some(value),
        F32Target::PaddingY => style.padding_y = Some(value),
        F32Target::PaddingTop => style.padding_top = Some(value),
        F32Target::PaddingRight => style.padding_right = Some(value),
        F32Target::PaddingBottom => style.padding_bottom = Some(value),
        F32Target::PaddingLeft => style.padding_left = Some(value),
        F32Target::Margin => style.margin = Some(LengthPercentageAuto::Length(value)),
        F32Target::MarginX => style.margin_x = Some(LengthPercentageAuto::Length(value)),
        F32Target::MarginY => style.margin_y = Some(LengthPercentageAuto::Length(value)),
        F32Target::MarginTop => style.margin_top = Some(LengthPercentageAuto::Length(value)),
        F32Target::MarginRight => style.margin_right = Some(LengthPercentageAuto::Length(value)),
        F32Target::MarginBottom => style.margin_bottom = Some(LengthPercentageAuto::Length(value)),
        F32Target::MarginLeft => style.margin_left = Some(LengthPercentageAuto::Length(value)),
        F32Target::BorderRadius => {
            style.border_radius = Some(crate::style::BorderRadius::uniform(value))
        }
        F32Target::BorderWidth => style.border_width = Some(value),
        F32Target::OpacityClamped => style.opacity = Some(value.clamp(0.0, 1.0)),
        F32Target::BlurSigma => style.blur_sigma = Some(value),
        F32Target::FlexGrow => style.flex_grow = Some(value),
        F32Target::FlexShrink => style.flex_shrink = Some(value),
    }
}

fn apply_color_target(style: &mut NodeStyle, target: ColorTarget, value: ColorToken) {
    match target {
        ColorTarget::Bg => style.bg_color = Some(value),
        ColorTarget::Text => style.text_color = Some(value),
        ColorTarget::Border => style.border_color = Some(value),
        ColorTarget::Fill => style.fill_color = Some(value),
        ColorTarget::Stroke => style.stroke_color = Some(value),
        ColorTarget::GradientFrom => style.bg_gradient_from = Some(value),
        ColorTarget::GradientVia => style.bg_gradient_via = Some(value),
        ColorTarget::GradientTo => style.bg_gradient_to = Some(value),
    }
}

fn parse_any_color_value(value: &str) -> Option<ColorToken> {
    parse_color_token_with_opacity(value)
        .or_else(|| color_from_hex(value))
        .or_else(|| parse_rgb_function_color(value))
}

fn parse_color_token_with_opacity(value: &str) -> Option<ColorToken> {
    let (base, opacity_suffix) = match value.rsplit_once('/') {
        Some((base, opacity_suffix)) => (base, Some(opacity_suffix)),
        None => (value, None),
    };

    let color = color_token_from_class_suffix(base)?;
    let Some(opacity_suffix) = opacity_suffix else {
        return Some(color);
    };

    let opacity_percent = opacity_suffix.parse::<f32>().ok()?;
    let opacity = (opacity_percent / 100.0).clamp(0.0, 1.0);
    let (r, g, b, a) = color.rgba();
    let alpha = ((a as f32) * opacity).round().clamp(0.0, 255.0) as u8;

    Some(ColorToken::Custom(r, g, b, alpha))
}

fn parse_rgb_function_color(value: &str) -> Option<ColorToken> {
    let lower = value.trim();
    let (inner, has_alpha) = if let Some(inner) = lower
        .strip_prefix("rgba(")
        .and_then(|value| value.strip_suffix(')'))
    {
        (inner, true)
    } else if let Some(inner) = lower
        .strip_prefix("rgb(")
        .and_then(|value| value.strip_suffix(')'))
    {
        (inner, false)
    } else {
        return None;
    };

    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if (!has_alpha && parts.len() != 3) || (has_alpha && parts.len() != 4) {
        return None;
    }

    let r = parse_rgb_channel(parts[0])?;
    let g = parse_rgb_channel(parts[1])?;
    let b = parse_rgb_channel(parts[2])?;
    let a = if has_alpha {
        parse_alpha_channel(parts[3])?
    } else {
        255
    };
    Some(ColorToken::Custom(r, g, b, a))
}

fn parse_rgb_channel(value: &str) -> Option<u8> {
    value
        .parse::<f32>()
        .ok()
        .map(|value| value.round().clamp(0.0, 255.0) as u8)
}

fn parse_alpha_channel(value: &str) -> Option<u8> {
    if let Some(percent) = value.strip_suffix('%') {
        let alpha = percent.parse::<f32>().ok()? / 100.0;
        return Some((alpha * 255.0).round().clamp(0.0, 255.0) as u8);
    }

    let alpha = value.parse::<f32>().ok()?;
    let alpha = if alpha <= 1.0 { alpha * 255.0 } else { alpha };
    Some(alpha.round().clamp(0.0, 255.0) as u8)
}

fn parse_signed_bracket_f32(
    class: &str,
    positive_prefix: &str,
    negative_prefix: &str,
) -> Option<f32> {
    if let Some(value) = class.strip_prefix(positive_prefix) {
        return parse_bracket_f32(value);
    }

    class
        .strip_prefix(negative_prefix)
        .and_then(parse_bracket_f32)
        .map(|value| -value)
}

fn parse_signed_spacing_scale_class(
    class: &str,
    positive_prefix: &str,
    negative_prefix: &str,
) -> Option<f32> {
    if let Some(value) = class.strip_prefix(positive_prefix) {
        if let Some(value) = value.strip_prefix('[').and_then(parse_bracket_f32) {
            return Some(value);
        }
        if let Some(value) = parse_bracket_f32(value) {
            return Some(value);
        }
        if let Some(value) = parse_tailwind_spacing_token(value) {
            return Some(value);
        }
    }

    if let Some(value) = class.strip_prefix(negative_prefix) {
        if let Some(value) = value.strip_prefix('[').and_then(parse_bracket_f32) {
            return Some(-value);
        }
        if let Some(value) = parse_bracket_f32(value) {
            return Some(-value);
        }
        if let Some(value) = parse_tailwind_spacing_token(value) {
            return Some(-value);
        }
    }

    None
}

fn parse_bracket_f32(value: &str) -> Option<f32> {
    let value = value.strip_prefix('[').unwrap_or(value);
    value
        .strip_suffix("px]")
        .or_else(|| value.strip_suffix(']'))
        .and_then(|value| value.parse::<f32>().ok())
}

fn parse_directional_border_width(class: &str, style: &mut NodeStyle) -> bool {
    let sides: &[(&str, fn(&mut NodeStyle, f32))] = &[
        ("border-t", |s, w| s.border_top_width = Some(w)),
        ("border-r", |s, w| s.border_right_width = Some(w)),
        ("border-b", |s, w| s.border_bottom_width = Some(w)),
        ("border-l", |s, w| s.border_left_width = Some(w)),
    ];
    for (prefix, setter) in sides {
        if class == *prefix {
            setter(style, 1.0);
            return true;
        }
        let Some(rest) = class.strip_prefix(prefix).and_then(|r| r.strip_prefix('-')) else {
            continue;
        };
        if let Some(n) = parse_bracket_f32(rest).or_else(|| rest.parse::<f32>().ok()) {
            setter(style, n);
            return true;
        }
    }
    false
}

fn parse_grid_line_value(value: &str) -> Option<i16> {
    value.parse::<i16>().ok().or_else(|| {
        value
            .strip_prefix('[')
            .and_then(|v| v.strip_suffix(']'))
            .and_then(|v| v.parse::<i16>().ok())
    })
}

fn parse_grid_span_value(value: &str) -> Option<u16> {
    value.parse::<u16>().ok().or_else(|| {
        value
            .strip_prefix('[')
            .and_then(|v| v.strip_suffix(']'))
            .and_then(|v| v.parse::<u16>().ok())
    })
}

fn parse_grid_axis_token(value: &str) -> Option<GridPlacement> {
    if value == "auto" {
        return Some(GridPlacement::Auto);
    }
    if let Some(value) = value.strip_prefix("span_") {
        return parse_grid_span_value(value).map(GridPlacement::Span);
    }
    parse_grid_line_value(value).map(GridPlacement::Line)
}

fn parse_grid_axis_bracket_shorthand(value: &str) -> Option<(GridPlacement, GridPlacement)> {
    let inner = value.strip_prefix('[')?.strip_suffix(']')?;
    let (start, end) = inner.split_once('/')?;
    Some((parse_grid_axis_token(start)?, parse_grid_axis_token(end)?))
}

fn parse_grid_axis_shorthand_class(
    class: &str,
    axis_prefix: &str,
    negative_axis_prefix: &str,
    span_prefix: &str,
    style: &mut NodeStyle,
    is_column: bool,
) -> bool {
    let mut assign = |start: GridPlacement, end: GridPlacement| {
        if is_column {
            style.col_start = Some(start);
            style.col_end = Some(end);
        } else {
            style.row_start = Some(start);
            style.row_end = Some(end);
        }
    };

    if let Some(value) = class.strip_prefix(span_prefix) {
        if value == "full" {
            assign(GridPlacement::Line(1), GridPlacement::Line(-1));
            return true;
        }
        if let Some(span) = parse_grid_span_value(value) {
            assign(GridPlacement::Span(span), GridPlacement::Span(span));
            return true;
        }
    }

    if let Some(value) = class.strip_prefix(axis_prefix) {
        if let Some((start, end)) = parse_grid_axis_bracket_shorthand(value) {
            assign(start, end);
            return true;
        }
        if value == "auto" {
            assign(GridPlacement::Auto, GridPlacement::Auto);
            return true;
        }
        if let Some(line) = parse_grid_line_value(value) {
            assign(GridPlacement::Line(line), GridPlacement::Auto);
            return true;
        }
    }

    if let Some(value) = class.strip_prefix(negative_axis_prefix) {
        if let Some(line) = parse_grid_line_value(value) {
            assign(GridPlacement::Line(-line), GridPlacement::Auto);
            return true;
        }
    }

    false
}

fn parse_tailwind_spacing_token(value: &str) -> Option<f32> {
    if value == "px" {
        return Some(1.0);
    }

    value.parse::<f32>().ok().map(|value| value * 4.0)
}

fn parse_tailwind_text_size(value: &str) -> Option<f32> {
    TAILWIND_TEXT_SIZE_RULES
        .iter()
        .find_map(|(name, px)| (*name == value).then_some(*px))
        .or_else(|| value.parse::<f32>().ok())
}

fn parse_tailwind_text_size_default_line_height_px(value: &str) -> Option<f32> {
    match value {
        "xs" => Some(16.0),
        "sm" => Some(20.0),
        "base" => Some(24.0),
        "lg" => Some(28.0),
        "xl" => Some(28.0),
        "2xl" => Some(32.0),
        "3xl" => Some(36.0),
        "4xl" => Some(40.0),
        "5xl" => Some(48.0),
        "6xl" => Some(60.0),
        "7xl" => Some(72.0),
        "8xl" => Some(96.0),
        "9xl" => Some(128.0),
        _ => None,
    }
}

fn parse_flex_shorthand_class(class: &str, style: &mut NodeStyle) -> bool {
    match class {
        "flex-auto" => {
            style.flex_grow = Some(1.0);
            style.flex_shrink = Some(1.0);
            style.flex_basis = Some(LengthPercentageAuto::auto());
            return true;
        }
        "flex-initial" => {
            style.flex_grow = Some(0.0);
            style.flex_shrink = Some(1.0);
            style.flex_basis = Some(LengthPercentageAuto::auto());
            return true;
        }
        "flex-none" => {
            style.flex_grow = Some(0.0);
            style.flex_shrink = Some(0.0);
            style.flex_basis = Some(LengthPercentageAuto::auto());
            return true;
        }
        _ => {}
    }

    if let Some(value) = class.strip_prefix("flex-[") {
        if let Some(value) = value
            .strip_suffix(']')
            .and_then(|value| value.parse::<f32>().ok())
        {
            style.flex_grow = Some(value);
            style.flex_shrink = Some(1.0);
            style.flex_basis = Some(LengthPercentageAuto::length(0.0));
            return true;
        }
    }

    if let Some(value) = class.strip_prefix("flex-") {
        if let Some(value) = value.parse::<f32>().ok() {
            style.flex_grow = Some(value);
            style.flex_shrink = Some(1.0);
            style.flex_basis = Some(LengthPercentageAuto::length(0.0));
            return true;
        }
        if let Some(value) = parse_fraction(value) {
            style.flex_grow = Some(1.0);
            style.flex_shrink = Some(1.0);
            style.flex_basis = Some(LengthPercentageAuto::percent(value));
            return true;
        }
    }

    false
}

fn parse_length_percentage_auto_class(
    class: &str,
    positive_prefix: &str,
    negative_prefix: &str,
) -> Option<LengthPercentageAuto> {
    let (negative, value) = if let Some(value) = class.strip_prefix(negative_prefix) {
        (true, value)
    } else if let Some(value) = class.strip_prefix(positive_prefix) {
        (false, value)
    } else {
        return None;
    };

    parse_length_percentage_auto_value(value, negative)
}

fn parse_length_percentage_auto_value(value: &str, negative: bool) -> Option<LengthPercentageAuto> {
    if value == "auto" {
        return (!negative).then_some(LengthPercentageAuto::auto());
    }
    if value == "full" {
        let sign = if negative { -1.0 } else { 1.0 };
        return Some(LengthPercentageAuto::percent(sign));
    }
    if let Some(fraction) = parse_fraction(value) {
        let sign = if negative { -1.0 } else { 1.0 };
        return Some(LengthPercentageAuto::percent(sign * fraction));
    }
    if let Some(value) = value.strip_prefix('[').and_then(parse_bracket_f32) {
        let sign = if negative { -1.0 } else { 1.0 };
        return Some(LengthPercentageAuto::length(sign * value));
    }
    if let Some(value) = parse_bracket_f32(value) {
        let sign = if negative { -1.0 } else { 1.0 };
        return Some(LengthPercentageAuto::length(sign * value));
    }
    if let Some(value) = parse_tailwind_spacing_token(value) {
        let sign = if negative { -1.0 } else { 1.0 };
        return Some(LengthPercentageAuto::length(sign * value));
    }
    None
}

fn parse_fraction(value: &str) -> Option<f32> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<f32>().ok()?;
    let denominator = denominator.parse::<f32>().ok()?;
    if denominator == 0.0 {
        return None;
    }
    Some(numerator / denominator)
}

fn color_from_hex(value: &str) -> Option<ColorToken> {
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

    Some(ColorToken::Custom(r, g, b, a))
}

fn parse_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_aspect_ratio(value: &str) -> Option<f32> {
    match value {
        "auto" => None, // auto = no constraint, don't set
        "square" => Some(1.0),
        "video" => Some(16.0 / 9.0),
        _ => {
            // aspect-[W/H]
            if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                return parse_fraction(inner);
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_class_name;
    use crate::style::ColorToken;

    #[test]
    fn parses_box_drop_and_inset_shadows_separately() {
        let style = parse_class_name("shadow-lg drop-shadow-md inset-shadow-sm");

        assert!(style.box_shadow.is_some());
        assert!(style.drop_shadow.is_some());
        assert!(style.inset_shadow.is_some());
    }

    #[test]
    fn shadow_color_override_is_order_independent() {
        let a = parse_class_name("shadow-red-500 shadow-lg");
        let b = parse_class_name("shadow-lg shadow-red-500");

        assert_eq!(a.box_shadow_color, Some(ColorToken::Red500));
        assert_eq!(b.box_shadow_color, Some(ColorToken::Red500));
    }

    #[test]
    fn parses_arbitrary_shadow_values_and_rgb_colors() {
        let style = parse_class_name(
            "shadow-[0_8px_24px_rgba(0,0,0,0.18)] drop-shadow-[2px_4px_12px_#00000066]",
        );

        let box_shadow = style.box_shadow.expect("box shadow should parse");
        assert_eq!(box_shadow.offset_x, 0.0);
        assert_eq!(box_shadow.offset_y, 8.0);
        assert!((box_shadow.blur_sigma - 4.0).abs() < f32::EPSILON);
        assert_eq!(box_shadow.color, ColorToken::Custom(0, 0, 0, 46));

        let drop_shadow = style.drop_shadow.expect("drop shadow should parse");
        assert_eq!(drop_shadow.offset_x, 2.0);
        assert_eq!(drop_shadow.offset_y, 4.0);
        assert!((drop_shadow.blur_sigma - 2.0).abs() < f32::EPSILON);
        assert_eq!(drop_shadow.color, ColorToken::Custom(0, 0, 0, 102));
    }

    #[test]
    fn parses_fill_none_as_transparent_fill() {
        let style = parse_class_name("fill-none");

        assert_eq!(style.fill_color, Some(ColorToken::Transparent));
    }

    #[test]
    fn parses_arbitrary_percent_width() {
        let style = parse_class_name("w-[65%]");

        assert_eq!(style.width_percent, Some(0.65));
        assert_eq!(style.width, None);
        assert!(!style.width_full);
    }

    #[test]
    fn arbitrary_percent_width_allows_values_above_one_hundred() {
        let style = parse_class_name("w-[150%]");

        assert_eq!(style.width_percent, Some(1.5));
    }

    #[test]
    fn arbitrary_percent_width_clamps_negative_to_zero() {
        // Tailwind 允许 `w-[-10%]` 字面量，CSS 浏览器对负宽度按 0 渲染。
        // 引擎需要保证传给 Taffy 的百分比非负，故 clamp 而非拒绝。
        let style = parse_class_name("w-[-10%]");

        assert_eq!(style.width_percent, Some(0.0));
        assert_eq!(style.width, None);
        assert!(!style.width_full);
    }

    #[test]
    fn arbitrary_percent_width_rejects_non_finite_values() {
        for class in ["w-[NaN%]", "w-[inf%]", "w-[-inf%]"] {
            let style = parse_class_name(class);
            assert!(
                style.width_percent.is_none(),
                "{class} 应被忽略，得到 {:?}",
                style.width_percent,
            );
        }
    }

    #[test]
    fn arbitrary_percent_width_rejects_malformed_input() {
        for class in ["w-[%]", "w-[abc%]", "w-[]"] {
            let style = parse_class_name(class);
            assert!(
                style.width_percent.is_none(),
                "{class} 应被忽略，得到 {:?}",
                style.width_percent,
            );
        }
    }

    #[test]
    fn later_arbitrary_width_overrides_prior_percent() {
        // 模拟 Tailwind 的 last-class-wins 语义：后写的 `w-[200px]` 必须完全覆盖 `w-[50%]`。
        let style = parse_class_name("w-[50%] w-[200px]");

        assert_eq!(style.width, Some(200.0));
        assert_eq!(style.width_percent, None);
        assert!(!style.width_full);
    }

    #[test]
    fn later_width_full_overrides_prior_percent() {
        let style = parse_class_name("w-[50%] w-full");

        assert!(style.width_full);
        assert_eq!(style.width_percent, None);
        assert_eq!(style.width, None);
    }

    #[test]
    fn later_percent_overrides_prior_width_full() {
        let style = parse_class_name("w-full w-[50%]");

        assert_eq!(style.width_percent, Some(0.5));
        assert!(!style.width_full);
        assert_eq!(style.width, None);
    }

    #[test]
    fn later_percent_overrides_prior_arbitrary_width() {
        let style = parse_class_name("w-[200px] w-[50%]");

        assert_eq!(style.width_percent, Some(0.5));
        assert_eq!(style.width, None);
        assert!(!style.width_full);
    }
}
