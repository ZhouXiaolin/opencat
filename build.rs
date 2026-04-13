use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const TAILWIND_THEME_COLORS_PATH: &str = "tailwind/theme-colors-v4.2.2.css";
const LUCIDE_ICONS_DIR: &str = "lucide";

#[derive(Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Clone)]
struct GeneratedColor {
    variant: String,
    class_suffix: String,
    script_names: Vec<String>,
    method_suffix: String,
    rgb: Rgb,
}

fn main() {
    if let Err(error) = try_main() {
        panic!("failed to generate tailwind color code: {error}");
    }
}

fn try_main() -> Result<(), String> {
    println!("cargo:rerun-if-changed={TAILWIND_THEME_COLORS_PATH}");
    println!("cargo:rerun-if-changed={LUCIDE_ICONS_DIR}");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").map_err(|error| error.to_string())?);

    let theme_colors = fs::read_to_string(TAILWIND_THEME_COLORS_PATH)
        .map_err(|error| format!("failed to read {TAILWIND_THEME_COLORS_PATH}: {error}"))?;
    let generated = collect_generated_colors(&theme_colors)?;

    fs::write(
        out_dir.join("tailwind_color_items.rs"),
        generate_items(&generated),
    )
    .map_err(|error| format!("failed to write generated color items: {error}"))?;
    fs::write(
        out_dir.join("tailwind_color_inherent_impls.rs"),
        generate_inherent_impls(&generated),
    )
    .map_err(|error| format!("failed to write generated color inherent impls: {error}"))?;
    fs::write(
        out_dir.join("tailwind_jsonl_rules.rs"),
        generate_tailwind_jsonl_rules(),
    )
    .map_err(|error| format!("failed to write generated jsonl tailwind rules: {error}"))?;

    generate_lucide_icons(&out_dir)?;

    Ok(())
}

fn collect_generated_colors(input: &str) -> Result<Vec<GeneratedColor>, String> {
    let mut direct_colors = Vec::new();
    let mut family_order = Vec::new();
    let mut family_index = HashMap::<String, usize>::new();
    let mut families = Vec::<(String, BTreeMap<u16, Rgb>)>::new();

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if !line.starts_with("--color-") {
            continue;
        }

        let Some((name, value)) = line
            .strip_prefix("--color-")
            .and_then(|line| line.strip_suffix(';'))
            .and_then(|line| line.split_once(':'))
        else {
            return Err(format!("invalid color declaration: {line}"));
        };

        let name = name.trim();
        let value = value.trim();
        let rgb = parse_color(value)?;

        if let Some((family, scale)) = split_family_scale(name) {
            let entry_index = if let Some(index) = family_index.get(family).copied() {
                index
            } else {
                let index = families.len();
                family_index.insert(family.to_string(), index);
                family_order.push(family.to_string());
                families.push((family.to_string(), BTreeMap::new()));
                index
            };
            families[entry_index].1.insert(scale, rgb);
        } else {
            direct_colors.push(GeneratedColor {
                variant: variant_name(name),
                class_suffix: name.to_string(),
                script_names: vec![script_name(name)],
                method_suffix: method_suffix(name),
                rgb,
            });
        }
    }

    let mut generated = direct_colors;
    for family in family_order {
        let (_, shades) = families
            .iter()
            .find(|(name, _)| name == &family)
            .ok_or_else(|| format!("missing collected family {family}"))?;

        if let Some(rgb) = shades.get(&500).copied() {
            generated.push(GeneratedColor {
                variant: variant_name(&family),
                class_suffix: family.clone(),
                script_names: vec![script_name(&family)],
                method_suffix: method_suffix(&family),
                rgb,
            });
        }

        for (scale, rgb) in shades {
            let class_suffix = format!("{family}-{scale}");
            generated.push(GeneratedColor {
                variant: variant_name(&class_suffix),
                class_suffix: class_suffix.clone(),
                script_names: vec![
                    script_name(&class_suffix),
                    method_suffix(&class_suffix),
                    class_suffix.clone(),
                ],
                method_suffix: method_suffix(&class_suffix),
                rgb: *rgb,
            });
        }
    }

    Ok(generated)
}

fn split_family_scale(name: &str) -> Option<(&str, u16)> {
    let (family, scale) = name.rsplit_once('-')?;
    if scale.chars().all(|char| char.is_ascii_digit()) {
        Some((family, scale.parse().ok()?))
    } else {
        None
    }
}

