use std::collections::HashMap;

use crate::parse::{
    easing::{Easing, SpringConfig, easing_from_name},
    node::Node,
    primitives::{
        ImageSource, VideoSource, canvas, div, image, lucide, path, text, video, video_url,
    },
    transition::{
        Transition, clock_wipe, cut, fade, gl_transition, iris, light_leak, slide, timeline, wipe,
    },
};
use crate::script::ScriptDriver;

use crate::fonts::{FontFamilyIndex, merge_faces_into_db};
use crate::parse::document::{
    DeclaredScript, ParsedElement, ParsedElementKind, ParsedTransition,
};

pub fn join_scripts(scripts: Vec<String>) -> Option<String> {
    if scripts.is_empty() {
        None
    } else {
        Some(scripts.join("\n"))
    }
}

/// Join global scripts for `ParsedComposition.script` (legacy field).
/// External-only globals leave `None` here; host injection fills drivers on nodes.
/// Mixed globals that are all-inline still populate the legacy string field.
fn join_global_scripts(scripts: Vec<DeclaredScript>) -> Option<String> {
    let mut inlines = Vec::new();
    for s in scripts {
        match s {
            DeclaredScript::Inline(t) => inlines.push(t),
            DeclaredScript::External { .. } => {
                // External globals are requirements; the string field only mirrors
                // inline content for back-compat tests.
            }
        }
    }
    join_scripts(inlines)
}

/// Build a [`ScriptDriver`] from one or more declarations.
///
/// - All-inline → single driver with joined source.
/// - Any external → one driver (first external wins for asset id) with remaining
///   pieces recorded; prepare injects external text then re-joins.
///
/// Build fontdb + family index from manifest bytes merged into `base_db`.
/// Build a [`ScriptDriver`] from one or more declarations.
///
/// - All-inline → single driver with joined source.
/// - Any external → one driver (first external wins for asset id) with remaining
///   pieces recorded; prepare injects external text then re-joins.
///
/// Multiple externals on the same parent are concatenated at prepare in order.
pub fn script_driver_from_decls(
    scripts: Vec<DeclaredScript>,
) -> anyhow::Result<Option<ScriptDriver>> {
    if scripts.is_empty() {
        return Ok(None);
    }

    let all_inline = scripts
        .iter()
        .all(|s| matches!(s, DeclaredScript::Inline(_)));
    if all_inline {
        let joined = scripts
            .into_iter()
            .map(|s| match s {
                DeclaredScript::Inline(t) => t,
                DeclaredScript::External { .. } => unreachable!(),
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(Some(ScriptDriver::from_source(&joined)?));
    }

    // Mixed or pure-external: keep pieces. Use the first external locator as the
    // primary asset id for requirements; remaining externals are also required
    // via the driver's external list (stored as joined locators at prepare).
    // For install identity we hash the ordered locator list.
    let mut pieces_inline = Vec::new();
    let mut externals = Vec::new();
    for s in scripts {
        match s {
            DeclaredScript::Inline(t) => pieces_inline.push(t),
            DeclaredScript::External { locator } => externals.push(locator),
        }
    }

    if externals.len() == 1 && pieces_inline.is_empty() {
        return Ok(Some(ScriptDriver::from_external(externals.remove(0))));
    }

    // Multiple externals / mix: encode as a single synthetic external whose
    // host must supply the fully joined text under a compound key, plus we
    // also declare each path as a requirement. Simpler contract: require host
    // to supply each external piece; prepare joins with inlines.
    Ok(Some(ScriptDriver::from_pieces(pieces_inline, externals)))
}

pub fn build_font_resources(
    base_db: fontdb::Database,
    manifest: &crate::fonts::FontManifest,
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

pub fn build_tree(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<DeclaredScript>>,
) -> anyhow::Result<Node> {
    let (children_map, roots) = index_elements(elements);
    if roots.len() > 1 {
        return Err(anyhow::anyhow!("multiple root elements found"));
    }

    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    build_node_inner(root, &children_map, scripts_by_parent, &HashMap::new())
}

pub fn build_tree_with_tl(
    elements: &[ParsedElement],
    scripts_by_parent: &HashMap<String, Vec<DeclaredScript>>,
    transitions_by_tl: &HashMap<String, Vec<&ParsedTransition>>,
) -> anyhow::Result<Node> {
    let (children_map, roots) = index_elements(elements);
    if roots.len() > 1 {
        return Err(anyhow::anyhow!("multiple root elements found"));
    }

    let root = roots
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no root element found"))?;
    build_node_inner(root, &children_map, scripts_by_parent, transitions_by_tl)
}

fn build_node_inner(
    el: &ParsedElement,
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    scripts_by_parent: &HashMap<String, Vec<DeclaredScript>>,
    transitions_by_tl: &HashMap<String, Vec<&ParsedTransition>>,
) -> anyhow::Result<Node> {
    let mut style = el.style.clone();
    style.id = el.id.clone();
    if let Some(decls) = scripts_by_parent.get(el.id.as_str())
        && let Some(driver) = script_driver_from_decls(decls.clone())?
    {
        style.script_driver = Some(std::sync::Arc::new(driver));
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

            // Adjacent scenes need not declare a transition: without one the
            // two scenes cut straight from one to the next.

            for (index, child_el) in children.iter().enumerate() {
                let duration = child_el.duration.ok_or_else(|| {
                    anyhow::anyhow!("timeline sequence `{}` is missing a duration", child_el.id)
                })?;
                let node = build_node_inner(
                    child_el,
                    children_map,
                    scripts_by_parent,
                    transitions_by_tl,
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
            // Hidden picture subtree: children rasterize offscreen and are only
            // reachable through `canvas.getSubTree().makeShader(...)`.
            for child in children {
                let child_node = build_node_inner(
                    child,
                    children_map,
                    scripts_by_parent,
                    transitions_by_tl,
                )?;
                canvas_node = canvas_node.hidden_child(child_node);
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
        ParsedElementKind::Path { data, view_box } => {
            let mut path_node = path(data);
            if let Some(vb) = view_box {
                path_node.set_view_box(*vb);
            }
            path_node.style = style;
            Ok(Node::new(path_node))
        }
        ParsedElementKind::Svg {
            src,
            width,
            height,
            preserve_aspect_ratio,
        } => {
            let mut svg_node = crate::parse::primitives::svg(src);
            svg_node.width = *width;
            svg_node.height = *height;
            svg_node.preserve_aspect_ratio = preserve_aspect_ratio.clone();
            svg_node.style = style;
            Ok(Node::new(svg_node))
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
                    )?;
                    video_node = video_node.child(child_node);
                }
            }
            Ok(Node::new(video_node))
        }
    }
}

fn build_transition(transition: &ParsedTransition) -> anyhow::Result<Transition> {
    let easing = parse_transition_easing(transition)?;
    let duration = transition.duration;
    let effect = normalize_transition_name(&transition.effect);

    match effect.as_str() {
        "fade" => Ok(fade().timing(easing, duration)),
        // Consumes the overlap but applies no effect: the incoming scene covers
        // the outgoing one outright, i.e. a hard cut at the seam.
        "cut" | "none" => Ok(cut().timing(easing, duration)),
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
