use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use crate::style::{
    AlignItems, ColorToken, FlexDirection, FontWeight, GradientDirection, JustifyContent,
    NodeStyle, ObjectFit, Position, ShadowStyle, TextAlign, TextTransform,
    color_token_from_class_suffix,
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

    for class in &classes {
        if !parse_single_class(class, &mut style) {
            if let Some((node_id, line_number)) = context {
                report_unsupported_tailwind_class(class, node_id, line_number);
            }
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
        ExactClassAction::FlexDirection(value) => {
            style.is_flex = true;
            style.flex_direction = Some(value);
        }
        ExactClassAction::JustifyContent(value) => style.justify_content = Some(value),
        ExactClassAction::AlignItems(value) => style.align_items = Some(value),
        ExactClassAction::ObjectFit(value) => style.object_fit = Some(value),
        ExactClassAction::FontWeight(value) => style.font_weight = Some(value),
        ExactClassAction::Shadow(value) => style.shadow = Some(value),
        ExactClassAction::BorderRadius(value) => style.border_radius = Some(value),
        ExactClassAction::BorderWidth(value) => style.border_width = Some(value),
        ExactClassAction::OverflowHidden => style.overflow_hidden = true,
        ExactClassAction::Noop => {}
        ExactClassAction::InsetZero => {
            style.inset_left = Some(0.0);
            style.inset_top = Some(0.0);
            style.inset_right = Some(0.0);
            style.inset_bottom = Some(0.0);
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
    }
}

fn parse_arbitrary_class(class: &str, style: &mut NodeStyle) -> bool {
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
        style.flex_basis = Some(n);
        return true;
    }

    if let Some(value) = class.strip_prefix("basis-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.flex_basis = Some(n);
            return true;
        }
    }

    if let Some(n) = parse_prefixed_bracket_f32(class, "inset-x-[") {
        style.inset_left = Some(n);
        style.inset_right = Some(n);
        return true;
    }

    if let Some(value) = class.strip_prefix("inset-x-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.inset_left = Some(n);
            style.inset_right = Some(n);
            return true;
        }
    }

    if let Some(n) = parse_prefixed_bracket_f32(class, "inset-y-[") {
        style.inset_top = Some(n);
        style.inset_bottom = Some(n);
        return true;
    }

    if let Some(value) = class.strip_prefix("inset-y-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.inset_top = Some(n);
            style.inset_bottom = Some(n);
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
    Width,
    Height,
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
    FlexDirection(FlexDirection),
    JustifyContent(JustifyContent),
    AlignItems(AlignItems),
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
        F32Target::Width => {
            style.width = Some(value);
            style.width_full = false;
        }
        F32Target::Height => {
            style.height = Some(value);
            style.height_full = false;
        }
        F32Target::InsetLeft => style.inset_left = Some(value),
        F32Target::InsetTop => style.inset_top = Some(value),
        F32Target::InsetRight => style.inset_right = Some(value),
        F32Target::InsetBottom => style.inset_bottom = Some(value),
        F32Target::TextPx => style.text_px = Some(value),
        F32Target::Padding => style.padding = Some(value),
        F32Target::PaddingX => style.padding_x = Some(value),
        F32Target::PaddingY => style.padding_y = Some(value),
        F32Target::PaddingTop => style.padding_top = Some(value),
        F32Target::PaddingRight => style.padding_right = Some(value),
        F32Target::PaddingBottom => style.padding_bottom = Some(value),
        F32Target::PaddingLeft => style.padding_left = Some(value),
        F32Target::Margin => style.margin = Some(value),
        F32Target::MarginX => style.margin_x = Some(value),
        F32Target::MarginY => style.margin_y = Some(value),
        F32Target::MarginTop => style.margin_top = Some(value),
        F32Target::MarginRight => style.margin_right = Some(value),
        F32Target::MarginBottom => style.margin_bottom = Some(value),
        F32Target::MarginLeft => style.margin_left = Some(value),
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

fn parse_bracket_f32(value: &str) -> Option<f32> {
    value
        .strip_suffix("px]")
        .or_else(|| value.strip_suffix(']'))
        .and_then(|value| value.parse::<f32>().ok())
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
