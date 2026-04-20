use std::collections::{HashMap, HashSet};

use crate::scene::{
    easing::{Easing, SpringConfig, easing_from_name},
    layer::layer,
    node::Node,
    primitives::{ImageSource, canvas, caption, div, image, lucide, parse_srt, text, video},
    script::ScriptDriver,
    transition::{Transition, clock_wipe, fade, iris, light_leak, slide, timeline, wipe},
};

use super::{ParsedElement, ParsedElementKind, ParsedTransition, TimelineEntry};

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
    build_node(root, &children_map, scripts_by_parent, fps)
}

pub(super) fn build_timeline(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    entries: &[TimelineEntry],
    fps: u32,
) -> anyhow::Result<(Node, i32)> {
    let (children_map, roots) = index_elements(elements);
    let roots_by_id = roots
        .into_iter()
        .map(|root| (root.id.as_str(), root))
        .collect::<HashMap<_, _>>();
    let root_ids = entries
        .iter()
        .filter_map(|entry| match entry {
            TimelineEntry::SequenceRoot { id } => Some(id.as_str()),
            TimelineEntry::Transition(_) => None,
        })
        .collect::<Vec<_>>();
    let root_positions = root_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect::<HashMap<_, _>>();
    let mut transitions_by_pair = HashMap::new();

    for entry in entries {
        let TimelineEntry::Transition(transition) = entry else {
            continue;
        };

        let Some(&from_index) = root_positions.get(transition.from.as_str()) else {
            return Err(anyhow::anyhow!(
                "transition references missing sequence root `{}`",
                transition.from
            ));
        };
        let Some(&to_index) = root_positions.get(transition.to.as_str()) else {
            return Err(anyhow::anyhow!(
                "transition references missing sequence root `{}`",
                transition.to
            ));
        };

        if to_index != from_index + 1 {
            let next_id = root_ids.get(from_index + 1).copied().unwrap_or("<end>");
            return Err(anyhow::anyhow!(
                "transition declares `{}` -> `{}`, but root order requires adjacent sequences `{}` -> `{}`",
                transition.from,
                transition.to,
                transition.from,
                next_id
            ));
        }

        if transitions_by_pair
            .insert(
                (transition.from.as_str(), transition.to.as_str()),
                transition,
            )
            .is_some()
        {
            return Err(anyhow::anyhow!(
                "duplicate transition declared for `{}` -> `{}`",
                transition.from,
                transition.to
            ));
        }
    }

    let mut timeline_builder = timeline();
    let mut frames = 0_i32;

    for (index, id) in root_ids.iter().enumerate() {
        let root = roots_by_id
            .get(*id)
            .ok_or_else(|| anyhow::anyhow!("sequence root `{id}` was not found"))?;
        let duration = root
            .duration
            .ok_or_else(|| anyhow::anyhow!("timeline sequence `{id}` is missing a duration"))?;
        let node = build_node(root, &children_map, scripts_by_parent, fps)?;
        timeline_builder = timeline_builder.sequence(duration, node);
        frames += duration as i32;

        let Some(next_id) = root_ids.get(index + 1) else {
            continue;
        };
        if let Some(transition) = transitions_by_pair.get(&(*id, *next_id)) {
            timeline_builder = timeline_builder.transition(build_transition(transition)?);
            frames += transition.duration as i32;
        }
    }

    Ok((timeline_builder.into(), frames))
}

