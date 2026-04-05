use std::collections::HashMap;

use serde::Deserialize;

use crate::nodes::{div, text};
use crate::style::{
    AlignItems, ColorToken, FlexDirection, FontWeight, JustifyContent, NodeStyle, ObjectFit,
    Position, TextAlign,
};
use crate::view::Node;

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
enum JsonLine {
    #[serde(rename = "composition")]
    Composition {
        width: i32,
        height: i32,
        fps: i32,
        frames: i32,
    },
    #[serde(rename = "script")]
    Script { content: String },
    #[serde(untagged)]
    Element {
        id: Option<u64>,
        #[serde(rename = "parentId")]
        parent_id: Option<u64>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        text: Option<String>,
    },
}

#[derive(Debug, Clone)]
struct ParsedElement {
    id: u64,
    parent_id: Option<u64>,
    style: NodeStyle,
    text: Option<String>,
}

pub struct ParsedComposition {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub frames: i32,
    pub root: Node,
    pub script: Option<String>,
}

pub fn parse(input: &str) -> anyhow::Result<ParsedComposition> {
    let mut width = 1920;
    let mut height = 1080;
    let mut fps = 30;
    let mut frames = 90;
    let mut script = None;
    let mut elements: Vec<ParsedElement> = Vec::new();

    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parsed: JsonLine = serde_json::from_str(line)?;
        match parsed {
            JsonLine::Composition {
                width: w,
                height: h,
                fps: f,
                frames: fs,
            } => {
                width = w;
                height = h;
                fps = f;
                frames = fs;
            }
            JsonLine::Script { content } => {
                script = Some(content);
            }
            JsonLine::Element {
                id,
                parent_id,
                class_name,
                text,
            } => {
                if let Some(id) = id {
                    let style = parse_class_name(class_name.as_deref().unwrap_or(""));
                    elements.push(ParsedElement {
                        id,
                        parent_id,
                        style,
                        text,
                    });
                }
            }
        }
    }

    let root = build_tree(&elements)?;

    Ok(ParsedComposition {
        width,
        height,
        fps,
        frames,
        root,
        script,
    })
}

fn build_tree(elements: &[ParsedElement]) -> anyhow::Result<Node> {
    let mut children_map: HashMap<u64, Vec<&ParsedElement>> = HashMap::new();
    let mut root_element = None;

    for el in elements {
        if el.parent_id.is_none() {
            if root_element.is_some() {
                return Err(anyhow::anyhow!("multiple root elements found"));
            }
            root_element = Some(el);
        } else {
            children_map
                .entry(el.parent_id.unwrap())
                .or_default()
                .push(el);
        }
    }

    let root = root_element.ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    let root_node = build_node(root, &children_map)?;

    Ok(root_node)
}

fn build_node(
    el: &ParsedElement,
    children_map: &HashMap<u64, Vec<&ParsedElement>>,
) -> anyhow::Result<Node> {
    let mut style = el.style.clone();
    style.data_id = Some(el.id.to_string());

    if el.text.is_some() {
        let mut text_node = text(el.text.as_ref().unwrap());
        text_node.style = style;
        return Ok(Node::new(text_node));
    }

    let mut div_node = div();
    div_node.style = style;

    if let Some(children) = children_map.get(&el.id) {
        for child in children {
            let child_node = build_node(child, children_map)?;
            div_node = div_node.child(child_node);
        }
    }

    Ok(Node::new(div_node))
}

fn parse_class_name(class_name: &str) -> NodeStyle {
    let mut style = NodeStyle::default();

    if class_name.is_empty() {
        return style;
    }

    let classes: Vec<&str> = class_name.split_whitespace().collect();

    for class in &classes {
        parse_single_class(class, &mut style);
    }

    style
}

