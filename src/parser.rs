use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::nodes::{ImageSource, OpenverseQuery, div, image, lucide, text, video};
use crate::style::{
    AlignItems, ColorToken, FlexDirection, FontWeight, GradientDirection, JustifyContent,
    NodeStyle, ObjectFit, Position, TextAlign, color_token_from_class_suffix,
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
    #[serde(rename = "div")]
    Div {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        text: String,
    },
    #[serde(rename = "image")]
    Image {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        path: Option<String>,
        url: Option<String>,
        query: Option<String>,
        #[serde(rename = "queryCount")]
        query_count: Option<usize>,
        #[serde(rename = "aspectRatio")]
        aspect_ratio: Option<String>,
    },
    #[serde(rename = "video")]
    Video {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        path: String,
    },
    #[serde(rename = "icon")]
    Icon {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        icon: String,
    },
}

#[derive(Debug, Clone)]
enum ParsedElementKind {
    Div,
    Text { content: String },
    Image { source: ImageSource },
    Icon { name: String },
    Video { path: PathBuf },
}

#[derive(Debug, Clone)]
struct ParsedElement {
    id: String,
    parent_id: Option<String>,
    style: NodeStyle,
    kind: ParsedElementKind,
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
            JsonLine::Div {
                id,
                parent_id,
                class_name,
            } => {
                let style = parse_class_name(class_name.as_deref().unwrap_or(""));
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    style,
                    kind: ParsedElementKind::Div,
                });
            }
            JsonLine::Text {
                id,
                parent_id,
                class_name,
                text,
            } => {
                let style = parse_class_name(class_name.as_deref().unwrap_or(""));
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    style,
                    kind: ParsedElementKind::Text { content: text },
                });
            }
            JsonLine::Image {
                id,
                parent_id,
                class_name,
                path,
                url,
                query,
                query_count,
                aspect_ratio,
            } => {
                let style = parse_class_name(class_name.as_deref().unwrap_or(""));
                let source = parse_image_source(path, url, query, query_count, aspect_ratio)?;
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    style,
                    kind: ParsedElementKind::Image { source },
                });
            }
            JsonLine::Video {
                id,
                parent_id,
                class_name,
                path,
            } => {
                let style = parse_class_name(class_name.as_deref().unwrap_or(""));
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    style,
                    kind: ParsedElementKind::Video {
                        path: PathBuf::from(path),
                    },
                });
            }
            JsonLine::Icon {
                id,
                parent_id,
                class_name,
                icon,
            } => {
                let style = parse_class_name(class_name.as_deref().unwrap_or(""));
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    style,
                    kind: ParsedElementKind::Icon { name: icon },
                });
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

fn parse_image_source(
    path: Option<String>,
    url: Option<String>,
    query: Option<String>,
    query_count: Option<usize>,
    aspect_ratio: Option<String>,
) -> anyhow::Result<ImageSource> {
    let sources = [
        path.as_ref().map(|_| "path"),
        url.as_ref().map(|_| "url"),
        query.as_ref().map(|_| "query"),
    ]
    .into_iter()
    .flatten()
    .count();

    if sources == 0 {
        return Err(anyhow::anyhow!(
            "image node requires one of: path, url, query"
        ));
    }

    if sources > 1 {
        return Err(anyhow::anyhow!(
            "image node accepts only one source: path, url, or query"
        ));
    }

    if let Some(path) = path {
        return Ok(ImageSource::Path(PathBuf::from(path)));
    }

    if let Some(url) = url {
        return Ok(ImageSource::Url(url));
    }

    let Some(query) = query else {
        return Err(anyhow::anyhow!(
            "image node requires one of: path, url, query"
        ));
    };

    Ok(ImageSource::Query(OpenverseQuery {
        query,
        count: query_count.unwrap_or(1).max(1),
        aspect_ratio,
    }))
}