fn build_node(
    el: &ParsedElement,
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    scripts_by_parent: &HashMap<String, Vec<String>>,
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
        ParsedElementKind::Div => {
            let mut div_node = div();
            div_node.style = style;

            if let Some(children) = children_map.get(el.id.as_str()) {
                for child in children {
                    let child_node = build_node(child, children_map, scripts_by_parent, fps)?;
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

pub(super) fn build_layer_root(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    timeline_entries: &[TimelineEntry],
    layer_children: &[String],
    fps: u32,
) -> anyhow::Result<(Node, i32)> {
    let (children_map, roots) = index_elements(elements);
    let roots_by_id: HashMap<&str, &ParsedElement> =
        roots.iter().map(|root| (root.id.as_str(), *root)).collect();

    let transition_to: HashSet<&str> = timeline_entries
        .iter()
        .filter_map(|e| match e {
            TimelineEntry::Transition(t) => Some(t.to.as_str()),
            _ => None,
        })
        .collect();

    let chain_starters: HashSet<&str> = timeline_entries
        .iter()
        .filter_map(|e| match e {
            TimelineEntry::SequenceRoot { id } => {
                let id_str = id.as_str();
                if !transition_to.contains(id_str) {
                    Some(id_str)
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    let mut layer_node = layer();
    let mut total_frames = 0_i32;

    for child_id in layer_children {
        let child_id_str = child_id.as_str();

        if chain_starters.contains(child_id_str) {
            let chain_entries = trace_chain(child_id_str, timeline_entries);
            let (timeline_node, chain_frames) = build_chain_from_entries(
                &chain_entries,
                &children_map,
                &roots_by_id,
                scripts_by_parent,
                fps,
            )?;
            layer_node = layer_node.child(timeline_node);
            total_frames = total_frames.max(chain_frames);
        } else if transition_to.contains(child_id_str) {
            continue;
        } else {
            let root = roots_by_id
                .get(child_id_str)
                .ok_or_else(|| anyhow::anyhow!("layer child `{child_id}` not found in elements"))?;
            let node = build_node(root, &children_map, scripts_by_parent, fps)?;
            if let Some(dur) = root.duration {
                total_frames = total_frames.max(dur as i32);
            }
            layer_node = layer_node.child(node);
        }
    }

    Ok((layer_node.into(), total_frames))
}

fn trace_chain<'a>(start_id: &str, entries: &'a [TimelineEntry]) -> Vec<&'a TimelineEntry> {
    let transitions_by_from: HashMap<&str, &ParsedTransition> = entries
        .iter()
        .filter_map(|e| match e {
            TimelineEntry::Transition(t) => Some((t.from.as_str(), t)),
            _ => None,
        })
        .collect();

    let mut chain = Vec::new();
    let mut current = start_id.to_string();
    loop {
        chain.push(
            entries
                .iter()
                .find(|e| matches!(e, TimelineEntry::SequenceRoot { id } if *id == current))
                .unwrap(),
        );
        if let Some(t) = transitions_by_from.get(current.as_str()) {
            chain.push(
                entries
                    .iter()
                    .find(|e| matches!(e, TimelineEntry::Transition(pt) if pt.from == t.from && pt.to == t.to))
                    .unwrap(),
            );
            current = t.to.clone();
        } else {
            break;
        }
    }
    chain
}

fn build_chain_from_entries(
    chain: &[&TimelineEntry],
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    roots_by_id: &HashMap<&str, &ParsedElement>,
    scripts_by_parent: &HashMap<String, Vec<String>>,
    fps: u32,
) -> anyhow::Result<(Node, i32)> {
    let mut transitions_by_pair: HashMap<(&str, &str), &ParsedTransition> = HashMap::new();
    for entry in chain {
        if let TimelineEntry::Transition(t) = entry {
            transitions_by_pair.insert((t.from.as_str(), t.to.as_str()), t);
        }
    }

    let chain_root_ids: Vec<&str> = chain
        .iter()
        .filter_map(|e| match e {
            TimelineEntry::SequenceRoot { id } => Some(id.as_str()),
            _ => None,
        })
        .collect();

    let mut timeline_builder = timeline();
    let mut frames = 0_i32;

    for (index, id) in chain_root_ids.iter().enumerate() {
        let root = roots_by_id
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("sequence root `{id}` was not found"))?;
        let duration = root
            .duration
            .ok_or_else(|| anyhow::anyhow!("timeline sequence `{id}` is missing a duration"))?;
        let node = build_node(root, children_map, scripts_by_parent, fps)?;
        timeline_builder = timeline_builder.sequence(duration, node);
        frames += duration as i32;

        let Some(next_id) = chain_root_ids.get(index + 1) else {
            continue;
        };
        if let Some(transition) = transitions_by_pair.get(&(id, next_id)) {
            timeline_builder = timeline_builder.transition(build_transition(transition)?);
            frames += transition.duration as i32;
        }
    }

    Ok((timeline_builder.into(), frames))
}

fn normalize_transition_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}
