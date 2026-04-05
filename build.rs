use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

const TAILWIND_THEME_COLORS_PATH: &str = "tailwind/theme-colors-v4.2.2.css";

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

    let theme_colors = fs::read_to_string(TAILWIND_THEME_COLORS_PATH)
        .map_err(|error| format!("failed to read {TAILWIND_THEME_COLORS_PATH}: {error}"))?;
    let generated = collect_generated_colors(&theme_colors)?;

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").map_err(|error| error.to_string())?);
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

fn generate_match_arms(colors: &[GeneratedColor]) -> String {
    let mut output = String::new();
    for color in colors {
        let _ = writeln!(
            output,
            "ColorToken::{} => Color::from_rgb(0x{:02x}, 0x{:02x}, 0x{:02x}),",
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
    output.push_str("    Custom(u8, u8, u8, u8),\n");
    output.push_str("}\n\n");

    output.push_str("impl ColorToken {\n");
    output.push_str("    pub fn to_skia(self) -> Color {\n");
    output.push_str("        match self {\n");
    output.push_str(&indent_block(&generate_match_arms(colors), 3));
    output.push_str("            ColorToken::Primary => ColorToken::Blue.to_skia(),\n");
    output
        .push_str("            ColorToken::Custom(r, g, b, a) => Color::from_argb(a, r, g, b),\n");
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
    output.push_str("    } else {\n");
    output.push_str("        tailwind_color_from_class_suffix(name)\n");
    output.push_str("    }\n");
    output.push_str("}\n\n");
    output.push_str(
        "pub(crate) fn color_token_from_script_name(name: &str) -> Option<ColorToken> {\n",
    );
    output.push_str("    if name == \"primary\" {\n");
    output.push_str("        Some(ColorToken::Primary)\n");
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
        "crate::nodes::Div",
        "crate::nodes::Image",
        "crate::nodes::Text",
        "crate::nodes::Video",
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