fn parse_color(value: &str) -> Result<Rgb, String> {
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex);
    }
    if let Some(oklch) = value
        .strip_prefix("oklch(")
        .and_then(|value| value.strip_suffix(')'))
    {
        return parse_oklch_color(oklch);
    }
    Err(format!("unsupported color value: {value}"))
}

fn parse_hex_color(hex: &str) -> Result<Rgb, String> {
    match hex.len() {
        3 => {
            let r = parse_hex_nibble(hex.as_bytes()[0])?;
            let g = parse_hex_nibble(hex.as_bytes()[1])?;
            let b = parse_hex_nibble(hex.as_bytes()[2])?;
            Ok(Rgb {
                r: r * 17,
                g: g * 17,
                b: b * 17,
            })
        }
        6 => Ok(Rgb {
            r: u8::from_str_radix(&hex[0..2], 16)
                .map_err(|error| format!("invalid hex color component {hex}: {error}"))?,
            g: u8::from_str_radix(&hex[2..4], 16)
                .map_err(|error| format!("invalid hex color component {hex}: {error}"))?,
            b: u8::from_str_radix(&hex[4..6], 16)
                .map_err(|error| format!("invalid hex color component {hex}: {error}"))?,
        }),
        _ => Err(format!("unsupported hex color length: {hex}")),
    }
}

fn parse_hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex nibble {}", byte as char)),
    }
}

fn parse_oklch_color(value: &str) -> Result<Rgb, String> {
    let mut parts = value.split_whitespace();
    let lightness = parts
        .next()
        .ok_or_else(|| format!("missing oklch lightness: {value}"))?;
    let chroma = parts
        .next()
        .ok_or_else(|| format!("missing oklch chroma: {value}"))?;
    let hue = parts
        .next()
        .ok_or_else(|| format!("missing oklch hue: {value}"))?;

    let lightness = lightness
        .strip_suffix('%')
        .ok_or_else(|| format!("expected percent lightness in {value}"))?
        .parse::<f64>()
        .map_err(|error| format!("invalid oklch lightness {value}: {error}"))?
        / 100.0;
    let chroma = chroma
        .parse::<f64>()
        .map_err(|error| format!("invalid oklch chroma {value}: {error}"))?;
    let hue = hue
        .parse::<f64>()
        .map_err(|error| format!("invalid oklch hue {value}: {error}"))?;

    Ok(oklch_to_srgb(lightness, chroma, hue))
}

fn oklch_to_srgb(lightness: f64, chroma: f64, hue_degrees: f64) -> Rgb {
    let hue_radians = hue_degrees.to_radians();
    let a = chroma * hue_radians.cos();
    let b = chroma * hue_radians.sin();

    let l = lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m = lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s = lightness - 0.089_484_177_5 * a - 1.291_485_548 * b;

    let l = l * l * l;
    let m = m * m * m;
    let s = s * s * s;

    let red = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
    let green = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
    let blue = -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s;

    Rgb {
        r: srgb_channel(red),
        g: srgb_channel(green),
        b: srgb_channel(blue),
    }
}

