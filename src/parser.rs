use std::collections::{HashMap, HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

use crate::nodes::{ImageSource, OpenverseQuery, canvas, div, image, lucide, text, video};
use crate::script::ScriptDriver;
use crate::style::{
    AlignItems, ColorToken, FlexDirection, FontWeight, GradientDirection, JustifyContent,
    NodeStyle, ObjectFit, Position, TextAlign, color_token_from_class_suffix,
};
use crate::transitions::{
    Timing, Transition, clock_wipe, fade, iris, light_leak, linear, slide, spring, timeline, wipe,
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
    Script {
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        content: String,
    },
    #[serde(rename = "div")]
    Div {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        duration: Option<u32>,
    },
    #[serde(rename = "text")]
    Text {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        text: String,
        duration: Option<u32>,
    },
    #[serde(rename = "canvas")]
    Canvas {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        duration: Option<u32>,
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
        duration: Option<u32>,
    },
    #[serde(rename = "video")]
    Video {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        path: String,
        duration: Option<u32>,
    },
    #[serde(rename = "icon")]
    Icon {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
        icon: String,
        duration: Option<u32>,
    },
    #[serde(rename = "transition")]
    Transition {
        from: String,
        to: String,
        effect: String,
        duration: u32,
        direction: Option<String>,
        timing: Option<String>,
        damping: Option<f32>,
        stiffness: Option<f32>,
        mass: Option<f32>,
        seed: Option<f32>,
        #[serde(rename = "hueShift")]
        hue_shift: Option<f32>,
        #[serde(rename = "maskScale")]
        mask_scale: Option<f32>,
    },
}

#[derive(Debug, Clone)]
enum ParsedElementKind {
    Div,
    Text { content: String },
    Canvas,
    Image { source: ImageSource },
    Icon { name: String },
    Video { path: PathBuf },
}

#[derive(Debug, Clone)]
struct ParsedElement {
    id: String,
    parent_id: Option<String>,
    duration: Option<u32>,
    style: NodeStyle,
    kind: ParsedElementKind,
}

#[derive(Debug, Clone)]
struct ParsedTransition {
    from: String,
    to: String,
    effect: String,
    duration: u32,
    direction: Option<String>,
    timing: Option<String>,
    damping: Option<f32>,
    stiffness: Option<f32>,
    mass: Option<f32>,
    seed: Option<f32>,
    hue_shift: Option<f32>,
    mask_scale: Option<f32>,
}

#[derive(Debug, Clone)]
enum TimelineEntry {
    SequenceRoot { id: String },
    Transition(ParsedTransition),
}

pub struct ParsedComposition {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub frames: i32,
    pub root: Node,
    pub script: Option<String>,
}

static UNSUPPORTED_TAILWIND_CLASSES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub fn parse(input: &str) -> anyhow::Result<ParsedComposition> {
    let mut width = 1920;
    let mut height = 1080;
    let mut fps = 30;
    let mut frames = 90;
    let mut global_scripts = Vec::new();
    let mut scripts_by_parent: HashMap<String, Vec<String>> = HashMap::new();
    let mut elements: Vec<ParsedElement> = Vec::new();
    let mut timeline_entries = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
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
            JsonLine::Script { parent_id, content } => {
                if let Some(parent_id) = parent_id {
                    scripts_by_parent
                        .entry(parent_id)
                        .or_default()
                        .push(content);
                } else {
                    global_scripts.push(content);
                }
            }
            JsonLine::Div {
                id,
                parent_id,
                class_name,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Div,
                });
            }
            JsonLine::Text {
                id,
                parent_id,
                class_name,
                text,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Text { content: text },
                });
            }
            JsonLine::Canvas {
                id,
                parent_id,
                class_name,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Canvas,
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
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                let source = parse_image_source(path, url, query, query_count, aspect_ratio)?;
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Image { source },
                });
            }
            JsonLine::Video {
                id,
                parent_id,
                class_name,
                path,
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
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
                duration,
            } => {
                let style = parse_class_name_with_context(
                    class_name.as_deref().unwrap_or(""),
                    &id,
                    line_index + 1,
                );
                if parent_id.is_none() {
                    timeline_entries.push(TimelineEntry::SequenceRoot { id: id.clone() });
                }
                elements.push(ParsedElement {
                    id,
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Icon { name: icon },
                });
            }
            JsonLine::Transition {
                from,
                to,
                effect,
                duration,
                direction,
                timing,
                damping,
                stiffness,
                mass,
                seed,
                hue_shift,
                mask_scale,
            } => timeline_entries.push(TimelineEntry::Transition(ParsedTransition {
                from,
                to,
                effect,
                duration,
                direction,
                timing,
                damping,
                stiffness,
                mass,
                seed,
                hue_shift,
                mask_scale,
            })),
        }
    }

    let has_timeline = timeline_entries
        .iter()
        .any(|entry| matches!(entry, TimelineEntry::Transition(_)))
        || elements.iter().filter(|el| el.parent_id.is_none()).count() > 1;
    let (root, frames) = if has_timeline {
        build_timeline(&elements, &scripts_by_parent, &timeline_entries)?
    } else {
        (build_tree(&elements, &scripts_by_parent)?, frames)
    };

    Ok(ParsedComposition {
        width,
        height,
        fps,
        frames,
        root,
        script: join_scripts(global_scripts),
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

fn join_scripts(scripts: Vec<String>) -> Option<String> {
    if scripts.is_empty() {
        None
    } else {
        Some(scripts.join("\n"))
    }
}

fn index_elements<'a>(
    elements: &'a [ParsedElement],
) -> (
    HashMap<&'a str, Vec<&'a ParsedElement>>,
    Vec<&'a ParsedElement>,
) {
    let mut children_map: HashMap<&str, Vec<&ParsedElement>> = HashMap::new();
    let mut roots = Vec::new();

    for el in elements {
        if let Some(parent_id) = el.parent_id.as_deref() {
            children_map.entry(parent_id).or_default().push(el);
        } else {
            roots.push(el);
        }
    }

    (children_map, roots)
}

fn build_tree(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
) -> anyhow::Result<Node> {
    let (children_map, roots) = index_elements(elements);
    if roots.len() > 1 {
        return Err(anyhow::anyhow!("multiple root elements found"));
    }

    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    build_node(root, &children_map, scripts_by_parent)
}

fn build_timeline(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    entries: &[TimelineEntry],
) -> anyhow::Result<(Node, i32)> {
    let (children_map, roots) = index_elements(elements);
    let roots_by_id = roots
        .into_iter()
        .map(|root| (root.id.as_str(), root))
        .collect::<HashMap<_, _>>();

    let mut timeline_builder = timeline();
    let mut frames = 0_i32;

    for (index, entry) in entries.iter().enumerate() {
        match entry {
            TimelineEntry::SequenceRoot { id } => {
                let root = roots_by_id
                    .get(id.as_str())
                    .ok_or_else(|| anyhow::anyhow!("sequence root `{id}` was not found"))?;
                let duration = root.duration.ok_or_else(|| {
                    anyhow::anyhow!("timeline sequence `{id}` is missing a duration")
                })?;
                let node = build_node(root, &children_map, scripts_by_parent)?;
                timeline_builder = timeline_builder.sequence(duration, node);
                frames += duration as i32;
            }
            TimelineEntry::Transition(transition) => {
                let Some(TimelineEntry::SequenceRoot { id: previous_id }) =
                    index.checked_sub(1).and_then(|idx| entries.get(idx))
                else {
                    return Err(anyhow::anyhow!(
                        "transition from `{}` to `{}` must appear between two sequences",
                        transition.from,
                        transition.to
                    ));
                };
                let Some(TimelineEntry::SequenceRoot { id: next_id }) = entries.get(index + 1)
                else {
                    return Err(anyhow::anyhow!(
                        "transition from `{}` to `{}` must appear between two sequences",
                        transition.from,
                        transition.to
                    ));
                };

                if previous_id != &transition.from || next_id != &transition.to {
                    return Err(anyhow::anyhow!(
                        "transition declares `{}` -> `{}`, but neighboring sequences are `{}` -> `{}`",
                        transition.from,
                        transition.to,
                        previous_id,
                        next_id
                    ));
                }

                timeline_builder = timeline_builder.transition(build_transition(transition)?);
                frames += transition.duration as i32;
            }
        }
    }

    Ok((timeline_builder.into(), frames))
}

fn build_node(
    el: &ParsedElement,
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    scripts_by_parent: &HashMap<String, Vec<String>>,
) -> anyhow::Result<Node> {
    let mut style = el.style.clone();
    style.id = el.id.clone();
    if let Some(script) = scripts_by_parent
        .get(el.id.as_str())
        .and_then(|scripts| join_scripts(scripts.clone()))
    {
        style.script_driver = Some(std::sync::Arc::new(ScriptDriver::from_source(&script)?));
    }

    match &el.kind {
        ParsedElementKind::Div => {
            let mut div_node = div();
            div_node.style = style;

            if let Some(children) = children_map.get(el.id.as_str()) {
                for child in children {
                    let child_node = build_node(child, children_map, scripts_by_parent)?;
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
        ParsedElementKind::Canvas => {
            if children_map
                .get(el.id.as_str())
                .is_some_and(|children| !children.is_empty())
            {
                return Err(anyhow::anyhow!(
                    "canvas node `{}` cannot have child nodes",
                    el.id
                ));
            }
            let mut canvas_node = canvas();
            canvas_node.style = style;
            Ok(Node::new(canvas_node))
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

fn build_transition(transition: &ParsedTransition) -> anyhow::Result<Transition> {
    let timing = parse_transition_timing(transition)?;
    let effect = normalize_transition_name(&transition.effect);

    match effect.as_str() {
        "fade" => Ok(fade().timing(timing)),
        "clock_wipe" => Ok(clock_wipe().timing(timing)),
        "iris" => Ok(iris().timing(timing)),
        "slide" => match transition
            .direction
            .as_deref()
            .map(normalize_transition_name)
            .as_deref()
        {
            None | Some("from_left") => Ok(slide().from_left().timing(timing)),
            Some("from_right") => Ok(slide().from_right().timing(timing)),
            Some("from_top") => Ok(slide().from_top().timing(timing)),
            Some("from_bottom") => Ok(slide().from_bottom().timing(timing)),
            Some(direction) => Err(anyhow::anyhow!("unsupported slide direction `{direction}`")),
        },
        "wipe" => match transition
            .direction
            .as_deref()
            .map(normalize_transition_name)
            .as_deref()
        {
            None | Some("from_left") => Ok(wipe().from_left().timing(timing)),
            Some("from_right") => Ok(wipe().from_right().timing(timing)),
            Some("from_top") => Ok(wipe().from_top().timing(timing)),
            Some("from_bottom") => Ok(wipe().from_bottom().timing(timing)),
            Some("from_top_left") => Ok(wipe().from_top_left().timing(timing)),
            Some("from_top_right") => Ok(wipe().from_top_right().timing(timing)),
            Some("from_bottom_left") => Ok(wipe().from_bottom_left().timing(timing)),
            Some("from_bottom_right") => Ok(wipe().from_bottom_right().timing(timing)),
            Some(direction) => Err(anyhow::anyhow!("unsupported wipe direction `{direction}`")),
        },
        "light_leak" => {
            let mut builder = light_leak();
            if let Some(seed) = transition.seed {
                builder = builder.seed(seed);
            }
            if let Some(hue_shift) = transition.hue_shift {
                builder = builder.hue_shift(hue_shift);
            }
            if let Some(mask_scale) = transition.mask_scale {
                builder = builder.mask_scale(mask_scale);
            }
            Ok(builder.timing(timing))
        }
        _ => Err(anyhow::anyhow!(
            "unsupported transition effect `{}`",
            transition.effect
        )),
    }
}

fn parse_transition_timing(transition: &ParsedTransition) -> anyhow::Result<Timing> {
    let timing = transition
        .timing
        .as_deref()
        .map(normalize_transition_name)
        .unwrap_or_else(|| {
            if transition.damping.is_some()
                || transition.stiffness.is_some()
                || transition.mass.is_some()
            {
                "spring".to_string()
            } else {
                "linear".to_string()
            }
        });

    match timing.as_str() {
        "linear" => Ok(linear().duration(transition.duration)),
        "spring" => {
            let mut builder = spring();
            if let Some(damping) = transition.damping {
                builder = builder.damping(damping);
            }
            if let Some(stiffness) = transition.stiffness {
                builder = builder.stiffness(stiffness);
            }
            if let Some(mass) = transition.mass {
                builder = builder.mass(mass);
            }
            Ok(builder.duration(transition.duration))
        }
        _ => Err(anyhow::anyhow!(
            "unsupported transition timing `{}`",
            transition.timing.as_deref().unwrap_or("unknown")
        )),
    }
}

fn normalize_transition_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

#[cfg_attr(not(test), allow(dead_code))]
fn parse_class_name(class_name: &str) -> NodeStyle {
    parse_class_name_impl(class_name, None)
}

fn parse_class_name_with_context(class_name: &str, node_id: &str, line_number: usize) -> NodeStyle {
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

fn parser_style_fingerprint(style: &NodeStyle) -> u64 {
    let mut hasher = DefaultHasher::new();
    style.position.hash(&mut hasher);
    style.inset_left.map(f32::to_bits).hash(&mut hasher);
    style.inset_top.map(f32::to_bits).hash(&mut hasher);
    style.inset_right.map(f32::to_bits).hash(&mut hasher);
    style.inset_bottom.map(f32::to_bits).hash(&mut hasher);
    style.width.map(f32::to_bits).hash(&mut hasher);
    style.height.map(f32::to_bits).hash(&mut hasher);
    style.width_full.hash(&mut hasher);
    style.height_full.hash(&mut hasher);
    style.padding.map(f32::to_bits).hash(&mut hasher);
    style.padding_x.map(f32::to_bits).hash(&mut hasher);
    style.padding_y.map(f32::to_bits).hash(&mut hasher);
    style.padding_top.map(f32::to_bits).hash(&mut hasher);
    style.padding_right.map(f32::to_bits).hash(&mut hasher);
    style.padding_bottom.map(f32::to_bits).hash(&mut hasher);
    style.padding_left.map(f32::to_bits).hash(&mut hasher);
    style.margin.map(f32::to_bits).hash(&mut hasher);
    style.margin_x.map(f32::to_bits).hash(&mut hasher);
    style.margin_y.map(f32::to_bits).hash(&mut hasher);
    style.margin_top.map(f32::to_bits).hash(&mut hasher);
    style.margin_right.map(f32::to_bits).hash(&mut hasher);
    style.margin_bottom.map(f32::to_bits).hash(&mut hasher);
    style.margin_left.map(f32::to_bits).hash(&mut hasher);
    style.flex_direction.hash(&mut hasher);
    style.justify_content.hash(&mut hasher);
    style.align_items.hash(&mut hasher);
    style.is_flex.hash(&mut hasher);
    style.auto_size.hash(&mut hasher);
    style.gap.map(f32::to_bits).hash(&mut hasher);
    style.flex_grow.map(f32::to_bits).hash(&mut hasher);
    style.flex_shrink.map(f32::to_bits).hash(&mut hasher);
    style.z_index.hash(&mut hasher);
    style.opacity.map(f32::to_bits).hash(&mut hasher);
    style.bg_color.hash(&mut hasher);
    style.bg_gradient_from.hash(&mut hasher);
    style.bg_gradient_via.hash(&mut hasher);
    style.bg_gradient_to.hash(&mut hasher);
    style.bg_gradient_direction.hash(&mut hasher);
    style.border_radius.map(f32::to_bits).hash(&mut hasher);
    style.border_width.map(f32::to_bits).hash(&mut hasher);
    style.border_color.hash(&mut hasher);
    style.blur_sigma.map(f32::to_bits).hash(&mut hasher);
    style.object_fit.hash(&mut hasher);
    style.overflow_hidden.hash(&mut hasher);
    style.text_color.hash(&mut hasher);
    style.text_px.map(f32::to_bits).hash(&mut hasher);
    style.font_weight.hash(&mut hasher);
    style.letter_spacing.map(f32::to_bits).hash(&mut hasher);
    style.text_align.hash(&mut hasher);
    style.line_height.map(f32::to_bits).hash(&mut hasher);
    style.shadow.hash(&mut hasher);
    hasher.finish()
}

fn parse_single_class(class: &str, style: &mut NodeStyle) -> bool {
    match class {
        // Position
        "relative" => style.position = Some(Position::Relative),
        "absolute" => style.position = Some(Position::Absolute),

        // Flex layout
        "flex" => {
            style.is_flex = true;
            if style.flex_direction.is_none() {
                style.flex_direction = Some(FlexDirection::Row);
            }
        }
        "flex-row" => {
            style.is_flex = true;
            style.flex_direction = Some(FlexDirection::Row);
        }
        "flex-col" | "flex-column" => {
            style.is_flex = true;
            style.flex_direction = Some(FlexDirection::Col);
        }

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
        "pointer-events-none" => {}
        "inset-0" => {
            style.inset_left = Some(0.0);
            style.inset_top = Some(0.0);
            style.inset_right = Some(0.0);
            style.inset_bottom = Some(0.0);
        }
        "bg-gradient-to-r" => style.bg_gradient_direction = Some(GradientDirection::ToRight),
        "bg-gradient-to-l" => style.bg_gradient_direction = Some(GradientDirection::ToLeft),
        "bg-gradient-to-b" => style.bg_gradient_direction = Some(GradientDirection::ToBottom),
        "bg-gradient-to-t" => style.bg_gradient_direction = Some(GradientDirection::ToTop),
        "bg-gradient-to-br" => style.bg_gradient_direction = Some(GradientDirection::ToBottomRight),
        "shrink-0" | "flex-shrink-0" => style.flex_shrink = Some(0.0),
        "flex-1" => style.flex_grow = Some(1.0),

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
        "tracking-normal" => style.letter_spacing = Some(0.0),
        "tracking-wide" => style.letter_spacing = Some(0.5),
        "tracking-wider" => style.letter_spacing = Some(1.0),
        "blur-none" => style.blur_sigma = Some(0.0),
        "blur-sm" => style.blur_sigma = Some(4.0),
        "blur" | "blur-md" => style.blur_sigma = Some(8.0),
        "blur-lg" => style.blur_sigma = Some(16.0),
        "blur-xl" => style.blur_sigma = Some(24.0),
        "blur-2xl" => style.blur_sigma = Some(40.0),
        "blur-3xl" => style.blur_sigma = Some(64.0),

        _ => {
            let before = parser_style_fingerprint(style);
            parse_arbitrary_class(class, style);
            return parser_style_fingerprint(style) != before;
        }
    }

    true
}

fn parse_arbitrary_class(class: &str, style: &mut NodeStyle) {
    if let Some(n) = parse_signed_bracket_f32(class, "left-[", "-left-[") {
        style.inset_left = Some(n);
        return;
    }

    if let Some(n) = parse_signed_bracket_f32(class, "top-[", "-top-[") {
        style.inset_top = Some(n);
        return;
    }

    if let Some(n) = parse_signed_bracket_f32(class, "right-[", "-right-[") {
        style.inset_right = Some(n);
        return;
    }

    if let Some(n) = parse_signed_bracket_f32(class, "bottom-[", "-bottom-[") {
        style.inset_bottom = Some(n);
        return;
    }

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
            if let Some(c) = parse_color_token_with_opacity(v) {
                style.border_color = Some(c);
                return;
            }
        }
    }

    if let Some(value) = class.strip_prefix("blur-[") {
        if let Some(v) = value.strip_suffix("px]") {
            if let Ok(n) = v.parse::<f32>() {
                style.blur_sigma = Some(n);
                return;
            }
        }
        if let Some(v) = value.strip_suffix(']') {
            if let Ok(n) = v.parse::<f32>() {
                style.blur_sigma = Some(n);
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
        .and_then(parse_color_token_with_opacity)
    {
        style.bg_color = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("text-")
        .and_then(parse_color_token_with_opacity)
    {
        style.text_color = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("border-")
        .and_then(parse_color_token_with_opacity)
    {
        style.border_color = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("from-")
        .and_then(parse_color_token_with_opacity)
    {
        style.bg_gradient_from = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("via-")
        .and_then(parse_color_token_with_opacity)
    {
        style.bg_gradient_via = Some(color);
        return;
    }

    if let Some(color) = class
        .strip_prefix("to-")
        .and_then(parse_color_token_with_opacity)
    {
        style.bg_gradient_to = Some(color);
        return;
    }

    if let Some(value) = class.strip_prefix("gap-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.gap = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("w-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.width = Some(n);
            style.width_full = false;
        }
    }

    if let Some(value) = class.strip_prefix("h-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.height = Some(n);
            style.height_full = false;
        }
    }

    if class.starts_with("text-") && !class.starts_with("text-[") {
        let after = &class[5..];
        if let Some(n) = parse_tailwind_text_size(after) {
            style.text_px = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("left-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.inset_left = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("top-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.inset_top = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("right-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.inset_right = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("bottom-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.inset_bottom = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("p-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.padding = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("px-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.padding_x = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("py-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.padding_y = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("pt-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.padding_top = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("pr-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.padding_right = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("pb-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.padding_bottom = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("pl-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.padding_left = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("m-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.margin = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("mx-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.margin_x = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("my-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.margin_y = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("mt-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.margin_top = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("mr-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.margin_right = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("mb-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
            style.margin_bottom = Some(n);
        }
    }

    if let Some(value) = class.strip_prefix("ml-") {
        if let Some(n) = parse_tailwind_spacing_token(value) {
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

    if let Some(value) = class.strip_prefix("z-[") {
        if let Some(v) = value.strip_suffix(']') {
            if let Ok(n) = v.parse::<i32>() {
                style.z_index = Some(n);
            }
        }
    }

    if class.starts_with("z-") {
        if let Ok(n) = class[2..].parse::<i32>() {
            style.z_index = Some(n);
        }
    }

    if class.starts_with("tracking-") {
        if let Ok(n) = class[9..].parse::<f32>() {
            style.letter_spacing = Some(n);
        }
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
    let color = color.to_skia();
    let alpha = ((color.a() as f32) * opacity).round().clamp(0.0, 255.0) as u8;

    Some(ColorToken::Custom(color.r(), color.g(), color.b(), alpha))
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
    match value {
        "xs" => Some(12.0),
        "sm" => Some(14.0),
        "base" => Some(16.0),
        "lg" => Some(18.0),
        "xl" => Some(20.0),
        "2xl" => Some(24.0),
        "3xl" => Some(30.0),
        "4xl" => Some(36.0),
        "5xl" => Some(48.0),
        "6xl" => Some(60.0),
        "7xl" => Some(72.0),
        "8xl" => Some(96.0),
        "9xl" => Some(128.0),
        _ => value.parse::<f32>().ok(),
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
    use crate::{
        FrameCtx,
        style::{ColorToken, GradientDirection, TextAlign},
        timeline::FrameState,
        timeline::frame_state_for_root,
        view::NodeKind,
    };

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
    fn parser_builds_timeline_from_root_sequences_and_transitions() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":999}
{"id":"root-a","parentId":null,"type":"div","className":"","duration":10}
{"type":"script","parentId":"root-a","content":"ctx.getNode('root-a').opacity(0.6);"}
{"type":"transition","from":"root-a","to":"root-b","effect":"fade","duration":5}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":10}"#,
        )
        .expect("timeline jsonl should parse");

        assert_eq!(parsed.frames, 25);
        assert!(matches!(parsed.root.kind(), NodeKind::Timeline(_)));

        let scene_frame = FrameCtx {
            frame: 0,
            fps: parsed.fps as u32,
            width: parsed.width,
            height: parsed.height,
            frames: parsed.frames as u32,
        };
        let transition_frame = FrameCtx {
            frame: 12,
            ..scene_frame
        };
        let final_scene_frame = FrameCtx {
            frame: 20,
            ..scene_frame
        };

        match frame_state_for_root(&parsed.root, &scene_frame) {
            FrameState::Scene { scene } => {
                assert_eq!(scene.style_ref().id, "root-a");
                assert!(scene.style_ref().script_driver.is_some());
            }
            _ => panic!("frame 0 should resolve to the first sequence"),
        }

        match frame_state_for_root(&parsed.root, &transition_frame) {
            FrameState::Transition { from, to, .. } => {
                assert_eq!(from.style_ref().id, "root-a");
                assert_eq!(to.style_ref().id, "root-b");
            }
            _ => panic!("frame 12 should resolve to the transition"),
        }

        match frame_state_for_root(&parsed.root, &final_scene_frame) {
            FrameState::Scene { scene } => {
                assert_eq!(scene.style_ref().id, "root-b");
            }
            _ => panic!("frame 20 should resolve to the final sequence"),
        }
    }

    #[test]
    fn parser_requires_sequence_duration_when_building_timeline() {
        let err = parse(
            r#"{"id":"root-a","parentId":null,"type":"div","className":""}
{"type":"transition","from":"root-a","to":"root-b","effect":"fade","duration":5}
{"id":"root-b","parentId":null,"type":"div","className":"","duration":10}"#,
        )
        .err()
        .expect("timeline without durations should fail");

        assert!(err.to_string().contains("missing a duration"));
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
    fn parser_maps_tailwind_spacing_scale_and_text_sizes() {
        let style = parse_class_name("w-20 h-28 left-8 top-4 px-6 py-2 gap-3.5 text-3xl");

        assert_eq!(style.width, Some(80.0));
        assert_eq!(style.height, Some(112.0));
        assert_eq!(style.inset_left, Some(32.0));
        assert_eq!(style.inset_top, Some(16.0));
        assert_eq!(style.padding_x, Some(24.0));
        assert_eq!(style.padding_y, Some(8.0));
        assert_eq!(style.gap, Some(14.0));
        assert_eq!(style.text_px, Some(30.0));
    }

    #[test]
    fn parser_maps_extended_gradient_and_layering_classes() {
        let style = parse_class_name(
            "bg-gradient-to-br from-transparent via-pink-500 to-violet-400 inset-0 -top-[80px] -left-[24px] z-10 flex-shrink-0 tracking-wider",
        );

        assert_eq!(
            style.bg_gradient_direction,
            Some(GradientDirection::ToBottomRight)
        );
        assert_eq!(style.bg_gradient_from, Some(ColorToken::Transparent));
        assert_eq!(style.bg_gradient_via, Some(ColorToken::Pink500));
        assert_eq!(style.bg_gradient_to, Some(ColorToken::Violet400));
        assert_eq!(style.inset_right, Some(0.0));
        assert_eq!(style.inset_bottom, Some(0.0));
        assert_eq!(style.inset_top, Some(-80.0));
        assert_eq!(style.inset_left, Some(-24.0));
        assert_eq!(style.z_index, Some(10));
        assert_eq!(style.flex_shrink, Some(0.0));
        assert_eq!(style.letter_spacing, Some(1.0));
    }

    #[test]
    fn parser_maps_tailwind_alpha_blur_and_additional_gradients() {
        let style = parse_class_name(
            "flex-1 bg-gradient-to-b from-amber-100/30 via-transparent to-rose-100/20 blur-xl pointer-events-none",
        );

        assert_eq!(style.flex_grow, Some(1.0));
        assert_eq!(
            style.bg_gradient_direction,
            Some(GradientDirection::ToBottom)
        );
        assert_eq!(style.bg_gradient_via, Some(ColorToken::Transparent));
        assert_eq!(style.blur_sigma, Some(24.0));
        assert_eq!(
            style.bg_gradient_from.expect("from color").to_skia().a(),
            77
        );
        assert_eq!(style.bg_gradient_to.expect("to color").to_skia().a(), 51);

        let overlay = parse_class_name("bg-gradient-to-t from-rose-900/10 to-amber-100/15");
        assert_eq!(
            overlay.bg_gradient_direction,
            Some(GradientDirection::ToTop)
        );
        assert_eq!(
            overlay.bg_gradient_from.expect("from color").to_skia().a(),
            26
        );
        assert_eq!(overlay.bg_gradient_to.expect("to color").to_skia().a(), 38);
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
