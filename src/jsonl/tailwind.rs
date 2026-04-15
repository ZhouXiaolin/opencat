use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use crate::style::{
    AlignItems, ColorToken, FlexDirection, FlexWrap, FontWeight, GradientDirection, GridAutoFlow,
    GridAutoRows, GridPlacement, JustifyContent, LengthPercentageAuto, NodeStyle, ObjectFit,
    Position, ShadowStyle, TextAlign, TextTransform, color_token_from_class_suffix,
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
        ExactClassAction::Shadow(value) => style.shadow = Some(value),
        ExactClassAction::BorderRadius(value) => style.border_radius = Some(value),
        ExactClassAction::BorderWidth(value) => style.border_width = Some(value),
        ExactClassAction::OverflowHidden => style.overflow_hidden = true,
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

    if let Some(n) = class
        .strip_prefix("rounded-")
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.border_radius = Some(n);
        return true;
    }

    if let Some(n) = class
        .strip_prefix("border-")
        .filter(|_| !class.starts_with("border-color-") && !class.starts_with("border-["))
        .and_then(|value| value.parse::<f32>().ok())
    {
        style.border_width = Some(n);
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
    GradientFrom,
    GradientVia,
    GradientTo,
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
    Shadow(ShadowStyle),
    BorderRadius(f32),
    BorderWidth(f32),
    OverflowHidden,
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
    let Some(color) = parse_color_token_with_opacity(value) else {
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

fn apply_f32_target(style: &mut NodeStyle, target: F32Target, value: f32) {
    match target {
        F32Target::Gap => style.gap = Some(value),
        F32Target::GapX => style.gap_x = Some(value),
        F32Target::GapY => style.gap_y = Some(value),
        F32Target::MinHeight => style.min_height = Some(LengthPercentageAuto::Length(value)),
        F32Target::Width => {
            style.width = Some(value);
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
        F32Target::BorderRadius => style.border_radius = Some(value),
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
        ColorTarget::GradientFrom => style.bg_gradient_from = Some(value),
        ColorTarget::GradientVia => style.bg_gradient_via = Some(value),
        ColorTarget::GradientTo => style.bg_gradient_to = Some(value),
    }
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

fn parse_grid_line_value(value: &str) -> Option<i16> {
    value
        .parse::<i16>()
        .ok()
        .or_else(|| {
            value
                .strip_prefix('[')
                .and_then(|v| v.strip_suffix(']'))
                .and_then(|v| v.parse::<i16>().ok())
        })
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