fn srgb_channel(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let value = if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

fn variant_name(name: &str) -> String {
    let mut result = String::new();
    for chunk in name.split('-') {
        if chunk.is_empty() {
            continue;
        }
        if chunk.chars().all(|char| char.is_ascii_digit()) {
            result.push_str(chunk);
            continue;
        }

        let mut chars = chunk.chars();
        if let Some(first) = chars.next() {
            result.push(first.to_ascii_uppercase());
            result.extend(chars);
        }
    }
    result
}

fn script_name(name: &str) -> String {
    name.replace('-', "")
}

fn method_suffix(name: &str) -> String {
    name.replace('-', "_")
}

fn generate_variants(colors: &[GeneratedColor]) -> String {
    let mut output = String::new();
    for color in colors {
        let _ = writeln!(output, "{},", color.variant);
    }
    output
}

fn generate_rgba_match_arms(colors: &[GeneratedColor]) -> String {
    let mut output = String::new();
    for color in colors {
        let _ = writeln!(
            output,
            "ColorToken::{} => (0x{:02x}, 0x{:02x}, 0x{:02x}, 0xff),",
            color.variant, color.rgb.r, color.rgb.g, color.rgb.b
        );
    }
    output
}

fn generate_impls(colors: &[GeneratedColor]) -> String {
    let mut output = String::new();

    output.push_str("pub(crate) fn tailwind_color_from_class_suffix(name: &str) -> Option<ColorToken> {\n    match name {\n");
    for color in colors {
        let _ = writeln!(
            output,
            "        {:?} => Some(ColorToken::{}),",
            color.class_suffix, color.variant
        );
    }
    output.push_str("        _ => None,\n    }\n}\n\n");

    output.push_str("pub(crate) fn tailwind_color_from_script_name(name: &str) -> Option<ColorToken> {\n    match name {\n");
    for color in colors {
        let mut script_names = color.script_names.clone();
        script_names.sort();
        script_names.dedup();
        let patterns = script_names
            .into_iter()
            .map(|name| format!("{name:?}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let _ = writeln!(
            output,
            "        {patterns} => Some(ColorToken::{}),",
            color.variant
        );
    }
    output.push_str("        _ => None,\n    }\n}\n\n");

    output
}

fn generate_items(colors: &[GeneratedColor]) -> String {
    let mut output = String::new();

    output.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n");
    output.push_str("pub enum ColorToken {\n");
    output.push_str(&indent_block(&generate_variants(colors), 1));
    output.push_str("    Primary,\n");
    output.push_str("    Transparent,\n");
    output.push_str("    Custom(u8, u8, u8, u8),\n");
    output.push_str("}\n\n");

    output.push_str("impl ColorToken {\n");
    output.push_str("    pub fn rgba(self) -> (u8, u8, u8, u8) {\n");
    output.push_str("        match self {\n");
    output.push_str(&indent_block(&generate_rgba_match_arms(colors), 3));
    output.push_str("            ColorToken::Primary => ColorToken::Blue.rgba(),\n");
    output.push_str("            ColorToken::Transparent => (0, 0, 0, 0),\n");
    output.push_str("            ColorToken::Custom(r, g, b, a) => (r, g, b, a),\n");
    output.push_str("        }\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");

    output.push_str(&generate_impls(colors));
    output.push('\n');
    output.push_str(
        "pub(crate) fn color_token_from_class_suffix(name: &str) -> Option<ColorToken> {\n",
    );
    output.push_str("    if name == \"primary\" {\n");
    output.push_str("        Some(ColorToken::Primary)\n");
    output.push_str("    } else if name == \"transparent\" {\n");
    output.push_str("        Some(ColorToken::Transparent)\n");
    output.push_str("    } else {\n");
    output.push_str("        tailwind_color_from_class_suffix(name)\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
    output.push_str(
        "pub(crate) fn color_token_from_script_name(name: &str) -> Option<ColorToken> {\n",
    );
    output.push_str("    if name == \"primary\" {\n");
    output.push_str("        Some(ColorToken::Primary)\n");
    output.push_str("    } else if name == \"transparent\" {\n");
    output.push_str("        Some(ColorToken::Transparent)\n");
    output.push_str("    } else {\n");
    output.push_str("        tailwind_color_from_script_name(name)\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");

    output
}

fn indent_block(block: &str, level: usize) -> String {
    let indent = "    ".repeat(level);
    let mut output = String::new();
    for line in block.lines() {
        if line.is_empty() {
            output.push('\n');
        } else {
            let _ = writeln!(output, "{indent}{line}");
        }
    }
    output
}

fn generate_inherent_impls(colors: &[GeneratedColor]) -> String {
    let mut output = String::new();
    for ty in [
        "crate::scene::primitives::Div",
        "crate::scene::primitives::Image",
        "crate::scene::primitives::Text",
        "crate::scene::primitives::Video",
        "crate::scene::primitives::Lucide",
    ] {
        let _ = writeln!(output, "impl {ty} {{");
        for color in colors {
            let _ = writeln!(
                output,
                "    pub fn bg_{}(self) -> Self {{ self.bg(crate::style::ColorToken::{}) }}",
                color.method_suffix, color.variant
            );
            let _ = writeln!(
                output,
                "    pub fn text_{}(self) -> Self {{ self.text_color(crate::style::ColorToken::{}) }}",
                color.method_suffix, color.variant
            );
            let _ = writeln!(
                output,
                "    pub fn border_{}(self) -> Self {{ self.border_color(crate::style::ColorToken::{}) }}",
                color.method_suffix, color.variant
            );
        }
        output.push_str("}\n\n");
    }
    output
}

fn generate_tailwind_jsonl_rules() -> String {
    const EXACT_CLASS_RULE_SPECS: &[(&str, &str)] = &[
        ("relative", "ExactClassAction::Position(Position::Relative)"),
        ("absolute", "ExactClassAction::Position(Position::Absolute)"),
        ("flex", "ExactClassAction::Flex"),
        (
            "flex-row",
            "ExactClassAction::FlexDirection(FlexDirection::Row)",
        ),
        (
            "flex-col",
            "ExactClassAction::FlexDirection(FlexDirection::Col)",
        ),
        (
            "flex-row-reverse",
            "ExactClassAction::FlexDirection(FlexDirection::RowReverse)",
        ),
        (
            "flex-col-reverse",
            "ExactClassAction::FlexDirection(FlexDirection::ColReverse)",
        ),
        (
            "flex-column",
            "ExactClassAction::FlexDirection(FlexDirection::Col)",
        ),
        ("flex-wrap", "ExactClassAction::FlexWrap(FlexWrap::Wrap)"),
        (
            "flex-wrap-reverse",
            "ExactClassAction::FlexWrap(FlexWrap::WrapReverse)",
        ),
        (
            "flex-nowrap",
            "ExactClassAction::FlexWrap(FlexWrap::NoWrap)",
        ),
        (
            "justify-start",
            "ExactClassAction::JustifyContent(JustifyContent::Start)",
        ),
        (
            "justify-center",
            "ExactClassAction::JustifyContent(JustifyContent::Center)",
        ),
        (
            "justify-end",
            "ExactClassAction::JustifyContent(JustifyContent::End)",
        ),
        (
            "justify-between",
            "ExactClassAction::JustifyContent(JustifyContent::Between)",
        ),
        (
            "justify-around",
            "ExactClassAction::JustifyContent(JustifyContent::Around)",
        ),
        (
            "justify-evenly",
            "ExactClassAction::JustifyContent(JustifyContent::Evenly)",
        ),
        (
            "justify-stretch",
            "ExactClassAction::JustifyContent(JustifyContent::Stretch)",
        ),
        (
            "justify-center-safe",
            "ExactClassAction::JustifyContent(JustifyContent::Center)",
        ),
        (
            "justify-end-safe",
            "ExactClassAction::JustifyContent(JustifyContent::End)",
        ),
        (
            "items-start",
            "ExactClassAction::AlignItems(AlignItems::Start)",
        ),
        (
            "items-center",
            "ExactClassAction::AlignItems(AlignItems::Center)",
        ),
        ("items-end", "ExactClassAction::AlignItems(AlignItems::End)"),
        (
            "items-center-safe",
            "ExactClassAction::AlignItems(AlignItems::Center)",
        ),
        (
            "items-end-safe",
            "ExactClassAction::AlignItems(AlignItems::End)",
        ),
        (
            "items-baseline",
            "ExactClassAction::AlignItems(AlignItems::Baseline)",
        ),
        (
            "items-baseline-last",
            "ExactClassAction::AlignItems(AlignItems::Baseline)",
        ),
        (
            "items-stretch",
            "ExactClassAction::AlignItems(AlignItems::Stretch)",
        ),
        (
            "content-start",
            "ExactClassAction::AlignContent(JustifyContent::Start)",
        ),
        (
            "content-center",
            "ExactClassAction::AlignContent(JustifyContent::Center)",
        ),
        (
            "content-center-safe",
            "ExactClassAction::AlignContent(JustifyContent::Center)",
        ),
        (
            "content-end",
            "ExactClassAction::AlignContent(JustifyContent::End)",
        ),
        (
            "content-end-safe",
            "ExactClassAction::AlignContent(JustifyContent::End)",
        ),
        (
            "content-between",
            "ExactClassAction::AlignContent(JustifyContent::Between)",
        ),
        (
            "content-around",
            "ExactClassAction::AlignContent(JustifyContent::Around)",
        ),
        (
            "content-evenly",
            "ExactClassAction::AlignContent(JustifyContent::Evenly)",
        ),
        (
            "content-stretch",
            "ExactClassAction::AlignContent(JustifyContent::Stretch)",
        ),
        ("content-normal", "ExactClassAction::Noop"),
        ("content-baseline", "ExactClassAction::Noop"),
        (
            "place-content-start",
            "ExactClassAction::PlaceContent(JustifyContent::Start)",
        ),
        (
            "place-content-center",
            "ExactClassAction::PlaceContent(JustifyContent::Center)",
        ),
        (
            "place-content-center-safe",
            "ExactClassAction::PlaceContent(JustifyContent::Center)",
        ),
        (
            "place-content-end",
            "ExactClassAction::PlaceContent(JustifyContent::End)",
        ),
        (
            "place-content-end-safe",
            "ExactClassAction::PlaceContent(JustifyContent::End)",
        ),
        (
            "place-content-between",
            "ExactClassAction::PlaceContent(JustifyContent::Between)",
        ),
        (
            "place-content-around",
            "ExactClassAction::PlaceContent(JustifyContent::Around)",
        ),
        (
            "place-content-evenly",
            "ExactClassAction::PlaceContent(JustifyContent::Evenly)",
        ),
        (
            "place-content-stretch",
            "ExactClassAction::PlaceContent(JustifyContent::Stretch)",
        ),
        ("place-content-baseline", "ExactClassAction::Noop"),
        (
            "place-items-start",
            "ExactClassAction::AlignItems(AlignItems::Start)",
        ),
        (
            "place-items-end",
            "ExactClassAction::AlignItems(AlignItems::End)",
        ),
        (
            "place-items-end-safe",
            "ExactClassAction::AlignItems(AlignItems::End)",
        ),
        (
            "place-items-center",
            "ExactClassAction::AlignItems(AlignItems::Center)",
        ),
        (
            "place-items-center-safe",
            "ExactClassAction::AlignItems(AlignItems::Center)",
        ),
        (
            "place-items-baseline",
            "ExactClassAction::AlignItems(AlignItems::Baseline)",
        ),
        (
            "place-items-stretch",
            "ExactClassAction::AlignItems(AlignItems::Stretch)",
        ),
        ("self-auto", "ExactClassAction::Noop"),
        (
            "self-start",
            "ExactClassAction::AlignSelf(AlignItems::Start)",
        ),
        ("self-end", "ExactClassAction::AlignSelf(AlignItems::End)"),
        (
            "self-end-safe",
            "ExactClassAction::AlignSelf(AlignItems::End)",
        ),
        (
            "self-center",
            "ExactClassAction::AlignSelf(AlignItems::Center)",
        ),
        (
            "self-center-safe",
            "ExactClassAction::AlignSelf(AlignItems::Center)",
        ),
        (
            "self-baseline",
            "ExactClassAction::AlignSelf(AlignItems::Baseline)",
        ),
        (
            "self-baseline-last",
            "ExactClassAction::AlignSelf(AlignItems::Baseline)",
        ),
        (
            "self-stretch",
            "ExactClassAction::AlignSelf(AlignItems::Stretch)",
        ),
        (
            "object-contain",
            "ExactClassAction::ObjectFit(ObjectFit::Contain)",
        ),
        (
            "object-cover",
            "ExactClassAction::ObjectFit(ObjectFit::Cover)",
        ),
        (
            "object-fill",
            "ExactClassAction::ObjectFit(ObjectFit::Fill)",
        ),
        (
            "font-light",
            "ExactClassAction::FontWeight(FontWeight::Light)",
        ),
        (
            "font-normal",
            "ExactClassAction::FontWeight(FontWeight::Normal)",
        ),
        (
            "font-medium",
            "ExactClassAction::FontWeight(FontWeight::Medium)",
        ),
        (
            "font-semibold",
            "ExactClassAction::FontWeight(FontWeight::SemiBold)",
        ),
        (
            "font-bold",
            "ExactClassAction::FontWeight(FontWeight::Bold)",
        ),
        ("shadow-sm", "ExactClassAction::Shadow(ShadowStyle::SM)"),
        ("shadow-md", "ExactClassAction::Shadow(ShadowStyle::MD)"),
        ("shadow-lg", "ExactClassAction::Shadow(ShadowStyle::LG)"),
        ("shadow-xl", "ExactClassAction::Shadow(ShadowStyle::XL)"),
        ("rounded-none", "ExactClassAction::BorderRadius(0.0)"),
        ("rounded-sm", "ExactClassAction::BorderRadius(4.0)"),
        ("rounded", "ExactClassAction::BorderRadius(8.0)"),
        ("rounded-md", "ExactClassAction::BorderRadius(8.0)"),
        ("rounded-lg", "ExactClassAction::BorderRadius(16.0)"),
        ("rounded-xl", "ExactClassAction::BorderRadius(24.0)"),
        ("rounded-2xl", "ExactClassAction::BorderRadius(32.0)"),
        ("rounded-full", "ExactClassAction::BorderRadius(9999.0)"),
        ("border", "ExactClassAction::BorderWidth(1.0)"),
        ("border-b", "ExactClassAction::BorderWidth(1.0)"),
        ("border-t", "ExactClassAction::BorderWidth(1.0)"),
        ("overflow-hidden", "ExactClassAction::OverflowHidden"),
        ("pointer-events-none", "ExactClassAction::Noop"),
        ("inset-0", "ExactClassAction::InsetZero"),
        (
            "bg-gradient-to-r",
            "ExactClassAction::BgGradientDirection(GradientDirection::ToRight)",
        ),
        (
            "bg-gradient-to-l",
            "ExactClassAction::BgGradientDirection(GradientDirection::ToLeft)",
        ),
        (
            "bg-gradient-to-b",
            "ExactClassAction::BgGradientDirection(GradientDirection::ToBottom)",
        ),
        (
            "bg-gradient-to-t",
            "ExactClassAction::BgGradientDirection(GradientDirection::ToTop)",
        ),
        (
            "bg-gradient-to-br",
            "ExactClassAction::BgGradientDirection(GradientDirection::ToBottomRight)",
        ),
        ("shrink-0", "ExactClassAction::FlexShrink(0.0)"),
        ("flex-shrink-0", "ExactClassAction::FlexShrink(0.0)"),
        ("flex-1", "ExactClassAction::FlexGrow(1.0)"),
        ("grow", "ExactClassAction::FlexGrow(1.0)"),
        ("text-left", "ExactClassAction::TextAlign(TextAlign::Left)"),
        (
            "text-center",
            "ExactClassAction::TextAlign(TextAlign::Center)",
        ),
        (
            "text-right",
            "ExactClassAction::TextAlign(TextAlign::Right)",
        ),
        ("w-full", "ExactClassAction::WidthFull"),
        ("w-screen", "ExactClassAction::WidthFull"),
        ("w-svw", "ExactClassAction::WidthFull"),
        ("w-lvw", "ExactClassAction::WidthFull"),
        ("w-dvw", "ExactClassAction::WidthFull"),
        ("h-full", "ExactClassAction::HeightFull"),
        ("h-screen", "ExactClassAction::HeightFull"),
        ("h-svh", "ExactClassAction::HeightFull"),
        ("h-lvh", "ExactClassAction::HeightFull"),
        ("h-dvh", "ExactClassAction::HeightFull"),
        ("leading-none", "ExactClassAction::LineHeight(1.0)"),
        ("leading-tight", "ExactClassAction::LineHeight(1.25)"),
        ("leading-snug", "ExactClassAction::LineHeight(1.375)"),
        ("leading-normal", "ExactClassAction::LineHeight(1.5)"),
        ("leading-relaxed", "ExactClassAction::LineHeight(1.625)"),
        ("leading-loose", "ExactClassAction::LineHeight(2.0)"),
        ("tracking-tight", "ExactClassAction::LetterSpacing(-0.5)"),
        ("tracking-normal", "ExactClassAction::LetterSpacing(0.0)"),
        ("tracking-wide", "ExactClassAction::LetterSpacing(0.5)"),
        ("tracking-wider", "ExactClassAction::LetterSpacing(1.0)"),
        (
            "uppercase",
            "ExactClassAction::TextTransform(TextTransform::Uppercase)",
        ),
        ("blur-none", "ExactClassAction::BlurSigma(0.0)"),
        ("blur-sm", "ExactClassAction::BlurSigma(4.0)"),
        ("blur", "ExactClassAction::BlurSigma(8.0)"),
        ("blur-md", "ExactClassAction::BlurSigma(8.0)"),
        ("blur-lg", "ExactClassAction::BlurSigma(16.0)"),
        ("blur-xl", "ExactClassAction::BlurSigma(24.0)"),
        ("blur-2xl", "ExactClassAction::BlurSigma(40.0)"),
        ("blur-3xl", "ExactClassAction::BlurSigma(64.0)"),
    ];

    const BRACKET_F32_RULE_SPECS: &[(&str, &str)] = &[
        ("gap-[", "Gap"),
        ("w-[", "Width"),
        ("h-[", "Height"),
        ("left-[", "InsetLeft"),
        ("top-[", "InsetTop"),
        ("right-[", "InsetRight"),
        ("bottom-[", "InsetBottom"),
        ("text-[", "TextPx"),
        ("p-[", "Padding"),
        ("px-[", "PaddingX"),
        ("py-[", "PaddingY"),
        ("m-[", "Margin"),
        ("mx-[", "MarginX"),
        ("my-[", "MarginY"),
        ("rounded-[", "BorderRadius"),
        ("border-[", "BorderWidth"),
        ("opacity-[", "OpacityClamped"),
        ("blur-[", "BlurSigma"),
        ("grow-[", "FlexGrow"),
        ("shrink-[", "FlexShrink"),
        ("pt-[", "PaddingTop"),
        ("pr-[", "PaddingRight"),
        ("pb-[", "PaddingBottom"),
        ("pl-[", "PaddingLeft"),
        ("mt-[", "MarginTop"),
        ("mr-[", "MarginRight"),
        ("mb-[", "MarginBottom"),
        ("ml-[", "MarginLeft"),
    ];
    const SIGNED_BRACKET_F32_RULE_SPECS: &[(&str, &str, &str)] = &[
        ("left-[", "-left-[", "InsetLeft"),
        ("top-[", "-top-[", "InsetTop"),
        ("right-[", "-right-[", "InsetRight"),
        ("bottom-[", "-bottom-[", "InsetBottom"),
    ];
    const SPACING_SCALE_RULE_SPECS: &[(&str, &str)] = &[
        ("gap-", "Gap"),
        ("w-", "Width"),
        ("h-", "Height"),
        ("left-", "InsetLeft"),
        ("top-", "InsetTop"),
        ("right-", "InsetRight"),
        ("bottom-", "InsetBottom"),
        ("p-", "Padding"),
        ("px-", "PaddingX"),
        ("py-", "PaddingY"),
        ("pt-", "PaddingTop"),
        ("pr-", "PaddingRight"),
        ("pb-", "PaddingBottom"),
        ("pl-", "PaddingLeft"),
        ("m-", "Margin"),
        ("mx-", "MarginX"),
        ("my-", "MarginY"),
        ("mt-", "MarginTop"),
        ("mr-", "MarginRight"),
        ("mb-", "MarginBottom"),
        ("ml-", "MarginLeft"),
    ];
    const COLOR_PREFIX_RULE_SPECS: &[(&str, &str)] = &[
        ("bg-", "Bg"),
        ("text-", "Text"),
        ("border-", "Border"),
        ("from-", "GradientFrom"),
        ("via-", "GradientVia"),
        ("to-", "GradientTo"),
    ];
    const TEXT_SIZE_RULE_SPECS: &[(&str, f32)] = &[
        ("xs", 12.0),
        ("sm", 14.0),
        ("base", 16.0),
        ("lg", 18.0),
        ("xl", 20.0),
        ("2xl", 24.0),
        ("3xl", 30.0),
        ("4xl", 36.0),
        ("5xl", 48.0),
        ("6xl", 60.0),
        ("7xl", 72.0),
        ("8xl", 96.0),
        ("9xl", 128.0),
    ];

    let mut output = String::new();
    output.push_str("// @generated by build.rs; do not edit.\n\n");

    output.push_str("const EXACT_CLASS_RULES: &[(&str, ExactClassAction)] = &[\n");
    for (class_name, action) in EXACT_CLASS_RULE_SPECS {
        let _ = writeln!(output, "    ({class_name:?}, {action}),");
    }
    output.push_str("];\n\n");

    output.push_str("const BRACKET_F32_RULES: &[(&str, F32Target)] = &[\n");
    for (prefix, target) in BRACKET_F32_RULE_SPECS {
        let _ = writeln!(output, "    ({prefix:?}, F32Target::{target}),");
    }
    output.push_str("];\n\n");

    output.push_str("const SIGNED_BRACKET_F32_RULES: &[(&str, &str, F32Target)] = &[\n");
    for (positive_prefix, negative_prefix, target) in SIGNED_BRACKET_F32_RULE_SPECS {
        let _ = writeln!(
            output,
            "    ({positive_prefix:?}, {negative_prefix:?}, F32Target::{target}),"
        );
    }
    output.push_str("];\n\n");

    output.push_str("const SPACING_SCALE_RULES: &[(&str, F32Target)] = &[\n");
    for (prefix, target) in SPACING_SCALE_RULE_SPECS {
        let _ = writeln!(output, "    ({prefix:?}, F32Target::{target}),");
    }
    output.push_str("];\n\n");

    output.push_str("const COLOR_PREFIX_RULES: &[(&str, ColorTarget)] = &[\n");
    for (prefix, target) in COLOR_PREFIX_RULE_SPECS {
        let _ = writeln!(output, "    ({prefix:?}, ColorTarget::{target}),");
    }
    output.push_str("];\n\n");

    output.push_str("const TAILWIND_TEXT_SIZE_RULES: &[(&str, f32)] = &[\n");
    for (name, px) in TEXT_SIZE_RULE_SPECS {
        let _ = writeln!(output, "    ({name:?}, {px:.1}),");
    }
    output.push_str("];\n");

    output
}

// ── Lucide icon codegen ──────────────────────────────────────────────────────

struct LucideIcon {
    name: String,
    paths: Vec<String>,
}

fn generate_lucide_icons(out_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(LUCIDE_ICONS_DIR)
        .map_err(|error| format!("failed to read lucide dir: {error}"))?;
    let mut icons: Vec<LucideIcon> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read dir entry: {error}"))?;
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "svg") {
            continue;
        }
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let paths = extract_svg_paths(&content, &name)?;
        icons.push(LucideIcon { name, paths });
    }
    icons.sort_by(|a, b| a.name.cmp(&b.name));

    let mut output = String::new();
    output.push_str("/// Auto-generated Lucide icon path data.\n");
    output.push_str("/// Each icon maps to a list of SVG path data strings.\n\n");
    output.push_str("pub fn lucide_icon_paths(name: &str) -> Option<&'static [&'static str]> {\n");
    output.push_str("    match name {\n");
    for icon in &icons {
        let paths_literal = icon
            .paths
            .iter()
            .map(|p| format!("\"{}\"", p.escape_default().to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            output,
            "        \"{}\" => Some(&[{}]),",
            icon.name, paths_literal
        );
    }
    output.push_str("        _ => None,\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
    output.push_str("pub fn lucide_icon_names() -> &'static [&'static str] {\n");
    output.push_str("    &[\n");
    for icon in &icons {
        let _ = writeln!(output, "        \"{}\",", icon.name);
    }
    output.push_str("    ]\n");
    output.push_str("}\n");

    fs::write(out_dir.join("lucide_icons.rs"), output)
        .map_err(|error| format!("failed to write lucide_icons.rs: {error}"))?;
    Ok(())
}

fn extract_svg_paths(content: &str, file_name: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('<') && !line.starts_with("</") && !line.starts_with("<svg") {
            if let Some(d) = extract_attr(line, "d") {
                paths.push(d.to_string());
            } else if line.starts_with("<circle") {
                let cx: f32 = extract_attr(line, "cx")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid cx: {e}"))?;
                let cy: f32 = extract_attr(line, "cy")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid cy: {e}"))?;
                let r: f32 = extract_attr(line, "r")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid r: {e}"))?;
                paths.push(format!(
                    "M{cx},{cy} m{neg_r},0 a{r},{r} 0 1,0 {d},0 a{r},{r} 0 1,0 {neg_d},0",
                    d = r * 2.0,
                    neg_d = -r * 2.0,
                    neg_r = -r
                ));
            } else if line.starts_with("<line") {
                let x1: f32 = extract_attr(line, "x1")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid x1: {e}"))?;
                let y1: f32 = extract_attr(line, "y1")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid y1: {e}"))?;
                let x2: f32 = extract_attr(line, "x2")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid x2: {e}"))?;
                let y2: f32 = extract_attr(line, "y2")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid y2: {e}"))?;
                paths.push(format!("M{x1},{y1} L{x2},{y2}"));
            } else if line.starts_with("<rect") {
                let x: f32 = extract_attr(line, "x")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid x: {e}"))?;
                let y: f32 = extract_attr(line, "y")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid y: {e}"))?;
                let w: f32 = extract_attr(line, "width")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid width: {e}"))?;
                let h: f32 = extract_attr(line, "height")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|e| format!("lucide {file_name}: invalid height: {e}"))?;
                paths.push(format!(
                    "M{x},{y} L{x},{y2} L{x2},{y2} L{x2},{y} Z",
                    x2 = x + w,
                    y2 = y + h
                ));
            } else if line.starts_with("<polyline") || line.starts_with("<polygon") {
                let points_str = extract_attr(line, "points").unwrap_or("");
                let points: Vec<&str> = points_str.split_whitespace().collect();
                if points.len() >= 2 {
                    let mut d = format!("M{}", points[0]);
                    for p in &points[1..] {
                        let _ = write!(d, " L{p}");
                    }
                    if line.starts_with("<polygon") {
                        d.push_str(" Z");
                    }
                    paths.push(d);
                }
            }
        }
    }
    Ok(paths)
}

fn extract_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let prefix = format!("{attr}=\"");
    let start = tag.find(&prefix)?;
    let rest = &tag[start + prefix.len()..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}