fn parse_single_class(class: &str, style: &mut NodeStyle) {
    match class {
        // Position
        "relative" => style.position = Some(Position::Relative),
        "absolute" => style.position = Some(Position::Absolute),

        // Flex layout
        "flex" => {
            if style.flex_direction.is_none() {
                style.flex_direction = Some(FlexDirection::Row);
            }
        }
        "flex-row" => style.flex_direction = Some(FlexDirection::Row),
        "flex-col" | "flex-column" => style.flex_direction = Some(FlexDirection::Col),

        // Justify content
        "justify-start" => style.justify_content = Some(JustifyContent::Start),
        "justify-center" => style.justify_content = Some(JustifyContent::Center),
        "justify-end" => style.justify_content = Some(JustifyContent::End),
        "justify-between" => style.justify_content = Some(JustifyContent::Between),
        "justify-around" => style.justify_content = Some(JustifyContent::Around),
        "justify-evenly" => style.justify_content = Some(JustifyContent::Evenly),

        // Align items
        "items-start" => style.align_items = Some(AlignItems::Start),
        "items-center" => style.align_items = Some(AlignItems::Center),
        "items-end" => style.align_items = Some(AlignItems::End),
        "items-stretch" => style.align_items = Some(AlignItems::Stretch),

        // Object fit
        "object-contain" => style.object_fit = Some(ObjectFit::Contain),
        "object-cover" => style.object_fit = Some(ObjectFit::Cover),
        "object-fill" => style.object_fit = Some(ObjectFit::Fill),

        // Font weight
        "font-normal" => style.font_weight = Some(FontWeight::Normal),
        "font-medium" => style.font_weight = Some(FontWeight::Medium),
        "font-semibold" => style.font_weight = Some(FontWeight::SemiBold),
        "font-bold" => style.font_weight = Some(FontWeight::Bold),

        // Shadows
        "shadow-sm" => style.shadow = Some(crate::style::ShadowStyle::SM),
        "shadow-md" => style.shadow = Some(crate::style::ShadowStyle::MD),
        "shadow-lg" => style.shadow = Some(crate::style::ShadowStyle::LG),
        "shadow-xl" => style.shadow = Some(crate::style::ShadowStyle::XL),

        // Border radius
        "rounded-none" => style.border_radius = Some(0.0),
        "rounded-sm" => style.border_radius = Some(4.0),
        "rounded" | "rounded-md" => style.border_radius = Some(8.0),
        "rounded-lg" => style.border_radius = Some(16.0),
        "rounded-xl" => style.border_radius = Some(24.0),
        "rounded-2xl" => style.border_radius = Some(32.0),
        "rounded-full" => style.border_radius = Some(9999.0),

        // Border
        "border" => style.border_width = Some(1.0),

        // Background colors
        "bg-white" => style.bg_color = Some(ColorToken::White),
        "bg-black" => style.bg_color = Some(ColorToken::Black),
        "bg-red" | "bg-red-500" => style.bg_color = Some(ColorToken::Red),
        "bg-green" | "bg-green-500" => style.bg_color = Some(ColorToken::Green),
        "bg-blue" | "bg-blue-500" => style.bg_color = Some(ColorToken::Blue),
        "bg-yellow" | "bg-yellow-500" => style.bg_color = Some(ColorToken::Yellow),
        "bg-orange" | "bg-orange-500" => style.bg_color = Some(ColorToken::Orange),
        "bg-purple" | "bg-purple-500" => style.bg_color = Some(ColorToken::Purple),
        "bg-pink" | "bg-pink-500" => style.bg_color = Some(ColorToken::Pink),
        "bg-gray" | "bg-gray-500" => style.bg_color = Some(ColorToken::Gray),
        "bg-slate-50" => style.bg_color = Some(ColorToken::Slate50),
        "bg-slate-200" => style.bg_color = Some(ColorToken::Slate200),
        "bg-slate-300" => style.bg_color = Some(ColorToken::Slate300),
        "bg-slate-400" => style.bg_color = Some(ColorToken::Slate400),
        "bg-slate-500" => style.bg_color = Some(ColorToken::Slate500),
        "bg-slate-600" => style.bg_color = Some(ColorToken::Slate600),
        "bg-slate-700" => style.bg_color = Some(ColorToken::Slate700),
        "bg-slate-900" => style.bg_color = Some(ColorToken::Slate900),
        "bg-primary" => style.bg_color = Some(ColorToken::Primary),

        // Text colors
        "text-white" => style.text_color = Some(ColorToken::White),
        "text-black" => style.text_color = Some(ColorToken::Black),
        "text-red" | "text-red-500" => style.text_color = Some(ColorToken::Red),
        "text-green" | "text-green-500" => style.text_color = Some(ColorToken::Green),
        "text-blue" | "text-blue-500" => style.text_color = Some(ColorToken::Blue),
        "text-yellow" | "text-yellow-500" => style.text_color = Some(ColorToken::Yellow),
        "text-orange" | "text-orange-500" => style.text_color = Some(ColorToken::Orange),
        "text-purple" | "text-purple-500" => style.text_color = Some(ColorToken::Purple),
        "text-pink" | "text-pink-500" => style.text_color = Some(ColorToken::Pink),
        "text-gray" | "text-gray-500" => style.text_color = Some(ColorToken::Gray),
        "text-slate-50" => style.text_color = Some(ColorToken::Slate50),
        "text-slate-200" => style.text_color = Some(ColorToken::Slate200),
        "text-slate-300" => style.text_color = Some(ColorToken::Slate300),
        "text-slate-400" => style.text_color = Some(ColorToken::Slate400),
        "text-slate-500" => style.text_color = Some(ColorToken::Slate500),
        "text-slate-600" => style.text_color = Some(ColorToken::Slate600),
        "text-slate-700" => style.text_color = Some(ColorToken::Slate700),
        "text-slate-900" => style.text_color = Some(ColorToken::Slate900),
        "text-primary" => style.text_color = Some(ColorToken::Primary),

        // Text alignment
        "text-left" => style.text_align = Some(TextAlign::Left),
        "text-center" => style.text_align = Some(TextAlign::Center),
        "text-right" => style.text_align = Some(TextAlign::Right),

        // Width/height shortcuts
        "w-full" => {
            style.width = None;
            style.width_full = true;
        }
        "h-full" => {
            style.height = None;
            style.height_full = true;
        }
        "grow" => style.flex_grow = Some(1.0),
        "border-t" => style.border_width = Some(1.0),

        // Line height / leading
        "leading-none" => style.line_height = Some(1.0),
        "leading-tight" => style.line_height = Some(1.25),
        "leading-snug" => style.line_height = Some(1.375),
        "leading-normal" => style.line_height = Some(1.5),
        "leading-relaxed" => style.line_height = Some(1.625),
        "leading-loose" => style.line_height = Some(2.0),

        _ => parse_arbitrary_class(class, style),
    }
}