fn build_tree(elements: &[ParsedElement]) -> anyhow::Result<Node> {
    let mut children_map: HashMap<&str, Vec<&ParsedElement>> = HashMap::new();
    let mut root_element = None;

    for el in elements {
        if el.parent_id.is_none() {
            if root_element.is_some() {
                return Err(anyhow::anyhow!("multiple root elements found"));
            }
            root_element = Some(el);
        } else {
            children_map
                .entry(el.parent_id.as_deref().unwrap())
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
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
) -> anyhow::Result<Node> {
    let mut style = el.style.clone();
    style.id = el.id.clone();

    match &el.kind {
        ParsedElementKind::Div => {
            let mut div_node = div();
            div_node.style = style;

            if let Some(children) = children_map.get(el.id.as_str()) {
                for child in children {
                    let child_node = build_node(child, children_map)?;
                    div_node = div_node.child(child_node);
                }
            }

            Ok(Node::new(div_node))
        }
        ParsedElementKind::Text { content } => {
            let mut text_node = text(content);
            text_node.style = style;
            Ok(Node::new(text_node))
        }
        ParsedElementKind::Image { source } => {
            let mut image_node = image();
            image_node = match source {
                ImageSource::Unset => {
                    return Err(anyhow::anyhow!(
                        "image node requires one of: path, url, query"
                    ));
                }
                ImageSource::Path(path) => image_node.path(path),
                ImageSource::Url(url) => image_node.url(url.clone()),
                ImageSource::Query(query) => {
                    let mut image_node = image_node.query(query.query.clone());
                    image_node = image_node.query_count(query.count);
                    if let Some(aspect_ratio) = &query.aspect_ratio {
                        image_node = image_node.aspect_ratio(aspect_ratio.clone());
                    }
                    image_node
                }
            };
            image_node.style = style;
            Ok(Node::new(image_node))
        }
        ParsedElementKind::Icon { name } => {
            let mut icon_node = lucide(name.clone());
            icon_node.style = style;
            Ok(Node::new(icon_node))
        }
        ParsedElementKind::Video { path } => {
            let mut video_node = video(path);
            video_node.style = style;
            Ok(Node::new(video_node))
        }
    }
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
        "overflow-hidden" => style.overflow_hidden = true,
        "bg-gradient-to-r" => style.bg_gradient_direction = Some(GradientDirection::ToRight),

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
    if let Some(value) = class.strip_prefix("bg-[") {
        if let Some(v) = value.strip_suffix(']') {
            if let Some(color) = color_from_hex(v) {
                style.bg_color = Some(color);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("text-[") {
        if let Some(v) = value.strip_suffix(']') {
            if let Some(color) = color_from_hex(v) {
                style.text_color = Some(color);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("border-[") {
        if let Some(v) = value.strip_suffix(']') {
            if let Some(color) = color_from_hex(v) {
                style.border_color = Some(color);
                return;
            }
        }
    }

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
            if let Some(c) = color_token_from_class_suffix(v) {
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

    if let Some(value) = class.strip_prefix("pt-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_top = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_top = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("pr-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_right = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_right = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("pb-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_bottom = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_bottom = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("pl-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_left = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.padding_left = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("mt-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_top = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_top = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("mr-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_right = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_right = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("mb-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_bottom = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_bottom = Some(n);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("ml-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_left = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix("]") {
            if let Ok(n) = v.parse::<f32>() {
                style.margin_left = Some(n);
                return;
            }
        }
    }

    if let Some(color) = class
        .strip_prefix("bg-")
        .and_then(color_token_from_class_suffix)
    {
        style.bg_color = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("text-")
        .and_then(color_token_from_class_suffix)
    {
        style.text_color = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("border-")
        .and_then(color_token_from_class_suffix)
    {
        style.border_color = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("from-")
        .and_then(color_token_from_class_suffix)
    {
        style.bg_gradient_from = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("to-")
        .and_then(color_token_from_class_suffix)
    {
        style.bg_gradient_to = Some(color);
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

    if class.starts_with("pt-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.padding_top = Some(n);
        }
    }

    if class.starts_with("pr-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.padding_right = Some(n);
        }
    }

    if class.starts_with("pb-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.padding_bottom = Some(n);
        }
    }

    if class.starts_with("pl-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.padding_left = Some(n);
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

    if class.starts_with("mt-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.margin_top = Some(n);
        }
    }

    if class.starts_with("mr-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.margin_right = Some(n);
        }
    }

    if class.starts_with("mb-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.margin_bottom = Some(n);
        }
    }

    if class.starts_with("ml-") {
        if let Ok(n) = class[3..].parse::<f32>() {
            style.margin_left = Some(n);
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

#[cfg(test)]
mod tests {
    use super::{parse, parse_class_name};
    use crate::style::{ColorToken, GradientDirection, TextAlign};

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
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","content":"ctx.getNode('root').opacity(0.5);"}"#,
        )
        .expect("jsonl should parse");

        assert_eq!(parsed.width, 640);
        assert_eq!(parsed.height, 360);
        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('root').opacity(0.5);")
        );
    }

    #[test]
    fn parser_rejects_multiple_roots() {
        let err = parse(
            r#"{"id":"root-a","parentId":null,"type":"div","className":"","text":null}
{"id":"root-b","parentId":null,"type":"div","className":"","text":null}"#,
        )
        .err()
        .expect("multiple roots should fail");

        assert!(err.to_string().contains("multiple root"));
    }

    #[test]
    fn parser_maps_hex_colors() {
        let style = parse_class_name("bg-[#fff8f0] text-[#e85d04] border-[#5c4033]");

        assert_eq!(
            style.bg_color,
            Some(ColorToken::Custom(0xff, 0xf8, 0xf0, 0xff))
        );
        assert_eq!(
            style.text_color,
            Some(ColorToken::Custom(0xe8, 0x5d, 0x04, 0xff))
        );
        assert_eq!(
            style.border_color,
            Some(ColorToken::Custom(0x5c, 0x40, 0x33, 0xff))
        );
    }

    #[test]
    fn parser_maps_common_icon_styling_classes() {
        let style = parse_class_name("bg-emerald-400 text-white border-[3] border-blue");

        assert_eq!(style.bg_color, Some(ColorToken::Emerald400));
        assert_eq!(style.text_color, Some(ColorToken::White));
        assert_eq!(style.border_width, Some(3.0));
        assert_eq!(style.border_color, Some(ColorToken::Blue));
    }

    #[test]
    fn parser_maps_gradient_overflow_and_directional_spacing_classes() {
        let style = parse_class_name(
            "bg-gradient-to-r from-orange-500 to-amber-500 overflow-hidden mt-[4px] mb-[16px] pb-[20px]",
        );

        assert_eq!(
            style.bg_gradient_direction,
            Some(GradientDirection::ToRight)
        );
        assert_eq!(style.bg_gradient_from, Some(ColorToken::Orange500));
        assert_eq!(style.bg_gradient_to, Some(ColorToken::Amber500));
        assert!(style.overflow_hidden);
        assert_eq!(style.margin_top, Some(4.0));
        assert_eq!(style.margin_bottom, Some(16.0));
        assert_eq!(style.padding_bottom, Some(20.0));
    }

    #[test]
    fn parser_accepts_image_query_nodes() {
        parse(
            r#"{"type":"composition","width":1280,"height":720,"fps":30,"frames":90}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full","text":null}
{"id":"hero","parentId":"root","type":"image","className":"w-[320px] h-[240px] object-cover","query":"pizza margherita"}"#,
        )
        .expect("jsonl with image query should parse");
    }

    #[test]
    fn parser_accepts_lucide_icon_nodes() {
        parse(
            r#"{"type":"composition","width":390,"height":844,"fps":30,"frames":180}
{"id":"root","parentId":null,"type":"div","className":"w-full h-full"}
{"id":"search-icon","parentId":"root","type":"icon","className":"w-[20px] h-[20px] text-slate-400","icon":"search"}"#,
        )
        .expect("jsonl with lucide icon should parse");
    }
}
