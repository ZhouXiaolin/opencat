use std::collections::HashMap;

use crate::scene::{
    easing::{Easing, SpringConfig, easing_from_name},
    node::Node,
    primitives::{ImageSource, canvas, caption, div, image, lucide, parse_srt, text, video},
    script::ScriptDriver,
    transition::{Transition, clock_wipe, fade, iris, light_leak, slide, timeline, wipe},
};

use super::{ParsedElement, ParsedElementKind, ParsedTransition};

pub(super) fn join_scripts(scripts: Vec<String>) -> Option<String> {
    if scripts.is_empty() {
        None
    } else {
        Some(scripts.join("\n"))
    }
}

fn index_elements(
    elements: &[ParsedElement],
) -> (HashMap<&str, Vec<&ParsedElement>>, Vec<&ParsedElement>) {
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

pub(super) fn build_tree(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    fps: u32,
) -> anyhow::Result<Node> {
    let (children_map, roots) = index_elements(elements);
    if roots.len() > 1 {
        return Err(anyhow::anyhow!("multiple root elements found"));
    }

    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    build_node_inner(root, &children_map, scripts_by_parent, &HashMap::new(), fps)
}

pub(super) fn build_tree_with_tl(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    transitions_by_tl: &HashMap<String, Vec<&ParsedTransition>>,
    fps: u32,
) -> anyhow::Result<Node> {
    let (children_map, roots) = index_elements(elements);
    if roots.len() > 1 {
        return Err(anyhow::anyhow!("multiple root elements found"));
    }

    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    build_node_inner(root, &children_map, scripts_by_parent, transitions_by_tl, fps)
}

fn build_node_inner(
    el: &ParsedElement,
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    scripts_by_parent: &HashMap<String, Vec<String>>,
    transitions_by_tl: &HashMap<String, Vec<&ParsedTransition>>,
    fps: u32,
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
        ParsedElementKind::Timeline => {
            let sequence_children = children_map.get(el.id.as_str());
            let transitions = transitions_by_tl.get(el.id.as_str());
            let children = sequence_children.map(|c| c.as_slice()).unwrap_or(&[]);
            let transitions = transitions.map(|t| t.as_slice()).unwrap_or(&[]);

            if children.len() < 2 {
                return Err(anyhow::anyhow!(
                    "timeline `{}` must have at least two direct child scenes",
                    el.id
                ));
            }

            let mut tl_builder = timeline();

            let child_ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
            let child_positions: HashMap<&str, usize> = child_ids
                .iter()
                .enumerate()
                .map(|(i, &id)| (id, i))
                .collect();

            let mut transitions_by_pair = HashMap::new();
            for t in transitions {
                let Some(&from_idx) = child_positions.get(t.from.as_str()) else {
                    return Err(anyhow::anyhow!(
                        "transition `from` references `{}`, which is not a direct child of this timeline",
                        t.from
                    ));
                };
                let Some(&to_idx) = child_positions.get(t.to.as_str()) else {
                    return Err(anyhow::anyhow!(
                        "transition `to` references `{}`, which is not a direct child of this timeline",
                        t.to
                    ));
                };
                if to_idx != from_idx + 1 {
                    let next_id = child_ids.get(from_idx + 1).copied().unwrap_or("<end>");
                    return Err(anyhow::anyhow!(
                        "transition `{}` -> `{}` is not between adjacent children (expected `{}` -> `{}`)",
                        t.from,
                        t.to,
                        t.from,
                        next_id
                    ));
                }
                if transitions_by_pair
                    .insert((t.from.as_str(), t.to.as_str()), *t)
                    .is_some()
                {
                    return Err(anyhow::anyhow!(
                        "duplicate transition declared for `{}` -> `{}`",
                        t.from,
                        t.to
                    ));
                }
            }

            for pair in child_ids.windows(2) {
                let from_id = pair[0];
                let to_id = pair[1];
                if !transitions_by_pair.contains_key(&(from_id, to_id)) {
                    return Err(anyhow::anyhow!(
                        "timeline `{}` is missing transition between adjacent scenes `{}` -> `{}`",
                        el.id,
                        from_id,
                        to_id
                    ));
                }
            }

            for (index, child_el) in children.iter().enumerate() {
                let duration = child_el.duration.ok_or_else(|| {
                    anyhow::anyhow!(
                        "timeline sequence `{}` is missing a duration",
                        child_el.id
                    )
                })?;
                let node = build_node_inner(
                    child_el,
                    children_map,
                    scripts_by_parent,
                    transitions_by_tl,
                    fps,
                )?;
                tl_builder = tl_builder.sequence(duration, node);

                let Some(&next_id) = child_ids.get(index + 1) else {
                    continue;
                };
                if let Some(transition) = transitions_by_pair.get(&(child_el.id.as_str(), next_id))
                {
                    tl_builder = tl_builder.transition(build_transition(transition)?);
                }
            }

            let timeline_node: Node = tl_builder.into();
            let mut kind = timeline_node.kind().clone();
            *kind.style_mut() = style;
            Ok(Node::new(kind))
        }
        ParsedElementKind::Div => {
            let mut div_node = div();
            div_node.style = style;

            if let Some(children) = children_map.get(el.id.as_str()) {
                for child in children {
                    let child_node = build_node_inner(
                        child,
                        children_map,
                        scripts_by_parent,
                        transitions_by_tl,
                        fps,
                    )?;
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
        ParsedElementKind::Caption { path } => {
            let entries = std::fs::read_to_string(path)
                .ok()
                .and_then(|content| parse_srt(&content, fps).ok())
                .unwrap_or_default();
            let mut caption_node = caption().path(path).entries(entries);
            caption_node.style = style;
            Ok(Node::new(caption_node))
        }
    }
}

fn build_transition(transition: &ParsedTransition) -> anyhow::Result<Transition> {
    let easing = parse_transition_easing(transition)?;
    let duration = transition.duration;
    let effect = normalize_transition_name(&transition.effect);

    match effect.as_str() {
        "fade" => Ok(fade().timing(easing, duration)),
        "clock_wipe" => Ok(clock_wipe().timing(easing, duration)),
        "iris" => Ok(iris().timing(easing, duration)),
        "slide" => match transition
            .direction
            .as_deref()
            .map(normalize_transition_name)
            .as_deref()
        {
            None | Some("from_left") => Ok(slide().from_left().timing(easing, duration)),
            Some("from_right") => Ok(slide().from_right().timing(easing, duration)),
            Some("from_top") => Ok(slide().from_top().timing(easing, duration)),
            Some("from_bottom") => Ok(slide().from_bottom().timing(easing, duration)),
            Some(direction) => Err(anyhow::anyhow!("unsupported slide direction `{direction}`")),
        },
        "wipe" => match transition
            .direction
            .as_deref()
            .map(normalize_transition_name)
            .as_deref()
        {
            None | Some("from_left") => Ok(wipe().from_left().timing(easing, duration)),
            Some("from_right") => Ok(wipe().from_right().timing(easing, duration)),
            Some("from_top") => Ok(wipe().from_top().timing(easing, duration)),
            Some("from_bottom") => Ok(wipe().from_bottom().timing(easing, duration)),
            Some("from_top_left") => Ok(wipe().from_top_left().timing(easing, duration)),
            Some("from_top_right") => Ok(wipe().from_top_right().timing(easing, duration)),
            Some("from_bottom_left") => Ok(wipe().from_bottom_left().timing(easing, duration)),
            Some("from_bottom_right") => Ok(wipe().from_bottom_right().timing(easing, duration)),
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
            Ok(builder.timing(easing, duration))
        }
        _ => Err(anyhow::anyhow!(
            "unsupported transition effect `{}`",
            transition.effect
        )),
    }
}

fn parse_transition_easing(transition: &ParsedTransition) -> anyhow::Result<Easing> {
    // Explicit spring parameters override timing field
    if transition.damping.is_some() || transition.stiffness.is_some() || transition.mass.is_some() {
        let mut config = SpringConfig::default();
        if let Some(damping) = transition.damping {
            config.damping = damping;
        }
        if let Some(stiffness) = transition.stiffness {
            config.stiffness = stiffness;
        }
        if let Some(mass) = transition.mass {
            config.mass = mass;
        }
        return Ok(Easing::Spring(config));
    }

    let timing = transition
        .timing
        .as_deref()
        .map(normalize_transition_name)
        .unwrap_or_else(|| "linear".to_string());

    easing_from_name(&timing).ok_or_else(|| {
        anyhow::anyhow!(
            "unsupported transition timing `{}`",
            transition.timing.as_deref().unwrap_or("unknown")
        )
    })
}

fn normalize_transition_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}