fn parse_arbitrary_class(class: &str, style: &mut NodeStyle) {
    if let Some(value) = class.strip_prefix("gap-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.gap = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.gap = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("w-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.width = Some(n);
                style.width_full = false;
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.width = Some(n);
                style.width_full = false;
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("h-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.height = Some(n);
                style.height_full = false;
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.height = Some(n);
                style.height_full = false;
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("left-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_left = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_left = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("top-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_top = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_top = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("right-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_right = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_right = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("bottom-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_bottom = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.inset_bottom = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("text-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.text_px = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.text_px = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("p-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("px-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_x = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_x = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("py-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_y = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_y = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("m-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("mx-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_x = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_x = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("my-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_y = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_y = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("rounded-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.border_radius = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.border_radius = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("border-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.border_width = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.border_width = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("opacity-[") {
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.opacity = Some(n.clamp(0.0, 1.0));
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("border-color-[") {
        if let Some(v) = value.strip_suffix("]") {
            if let Some(c) = color_from_name(v) {
                style.border_color = Some(c);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("tracking-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.letter_spacing = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.letter_spacing = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("grow-[") {
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.flex_grow = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("leading-[") {
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.line_height = Some(n);
                return;
            }
        }
    }

    if let Some(color) = class.strip_prefix("bg-").and_then(color_from_name) {
        style.bg_color = Some(color);
        return;
    }

    if let Some(color) = class.strip_prefix("text-").and_then(color_from_name) {
        style.text_color = Some(color);
        return;
    }

    if let Some(color) = class.strip_prefix("border-").and_then(color_from_name) {
        style.border_color = Some(color);
        return;
    }

    if class.starts_with("gap-") {
        if let Ok(n) = class[4..].parse::<f32>() {
            style.gap = Some(n);
        }
    }

    if class.starts_with("w-") {
        if let Ok(n) = class[2..].parse::<f32>() {
            style.width = Some(n);
            style.width_full = false;
        }
    }

    if class.starts_with("h-") {
        if let Ok(n) = class[2..].parse::<f32>() {
            style.height = Some(n);
            style.height_full = false;
        }
    }

    if class.starts_with("text-") && !class.starts_with("text-[") {
        let after = &class[5..];
        if let Ok(n) = after.parse::<f32>() {
            style.text_px = Some(n);
        }
    }

    if class.starts_with("left-") {
        if let Ok(n) = class[5..].parse::<f32>() {
            style.inset_left = Some(n);
        }
    }

    if class.starts_with("top-") {
        if let Ok(n) = class[4..].parse::<f32>() {
            style.inset_top = Some(n);
        }
    }

    if class.starts_with("right-") {
        if let Ok(n) = class[6..].parse::<f32>() {
            style.inset_right = Some(n);
        }
    }

    if class.starts_with("bottom-") {
        if let Ok(n) = class[7..].parse::<f32>() {
            style.inset_bottom = Some(n);
        }
    }

    if class.starts_with("p-") {
        if let Ok(n) = class[2..].parse::<f32>() {
            style.padding = Some(n);
        }
    }

    if class.starts_with("px-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.padding_x = Some(n);
        }
    }

    if class.starts_with("py-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.padding_y = Some(n);
        }
    }

    if class.starts_with("m-") {
        if let Ok(n) = class[2..].parse::<f32>() {
            style.margin = Some(n);
        }
    }

    if class.starts_with("mx-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.margin_x = Some(n);
        }
    }

    if class.starts_with("my-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.margin_y = Some(n);
        }
    }

    if class.starts_with("rounded-") {
        if let Ok(n) = class[8..].parse::<f32>() {
            style.border_radius = Some(n);
        }
    }

    if class.starts_with("border-")
        && !class.starts_with("border-color-")
        && !class.starts_with("border-[")
    {
        if let Ok(n) = class[7..].parse::<f32>() {
            style.border_width = Some(n);
        }
    }

    if class.starts_with("opacity-") {
        if let Ok(n) = class[8..].parse::<f32>() {
            style.opacity = Some((n / 100.0).clamp(0.0, 1.0));
        }
    }

    if class.starts_with("grow-") {
        if let Ok(n) = class[5..].parse::<f32>() {
            style.flex_grow = Some(n);
        }
    }

    if class.starts_with("tracking-") {
        if let Ok(n) = class[9..].parse::<f32>() {
            style.letter_spacing = Some(n);
        }
    }
}

fn color_from_name(name: &str) -> Option<ColorToken> {
    match name {
        "white" => Some(ColorToken::White),
        "black" => Some(ColorToken::Black),
        "red" | "red-500" => Some(ColorToken::Red),
        "green" | "green-500" => Some(ColorToken::Green),
        "blue" | "blue-400" | "blue-500" => Some(ColorToken::Blue),
        "teal-400" => Some(ColorToken::Teal400),
        "teal-500" => Some(ColorToken::Teal500),
        "yellow" | "yellow-500" => Some(ColorToken::Yellow),
        "orange" | "orange-500" => Some(ColorToken::Orange),
        "purple" | "purple-500" => Some(ColorToken::Purple),
        "pink" | "pink-500" => Some(ColorToken::Pink),
        "gray" | "gray-500" => Some(ColorToken::Gray),
        "slate-50" => Some(ColorToken::Slate50),
        "slate-200" => Some(ColorToken::Slate200),
        "slate-300" => Some(ColorToken::Slate300),
        "slate-400" => Some(ColorToken::Slate400),
        "slate-500" => Some(ColorToken::Slate500),
        "slate-600" => Some(ColorToken::Slate600),
        "slate-700" => Some(ColorToken::Slate700),
        "slate-800" => Some(ColorToken::Slate800),
        "slate-900" => Some(ColorToken::Slate900),
        "primary" => Some(ColorToken::Primary),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, parse_class_name};
    use crate::style::{ColorToken, TextAlign};

    #[test]
    fn parser_maps_full_size_alignment_and_tailwind_colors() {
        let style = parse_class_name(
            "text-center w-full h-full text-teal-400 bg-slate-800 border-slate-700 leading-[1.8]",
        );

        assert_eq!(style.text_align, Some(TextAlign::Center));
        assert!(style.width_full);
        assert!(style.height_full);
        assert_eq!(style.text_color, Some(ColorToken::Teal400));
        assert_eq!(style.bg_color, Some(ColorToken::Slate800));
        assert_eq!(style.border_color, Some(ColorToken::Slate700));
        assert_eq!(style.line_height, Some(1.8));
    }

    #[test]
    fn parser_keeps_script_line() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":90}
{"id":1,"parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","content":"ctx.getNode('1').opacity(0.5);"}"#,
        )
        .expect("jsonl should parse");

        assert_eq!(parsed.width, 640);
        assert_eq!(parsed.height, 360);
        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('1').opacity(0.5);")
        );
    }

    #[test]
    fn parser_rejects_multiple_roots() {
        let err = parse(
            r#"{"id":1,"parentId":null,"type":"div","className":"","text":null}
{"id":2,"parentId":null,"type":"div","className":"","text":null}"#,
        )
        .err()
        .expect("multiple roots should fail");

        assert!(err.to_string().contains("multiple root"));
    }
}
