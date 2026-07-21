use std::collections::HashMap;

use crate::parse::{
    easing::{Easing, SpringConfig, easing_from_name},
    node::Node,
    primitives::{
        ImageSource, VideoSource, canvas, caption, div, image, lucide, path, text, video,
        video_url,
    },
    transition::{
        Transition, clock_wipe, fade, gl_transition, iris, light_leak, slide, timeline, wipe,
    },
};
use crate::script::ScriptDriver;

use crate::parse::composition::{AudioAttachment, CompositionAudioSource};
use crate::parse::document::{
    CanvasChildrenMode, ParsedComposition, ParsedDocumentParts, ParsedElement, ParsedElementKind,
    ParsedTransition,
};
use crate::resource::fonts::{FontFamilyIndex, merge_faces_into_db};

#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    pub canvas_children_mode: CanvasChildrenMode,
}

impl BuildOptions {
    pub const JSONL: Self = Self {
        canvas_children_mode: CanvasChildrenMode::Forbid,
    };

    pub const MARKUP: Self = Self {
        canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
    };
}

pub fn join_scripts(scripts: Vec<String>) -> Option<String> {
    if scripts.is_empty() {
        None
    } else {
        Some(scripts.join("\n"))
    }
}

pub fn build_parsed_document(
    mut parts: ParsedDocumentParts,
    options: BuildOptions,
    font_index: Option<&FontFamilyIndex>,
) -> anyhow::Result<ParsedComposition> {
    // Core always applies `font-sans` / `font-[id]` from the document manifest.
    // Callers may still pass a loaded index (e.g. after face load), but a pure
    // parse path without host font bytes must resolve the same family names.
    if !parts.font_manifest.is_empty() {
        let owned = font_index
            .cloned()
            .unwrap_or_else(|| parts.font_manifest.build_family_index());
        parts
            .font_manifest
            .apply_font_refs_to_styles(&owned, &mut parts.elements);
    }

    let audio_sources: Vec<CompositionAudioSource> = parts
        .audio_elements
        .iter()
        .map(|audio| {
            let attach = audio.attach.clone();
            let element = parts.elements.iter().find(|el| el.id == attach);
            let attachment = match element {
                Some(el) if matches!(el.kind, ParsedElementKind::Timeline) => {
                    AudioAttachment::Timeline
                }
                Some(_) => AudioAttachment::Scene { scene_id: attach },
                None => anyhow::bail!(
                    "audio `{}` attach references non-existent element `{}`",
                    audio.id,
                    attach
                ),
            };
            Ok(CompositionAudioSource {
                id: audio.id.clone(),
                source: audio.source.clone(),
                attach: attachment,
                duration_secs: audio.duration,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let transitions_by_tl: HashMap<String, Vec<&ParsedTransition>> = {
        let mut map: HashMap<String, Vec<&ParsedTransition>> = HashMap::new();
        for t in &parts.transitions {
            map.entry(t.parent_id.clone()).or_default().push(t);
        }
        map
    };

    let mut root = if parts.transitions.is_empty() {
        build_tree_with_options(
            &parts.elements,
            &parts.scripts_by_parent,
            parts.fps as u32,
            options,
        )?
    } else {
        build_tree_with_tl_options(
            &parts.elements,
            &parts.scripts_by_parent,
            &transitions_by_tl,
            parts.fps as u32,
            options,
        )?
    };

    if let Some(script) = parts.markup_root_script {
        root = root.script_source(&script)?;
    }

    Ok(ParsedComposition {
        width: parts.width,
        height: parts.height,
        fps: parts.fps,
        duration: parts.duration,
        root,
        script: join_scripts(parts.global_scripts),
        audio_sources,
        font_manifest: parts.font_manifest,
    })
}

/// Build fontdb + family index from manifest bytes merged into `base_db`.
pub fn build_font_resources(
    base_db: fontdb::Database,
    manifest: &crate::resource::fonts::FontManifest,
    bytes_by_id: &std::collections::HashMap<String, Vec<u8>>,
) -> anyhow::Result<(fontdb::Database, FontFamilyIndex)> {
    merge_faces_into_db(base_db, manifest, bytes_by_id).map_err(|e| anyhow::anyhow!("{e}"))
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

pub fn build_tree_with_options(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    _fps: u32,
    options: BuildOptions,
) -> anyhow::Result<Node> {
    let (children_map, roots) = index_elements(elements);
    if roots.len() > 1 {
        return Err(anyhow::anyhow!("multiple root elements found"));
    }

    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    build_node_inner(
        root,
        &children_map,
        scripts_by_parent,
        &HashMap::new(),
        options,
    )
}

pub fn build_tree_with_tl_options(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    transitions_by_tl: &HashMap<String, Vec<&ParsedTransition>>,
    _fps: u32,
    options: BuildOptions,
) -> anyhow::Result<Node> {
    let (children_map, roots) = index_elements(elements);
    if roots.len() > 1 {
        return Err(anyhow::anyhow!("multiple root elements found"));
    }

    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    build_node_inner(
        root,
        &children_map,
        scripts_by_parent,
        transitions_by_tl,
        options,
    )
}

pub fn build_tree(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    fps: u32,
) -> anyhow::Result<Node> {
    build_tree_with_options(elements, scripts_by_parent, fps, BuildOptions::JSONL)
}

pub fn build_tree_with_tl(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<String>>,
    transitions_by_tl: &HashMap<String, Vec<&ParsedTransition>>,
    fps: u32,
) -> anyhow::Result<Node> {
    build_tree_with_tl_options(
        elements,
        scripts_by_parent,
        transitions_by_tl,
        fps,
        BuildOptions::JSONL,
    )
}

fn build_node_inner(
    el: &ParsedElement,
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    scripts_by_parent: &HashMap<String, Vec<String>>,
    transitions_by_tl: &HashMap<String, Vec<&ParsedTransition>>,
    options: BuildOptions,
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
                    anyhow::anyhow!("timeline sequence `{}` is missing a duration", child_el.id)
                })?;
                let node = build_node_inner(
                    child_el,
                    children_map,
                    scripts_by_parent,
                    transitions_by_tl,
                    options,
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
                        options,
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
            let children = children_map
                .get(el.id.as_str())
                .map(|c| c.as_slice())
                .unwrap_or(&[]);
            let mut canvas_node = canvas();
            canvas_node.style = style;
            match options.canvas_children_mode {
                CanvasChildrenMode::Forbid if !children.is_empty() => {
                    return Err(anyhow::anyhow!(
                        "canvas node `{}` cannot have child nodes",
                        el.id
                    ));
                }
                CanvasChildrenMode::Forbid => {}
                CanvasChildrenMode::HiddenPictureSubtree => {
                    for child in children {
                        let child_node = build_node_inner(
                            child,
                            children_map,
                            scripts_by_parent,
                            transitions_by_tl,
                            options,
                        )?;
                        canvas_node = canvas_node.hidden_child(child_node);
                    }
                }
            }
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
        ParsedElementKind::Lottie { source, timing } => {
            let mut lottie_node = crate::parse::primitives::lottie();
            lottie_node = match source {
                crate::parse::primitives::LottieSource::Unset => {
                    return Err(anyhow::anyhow!(
                        "lottie node requires one of: path, url, src"
                    ));
                }
                crate::parse::primitives::LottieSource::Path(path) => lottie_node.path(path),
                crate::parse::primitives::LottieSource::Url(url) => lottie_node.url(url.clone()),
            };
            lottie_node = lottie_node.with_timing(*timing);
            lottie_node.style = style;
            Ok(Node::new(lottie_node))
        }
        ParsedElementKind::Icon { name } => {
            let mut icon_node = lucide(name.clone());
            icon_node.style = style;
            Ok(Node::new(icon_node))
        }
        ParsedElementKind::Path { data } => {
            let mut path_node = path(data);
            path_node.style = style;
            Ok(Node::new(path_node))
        }
        ParsedElementKind::Video { source, timing } => {
            let mut video_node = match source {
                VideoSource::Path(p) => video(p),
                VideoSource::Url(u) => video_url(u),
            };
            video_node = video_node.with_timing(*timing);
            video_node.style = style;
            if let Some(children) = children_map.get(el.id.as_str()) {
                for child in children {
                    let child_node = build_node_inner(
                        child,
                        children_map,
                        scripts_by_parent,
                        transitions_by_tl,
                        options,
                    )?;
                    video_node = video_node.child(child_node);
                }
            }
            Ok(Node::new(video_node))
        }
        ParsedElementKind::Caption { path } => {
            let mut caption_node = caption().path(path);
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
        _ => Ok(gl_transition(transition.effect.clone()).timing(easing, duration)),
    }
}

fn parse_transition_easing(transition: &ParsedTransition) -> anyhow::Result<Easing> {
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
