#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtractedMarkup {
    pub xml: String,
    pub script: Option<String>,
}

fn is_xml_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

fn find_script_open(input: &str) -> anyhow::Result<Option<(usize, usize)>> {
    let mut chars = input.char_indices().peekable();
    while let Some((idx, c)) = chars.next() {
        if c == '<' {
            let remaining = &input[idx..];
            if remaining.starts_with("</script") {
                anyhow::bail!("unexpected </script> without matching <script>");
            }
            if remaining.starts_with("<script") {
                let after_tag = idx + 7;
                let rest = &input[after_tag..];
                let mut end = after_tag;
                let mut saw_gt = false;
                let mut saw_attrs = false;
                let mut saw_slash = false;
                for (_, c) in rest.char_indices() {
                    end += c.len_utf8();
                    if c == '>' {
                        if saw_slash {
                            anyhow::bail!("self-closing <script/> is not allowed");
                        }
                        saw_gt = true;
                        break;
                    } else if c == '/' {
                        saw_slash = true;
                    } else if !is_xml_whitespace(c) {
                        saw_attrs = true;
                    }
                }
                if !saw_gt {
                    anyhow::bail!("unclosed <script> tag");
                }
                if saw_attrs {
                    anyhow::bail!("<script> tag with attributes is not allowed");
                }
                return Ok(Some((idx, end)));
            }
        }
    }
    Ok(None)
}

fn find_script_close(input: &str, from: usize) -> anyhow::Result<(usize, usize)> {
    let search_from = &input[from..];
    let mut last_close: Option<(usize, usize)> = None;

    let mut chars = search_from.char_indices().peekable();
    while let Some((rel_idx, c)) = chars.next() {
        if c == '<' {
            let remaining = &search_from[rel_idx..];
            if remaining.starts_with("</script") {
                let after_close_tag = rel_idx + 8;
                let rest = &search_from[after_close_tag..];
                let mut end = after_close_tag;
                let mut saw_gt = false;
                for (_, c) in rest.char_indices() {
                    end += c.len_utf8();
                    if c == '>' {
                        saw_gt = true;
                        break;
                    } else if !is_xml_whitespace(c) {
                        break;
                    }
                }
                if saw_gt {
                    last_close = Some((from + rel_idx, from + end));
                }
            }
        }
    }

    last_close.ok_or_else(|| anyhow::anyhow!("unclosed <script> tag (missing </script>)"))
}

fn ensure_direct_opencat_child(input: &str, open_start: usize) -> anyhow::Result<()> {
    let before_script = &input[..open_start];

    let mut depth = 0i32;
    let mut i = 0;
    let bytes = before_script.as_bytes();

    while i < before_script.len() {
        if bytes[i] == b'<' {
            let rest = &before_script[i..];

            if rest.starts_with("<!--") {
                if let Some(e) = rest.find("-->") {
                    i += e + 3;
                    continue;
                }
                i += 1;
                continue;
            }
            if rest.starts_with("<?") {
                if let Some(e) = rest.find("?>") {
                    i += e + 2;
                    continue;
                }
                i += 1;
                continue;
            }
            if rest.starts_with("<![CDATA[") {
                if let Some(e) = rest.find("]]>") {
                    i += e + 3;
                    continue;
                }
                i += 1;
                continue;
            }

            if let Some(e) = rest.find('>') {
                let trimmed = rest[1..e].trim_end();
                if rest.starts_with("</") {
                    depth -= 1;
                } else if !trimmed.ends_with('/') {
                    depth += 1;
                }
                i += e + 1;
                continue;
            }
        }
        i += 1;
    }

    if depth != 1 {
        anyhow::bail!("<script> must be a direct child of <opencat>");
    }
    Ok(())
}

fn reject_remaining_script_tags(xml: &str) -> anyhow::Result<()> {
    for (idx, c) in xml.char_indices() {
        if c == '<' {
            let remaining = &xml[idx..];
            if remaining.starts_with("<script") || remaining.starts_with("</script") {
                anyhow::bail!("unexpected <script> or </script> after script extraction");
            }
        }
    }
    Ok(())
}

pub(crate) fn extract_raw_script(input: &str) -> anyhow::Result<ExtractedMarkup> {
    let Some((open_start, open_end)) = find_script_open(input)? else {
        reject_remaining_script_tags(input)?;
        return Ok(ExtractedMarkup {
            xml: input.to_string(),
            script: None,
        });
    };

    ensure_direct_opencat_child(input, open_start)?;

    let (close_start, close_end) = find_script_close(input, open_end)?;

    let script = input[open_end..close_start].to_string();

    let mut xml = String::with_capacity(input.len() - (close_end - open_start));
    xml.push_str(&input[..open_start]);
    xml.push_str(&input[close_end..]);

    reject_remaining_script_tags(&xml)?;

    Ok(ExtractedMarkup {
        xml,
        script: Some(script),
    })
}

use std::path::PathBuf;

use crate::parse::document::{
    BuildOptions, CanvasChildrenMode, ParsedAudioElement, ParsedComposition, ParsedDocumentParts,
    ParsedElement, ParsedElementKind, ParsedTransition, build_parsed_document,
};
use crate::parse::primitives::{AudioSource, ImageSource, OpenverseQuery, VideoSource};

pub fn parse(input: &str) -> anyhow::Result<ParsedComposition> {
    parse_with_base_dir(input, None)
}

pub fn parse_with_base_dir(
    input: &str,
    base_dir: Option<&std::path::Path>,
) -> anyhow::Result<ParsedComposition> {
    let extracted = extract_raw_script(input)?;
    let doc = roxmltree::Document::parse(&extracted.xml)?;
    let root = doc.root_element();
    if root.tag_name().name() != "opencat" {
        anyhow::bail!("markup document root must be <opencat>");
    }
    ensure_allowed_attrs(root, &["width", "height", "fps", "frames"])?;
    let mut parts = ParsedDocumentParts {
        width: parse_positive_i32_attr(root, "width", 1920)?,
        height: parse_positive_i32_attr(root, "height", 1080)?,
        fps: parse_positive_i32_attr(root, "fps", 30)?,
        frames: parse_positive_i32_attr(root, "frames", 90)?,
        markup_root_script: extracted.script,
        ..Default::default()
    };
    parse_opencat_children(root, base_dir, &mut parts)?;

    build_parsed_document(
        parts,
        BuildOptions {
            canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
        },
    )
}

const DIV_ATTRS: &[&str] = &["id", "class", "duration"];
const TEXT_ATTRS: &[&str] = &["id", "class", "duration"];
const CANVAS_ATTRS: &[&str] = &["id", "class", "duration"];
const IMAGE_ATTRS: &[&str] = &[
    "id",
    "class",
    "duration",
    "path",
    "url",
    "query",
    "queryCount",
    "aspectRatio",
];
const AUDIO_ATTRS: &[&str] = &["id", "duration", "path", "url"];
const VIDEO_ATTRS: &[&str] = &["id", "class", "duration", "path", "url"];
const ICON_ATTRS: &[&str] = &["id", "class", "duration", "icon"];
const TL_ATTRS: &[&str] = &["id", "class"];
const CAPTION_ATTRS: &[&str] = &["id", "class", "duration", "path"];
const PATH_ATTRS: &[&str] = &["id", "class", "duration", "d"];
const TRANSITION_ATTRS: &[&str] = &[
    "from",
    "to",
    "effect",
    "duration",
    "direction",
    "timing",
    "damping",
    "stiffness",
    "mass",
    "seed",
    "hueShift",
    "maskScale",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParentContext {
    Root,
    Div,
    Text,
    Canvas,
    Timeline,
    Transition,
}

fn parse_opencat_children(
    root: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    let mut visual_root: Option<String> = None;

    for child in root.children() {
        match child.node_type() {
            roxmltree::NodeType::Element => {
                let tag = child.tag_name().name();
                match tag {
                    "audio" => {
                        parse_audio_element(child, None, base_dir, parts)?;
                    }
                    "div" | "text" | "canvas" | "image" | "video" | "icon" | "path" | "caption"
                    | "tl" => {
                        let id = required_attr(child, "id")?;
                        if visual_root.is_some() {
                            anyhow::bail!("multiple visual root elements found");
                        }
                        visual_root = Some(id.to_string());
                        parse_visual_node(child, None, base_dir, parts, ParentContext::Root)?;
                    }
                    "transition" => {
                        anyhow::bail!("transition must be a direct child of <tl>");
                    }
                    other => anyhow::bail!("unknown element <{other}>"),
                }
            }
            roxmltree::NodeType::Comment => {}
            roxmltree::NodeType::Text => {
                if !child.text().unwrap_or("").trim().is_empty() {
                    anyhow::bail!("non-whitespace text outside elements is not allowed");
                }
            }
            _ => {
                anyhow::bail!("processing instructions and other node types are not allowed")
            }
        }
    }

    if visual_root.is_none() {
        anyhow::bail!("markup document must have a visual root element");
    }

    Ok(())
}

fn parse_visual_node(
    node: roxmltree::Node<'_, '_>,
    parent_id: Option<&str>,
    base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
    _parent_context: ParentContext,
) -> anyhow::Result<()> {
    let tag = node.tag_name().name();
    let id = required_attr(node, "id")?;
    let mut style = crate::style::NodeStyle::default();
    if let Some(class) = node.attribute("class") {
        style = crate::parse::jsonl::tailwind::parse_class_name(class);
    }
    let duration = parse_optional_u32_attr(node, "duration")?;

    match tag {
        "div" => {
            ensure_allowed_attrs(node, DIV_ATTRS)?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Div,
            });
            for child in node.children() {
                match child.node_type() {
                    roxmltree::NodeType::Element => {
                        let child_tag = child.tag_name().name();
                        match child_tag {
                            "div" | "text" | "canvas" | "image" | "video" | "icon" | "path"
                            | "caption" | "audio" | "tl" => {
                                parse_visual_node(
                                    child,
                                    Some(&id),
                                    base_dir,
                                    parts,
                                    ParentContext::Div,
                                )?;
                            }
                            "transition" => {
                                anyhow::bail!("transition must be a direct child of <tl>");
                            }
                            other => anyhow::bail!("unknown element <{other}>"),
                        }
                    }
                    roxmltree::NodeType::Comment => {}
                    roxmltree::NodeType::Text => {
                        if !child.text().unwrap_or("").trim().is_empty() {
                            anyhow::bail!(
                                "non-whitespace text is not allowed outside <text> elements"
                            );
                        }
                    }
                    _ => anyhow::bail!("processing instructions not allowed"),
                }
            }
        }
        "text" => {
            ensure_allowed_attrs(node, TEXT_ATTRS)?;
            let mut content = String::new();
            for child in node.children() {
                match child.node_type() {
                    roxmltree::NodeType::Text => {
                        content.push_str(child.text().unwrap_or(""));
                    }
                    roxmltree::NodeType::Element => {
                        anyhow::bail!("<text> cannot contain child elements");
                    }
                    roxmltree::NodeType::Comment => {}
                    _ => {}
                }
            }
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Text { content },
            });
        }
        "canvas" => {
            ensure_allowed_attrs(node, CANVAS_ATTRS)?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Canvas,
            });
            for child in node.children() {
                match child.node_type() {
                    roxmltree::NodeType::Element => {
                        let child_tag = child.tag_name().name();
                        match child_tag {
                            "div" | "text" | "canvas" | "image" | "video" | "icon" | "path"
                            | "caption" | "tl" => {
                                parse_visual_node(
                                    child,
                                    Some(&id),
                                    base_dir,
                                    parts,
                                    ParentContext::Canvas,
                                )?;
                            }
                            "audio" => {
                                anyhow::bail!("audio is not allowed inside canvas");
                            }
                            "transition" => {
                                anyhow::bail!("transition must be a direct child of <tl>");
                            }
                            other => anyhow::bail!("unknown element <{other}>"),
                        }
                    }
                    roxmltree::NodeType::Comment => {}
                    roxmltree::NodeType::Text => {
                        if !child.text().unwrap_or("").trim().is_empty() {
                            anyhow::bail!(
                                "non-whitespace text is not allowed outside <text> elements"
                            );
                        }
                    }
                    _ => anyhow::bail!("processing instructions not allowed"),
                }
            }
        }
        "image" => {
            ensure_allowed_attrs(node, IMAGE_ATTRS)?;
            let source = parse_image_source(node)?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Image { source },
            });
            validate_no_element_children(node, "image")?;
        }
        "video" => {
            ensure_allowed_attrs(node, VIDEO_ATTRS)?;
            let source = parse_video_source(node)?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Video { source },
            });
            validate_no_element_children(node, "video")?;
        }
        "icon" => {
            ensure_allowed_attrs(node, ICON_ATTRS)?;
            let icon_value = required_non_empty_attr(node, "icon")?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Icon {
                    name: icon_value.to_string(),
                },
            });
            validate_no_element_children(node, "icon")?;
        }
        "path" => {
            ensure_allowed_attrs(node, PATH_ATTRS)?;
            let d = required_non_empty_attr(node, "d")?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Path {
                    data: d.to_string(),
                },
            });
            validate_no_element_children(node, "path")?;
        }
        "caption" => {
            ensure_allowed_attrs(node, CAPTION_ATTRS)?;
            let path_str = required_non_empty_attr(node, "path")?;
            let path = PathBuf::from(&path_str);
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Caption { path },
            });
            validate_no_element_children(node, "caption")?;
        }
        "tl" => {
            ensure_allowed_attrs(node, TL_ATTRS)?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration: None,
                style: style.clone(),
                kind: ParsedElementKind::Timeline,
            });
            for child in node.children() {
                match child.node_type() {
                    roxmltree::NodeType::Element => {
                        let child_tag = child.tag_name().name();
                        match child_tag {
                            "div" | "text" | "canvas" | "image" | "video" | "icon" | "path"
                            | "caption" | "tl" => {
                                parse_visual_node(
                                    child,
                                    Some(&id),
                                    base_dir,
                                    parts,
                                    ParentContext::Timeline,
                                )?;
                            }
                            "transition" => {
                                parse_transition_node(child, &id, parts)?;
                            }
                            "audio" => {
                                parse_audio_element(child, Some(&id), base_dir, parts)?;
                            }
                            other => anyhow::bail!("unknown element <{other}>"),
                        }
                    }
                    roxmltree::NodeType::Comment => {}
                    roxmltree::NodeType::Text => {
                        if !child.text().unwrap_or("").trim().is_empty() {
                            anyhow::bail!("non-whitespace text is not allowed inside <tl>");
                        }
                    }
                    _ => anyhow::bail!("processing instructions not allowed"),
                }
            }
        }
        "audio" => {
            parse_audio_element(node, parent_id, base_dir, parts)?;
        }
        _ => anyhow::bail!("unknown element <{tag}>"),
    }

    Ok(())
}

fn parse_audio_element(
    node: roxmltree::Node<'_, '_>,
    parent_id: Option<&str>,
    _base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    let id = required_non_empty_attr(node, "id")?;
    let duration = parse_optional_u32_attr(node, "duration")?;

    let source = match (node.attribute("path"), node.attribute("url")) {
        (Some(p), None) => {
            if p.is_empty() {
                anyhow::bail!("<audio> `path` must not be empty");
            }
            AudioSource::Path(PathBuf::from(p))
        }
        (None, Some(u)) => {
            if u.is_empty() {
                anyhow::bail!("<audio> `url` must not be empty");
            }
            AudioSource::Url(u.to_string())
        }
        (None, None) => {
            anyhow::bail!("<audio> requires one of: path, url");
        }
        (Some(_), Some(_)) => {
            anyhow::bail!("<audio> requires only one of: path, url");
        }
    };

    for attr in node.attributes() {
        let name = attr.name();
        if matches!(name, "className" | "parentId" | "style") {
            anyhow::bail!("attribute `{name}` is not allowed in markup");
        }
        if !AUDIO_ATTRS.contains(&name) {
            anyhow::bail!("unknown attribute `{name}` on <audio>");
        }
    }

    let parent_id = parent_id.map(|s| s.to_string());
    parts.audio_elements.push(ParsedAudioElement {
        id: id.to_string(),
        parent_id,
        duration,
        source,
    });

    validate_no_element_children(node, "audio")?;
    Ok(())
}

fn parse_image_source(node: roxmltree::Node<'_, '_>) -> anyhow::Result<ImageSource> {
    let path = node.attribute("path");
    let url = node.attribute("url");
    let query = node.attribute("query");

    if node.attribute("queryCount").is_some() && query.is_none() {
        anyhow::bail!("<image> `queryCount` requires `query`");
    }
    if node.attribute("aspectRatio").is_some() && query.is_none() {
        anyhow::bail!("<image> `aspectRatio` requires `query`");
    }

    let count = [path.is_some(), url.is_some(), query.is_some()]
        .iter()
        .filter(|&&b| b)
        .count();

    if count == 0 {
        anyhow::bail!("<image> requires one of: path, url, query");
    }
    if count > 1 {
        anyhow::bail!("<image> requires only one of: path, url, query");
    }

    if let Some(p) = path {
        if p.is_empty() {
            anyhow::bail!("<image> `path` must not be empty");
        }
        return Ok(ImageSource::Path(PathBuf::from(p)));
    }
    if let Some(u) = url {
        if u.is_empty() {
            anyhow::bail!("<image> `url` must not be empty");
        }
        return Ok(ImageSource::Url(u.to_string()));
    }
    if let Some(q) = query {
        if q.is_empty() {
            anyhow::bail!("<image> `query` must not be empty");
        }
        let count = node
            .attribute("queryCount")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1);
        if count == 0 {
            anyhow::bail!("<image> `queryCount` must be positive");
        }
        let aspect_ratio = node.attribute("aspectRatio").map(|s| s.to_string());
        return Ok(ImageSource::Query(OpenverseQuery {
            query: q.to_string(),
            count,
            aspect_ratio,
        }));
    }

    anyhow::bail!("image must have one of: path, url, query")
}

fn parse_video_source(node: roxmltree::Node<'_, '_>) -> anyhow::Result<VideoSource> {
    match (node.attribute("path"), node.attribute("url")) {
        (Some(p), None) => {
            if p.is_empty() {
                anyhow::bail!("<video> `path` must not be empty");
            }
            Ok(VideoSource::Path(PathBuf::from(p)))
        }
        (None, Some(u)) => {
            if u.is_empty() {
                anyhow::bail!("<video> `url` must not be empty");
            }
            Ok(VideoSource::Url(u.to_string()))
        }
        (None, None) => {
            anyhow::bail!("<video> requires one of: path, url");
        }
        (Some(_), Some(_)) => {
            anyhow::bail!("<video> requires only one of: path, url");
        }
    }
}

fn parse_transition_node(
    node: roxmltree::Node<'_, '_>,
    parent_tl_id: &str,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    ensure_allowed_attrs(node, TRANSITION_ATTRS)?;

    let from = required_non_empty_attr(node, "from")?.to_string();
    let to = required_non_empty_attr(node, "to")?.to_string();
    let effect = required_non_empty_attr(node, "effect")?.to_string();
    let duration: u32 = {
        let val = required_non_empty_attr(node, "duration")?;
        if !val.bytes().all(|b| b.is_ascii_digit()) {
            anyhow::bail!("<transition> `duration` must be a positive integer (got `{val}`)");
        }
        if val.starts_with('0') && val.len() > 1 {
            anyhow::bail!("<transition> `duration` must not have leading zeros");
        }
        let n: u32 = val
            .parse()
            .map_err(|e| anyhow::anyhow!("<transition> `duration`: {e}"))?;
        if n == 0 {
            anyhow::bail!("<transition> `duration` must be positive");
        }
        n
    };

    if from == to {
        anyhow::bail!("<transition> `from` and `to` must be distinct");
    }

    let direction = node.attribute("direction").map(|s| s.to_string());
    let timing = node.attribute("timing").map(|s| s.to_string());
    let damping = parse_optional_f32_positive(node, "damping")?;
    let stiffness = parse_optional_f32_positive(node, "stiffness")?;
    let mass = parse_optional_f32_positive(node, "mass")?;
    let seed = parse_optional_f32(node, "seed")?;
    let hue_shift = parse_optional_f32(node, "hueShift")?;
    let mask_scale = parse_optional_f32_positive(node, "maskScale")?;

    parts.transitions.push(ParsedTransition {
        parent_id: parent_tl_id.to_string(),
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
    });

    validate_no_element_children(node, "transition")?;
    Ok(())
}

fn parse_optional_f32(node: roxmltree::Node<'_, '_>, name: &str) -> anyhow::Result<Option<f32>> {
    match node.attribute(name) {
        None => Ok(None),
        Some(val) => {
            let n: f32 = val.parse().map_err(|e| anyhow::anyhow!("`{name}`: {e}"))?;
            if !n.is_finite() {
                anyhow::bail!("`{name}` must be finite");
            }
            Ok(Some(n))
        }
    }
}

fn parse_optional_f32_positive(
    node: roxmltree::Node<'_, '_>,
    name: &str,
) -> anyhow::Result<Option<f32>> {
    match parse_optional_f32(node, name)? {
        None => Ok(None),
        Some(val) => {
            if val <= 0.0 {
                anyhow::bail!("`{name}` must be positive");
            }
            Ok(Some(val))
        }
    }
}

fn required_attr<'a>(node: roxmltree::Node<'a, '_>, name: &str) -> anyhow::Result<&'a str> {
    node.attribute(name)
        .ok_or_else(|| anyhow::anyhow!("<{}> requires `{name}`", node.tag_name().name()))
}

fn required_non_empty_attr<'a>(
    node: roxmltree::Node<'a, '_>,
    name: &str,
) -> anyhow::Result<&'a str> {
    let value = required_attr(node, name)?;
    if value.is_empty() {
        anyhow::bail!("<{}> `{name}` must not be empty", node.tag_name().name());
    }
    Ok(value)
}

fn parse_optional_u32_attr(
    node: roxmltree::Node<'_, '_>,
    name: &str,
) -> anyhow::Result<Option<u32>> {
    match node.attribute(name) {
        None => Ok(None),
        Some(value) => {
            if value.is_empty() {
                anyhow::bail!("`{name}` must not be empty");
            }
            if !value.bytes().all(|b| b.is_ascii_digit()) {
                anyhow::bail!("`{name}` must be a positive integer (got `{value}`)");
            }
            if value.starts_with('0') && value.len() > 1 {
                anyhow::bail!("`{name}` must not have leading zeros");
            }
            let n: u32 = value
                .parse()
                .map_err(|e| anyhow::anyhow!("`{name}`: {e}"))?;
            if n == 0 {
                anyhow::bail!("`{name}` must be positive");
            }
            Ok(Some(n))
        }
    }
}

fn validate_no_element_children(node: roxmltree::Node<'_, '_>, tag: &str) -> anyhow::Result<()> {
    for child in node.children() {
        if child.is_element() {
            anyhow::bail!("<{tag}> cannot have child elements");
        }
    }
    Ok(())
}

fn parse_positive_i32(value: &str) -> anyhow::Result<i32> {
    if value.is_empty() {
        anyhow::bail!("value must not be empty");
    }
    if !value.bytes().all(|b| b.is_ascii_digit()) {
        anyhow::bail!("value must be a positive integer (got `{value}`)");
    }
    if value.starts_with('0') && value.len() > 1 {
        anyhow::bail!("value must not have leading zeros");
    }
    let n: i32 = value.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
    if n <= 0 {
        anyhow::bail!("value must be positive");
    }
    Ok(n)
}

fn parse_positive_i32_attr(
    node: roxmltree::Node<'_, '_>,
    name: &str,
    default: i32,
) -> anyhow::Result<i32> {
    match node.attribute(name) {
        None => Ok(default),
        Some(value) => parse_positive_i32(value).map_err(|e| anyhow::anyhow!("`{name}`: {e}")),
    }
}

fn ensure_allowed_attrs(node: roxmltree::Node<'_, '_>, allowed: &[&str]) -> anyhow::Result<()> {
    for attr in node.attributes() {
        let name = attr.name();
        if matches!(name, "className" | "parentId" | "style") {
            anyhow::bail!("attribute `{name}` is not allowed in markup");
        }
        if !allowed.contains(&name) {
            anyhow::bail!("unknown attribute `{name}` on <{}>", node.tag_name().name());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_raw_script_with_unescaped_js_and_removes_island() {
        let input = "<opencat><script>\r\nif (ctx.frame < 10 && ctx.fps > 0) {}\r\n</script><div id=\"root\" /></opencat>";
        let extracted = extract_raw_script(input).expect("script should extract");

        assert_eq!(
            extracted.script.as_deref(),
            Some("\r\nif (ctx.frame < 10 && ctx.fps > 0) {}\r\n")
        );
        assert_eq!(extracted.xml, "<opencat><div id=\"root\" /></opencat>");
    }

    #[test]
    fn extraction_uses_final_closing_script_tag() {
        let input = "<opencat><script>var s = \"</script>\";</script><div id=\"root\" /></opencat>";
        let extracted = extract_raw_script(input).expect("script should extract");

        assert_eq!(extracted.script.as_deref(), Some("var s = \"</script>\";"));
        assert!(extracted.xml.contains("<div id=\"root\" />"));
    }

    #[test]
    fn rejects_nested_script_island() {
        let err =
            extract_raw_script("<opencat><div id=\"root\"><script>x</script></div></opencat>")
                .expect_err("nested script should fail");

        assert!(err.to_string().contains("direct child of <opencat>"));
    }

    #[test]
    fn script_works_after_audio_sibling() {
        let extracted = extract_raw_script(
            "<opencat><audio id=\"bgm\" url=\"x.mp3\" /><script>ctx.doSomething();</script><div id=\"root\" /></opencat>",
        )
        .expect("script after audio should extract");
        assert_eq!(extracted.script.as_deref(), Some("ctx.doSomething();"));
    }

    #[test]
    fn rejects_script_nested_in_div() {
        let err = extract_raw_script(
            "<opencat><div id=\"root\"><div id=\"inner\"><script>x</script></div></div></opencat>",
        )
        .expect_err("script nested in div>div should fail");

        assert!(err.to_string().contains("direct child of <opencat>"));
    }

    #[test]
    fn rejects_script_attributes_and_self_closing_script() {
        assert!(
            extract_raw_script(
                "<opencat><script type=\"js\">x</script><div id=\"root\" /></opencat>"
            )
            .is_err()
        );
        assert!(extract_raw_script("<opencat><script /><div id=\"root\" /></opencat>").is_err());
    }

    #[test]
    fn parses_opencat_defaults() {
        let parsed = parse(r#"<opencat><div id="root" /></opencat>"#).expect("markup should parse");

        assert_eq!(parsed.width, 1920);
        assert_eq!(parsed.height, 1080);
        assert_eq!(parsed.fps, 30);
        assert_eq!(parsed.frames, 90);
        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn parses_explicit_positive_integer_envelope() {
        let parsed = parse(r#"<opencat width="640" height="360" fps="24" frames="120"><div id="root" /></opencat>"#)
            .expect("markup should parse");

        assert_eq!(
            (parsed.width, parsed.height, parsed.fps, parsed.frames),
            (640, 360, 24, 120)
        );
    }

    #[test]
    fn rejects_invalid_envelope_numbers() {
        for attr in [
            "width=\"0\"",
            "height=\"-1\"",
            "fps=\"30.0\"",
            "frames=\"90px\"",
        ] {
            let input = format!(r#"<opencat {attr}><div id="root" /></opencat>"#);
            assert!(parse(&input).is_err(), "{attr} should fail");
        }
    }

    #[test]
    fn rejects_unknown_opencat_attribute() {
        let err = parse(r#"<opencat foo="bar"><div id="root" /></opencat>"#)
            .expect_err("unknown root attribute should fail");

        assert!(
            err.to_string()
                .contains("unknown attribute `foo` on <opencat>")
        );
    }

    #[test]
    fn parses_nested_visual_nodes_and_xml_text_content() {
        let parsed = parse(
            r#"<opencat width="320" height="180" fps="30" frames="1">
  <div id="root" class="flex">
    <text id="title" class="text-[32px]">Open&amp;Cat<![CDATA[!]]></text>
    <image id="img" path="/tmp/a.png" />
    <video id="vid" url="https://example.test/a.mp4" />
    <icon id="icon" icon="play" />
    <path id="curve" d="M0 0 L10 10" />
  </div>
</opencat>"#,
        )
        .expect("markup should parse");

        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn accepts_direct_and_nested_audio() {
        let parsed = parse(
            r#"<opencat>
  <audio id="music" path="/tmp/music.wav" duration="30" />
  <div id="root">
    <audio id="scene-audio" url="https://example.test/a.wav" />
  </div>
</opencat>"#,
        )
        .expect("markup should parse");

        assert_eq!(parsed.audio_sources.len(), 2);
    }

    #[test]
    fn rejects_disallowed_attributes_and_bad_resources() {
        let cases = [
            (
                r#"<opencat><div id="root" className="x" /></opencat>"#,
                "className",
            ),
            (
                r#"<opencat><div id="root" parentId="x" /></opencat>"#,
                "parentId",
            ),
            (
                r#"<opencat><div id="root" style="color:red" /></opencat>"#,
                "style",
            ),
            (
                r#"<opencat><image id="img" /></opencat>"#,
                "requires one of",
            ),
            (
                r#"<opencat><image id="img" path="a" url="b" /></opencat>"#,
                "only one",
            ),
            (
                r#"<opencat><image id="img" path="a" queryCount="2" /></opencat>"#,
                "queryCount",
            ),
            (r#"<opencat><video id="v" /></opencat>"#, "requires one of"),
            (r#"<opencat><audio id="a" /></opencat>"#, "requires one of"),
            (r#"<opencat><caption id="c" /></opencat>"#, "path"),
            (r#"<opencat><path id="p" /></opencat>"#, "d"),
            (r#"<opencat><icon id="i" /></opencat>"#, "icon"),
        ];

        for (input, expected) in cases {
            let err = parse(input).expect_err(input);
            assert!(err.to_string().contains(expected), "{input}: {err}");
        }
    }

    #[test]
    fn parses_timeline_with_adjacent_transition() {
        let parsed = parse(
            r#"<opencat>
  <tl id="main">
    <div id="scene-a" duration="30" />
    <transition from="scene-a" to="scene-b" effect="fade" duration="10" timing="linear" />
    <div id="scene-b" duration="30" />
  </tl>
</opencat>"#,
        )
        .expect("timeline should parse");

        assert_eq!(parsed.root.style_ref().id, "main");
    }

    #[test]
    fn attaches_markup_script_to_visual_root_not_global_script_field() {
        let parsed = parse(
            r#"<opencat>
  <script>ctx.getNode('root').opacity(0.5);</script>
  <div id="root" />
</opencat>"#,
        )
        .expect("markup with script should parse");

        assert!(
            parsed.script.is_none(),
            "markup does not use ParsedComposition.script"
        );
        assert!(parsed.root.style_ref().script_driver.is_some());
    }

    #[test]
    fn markup_allows_canvas_hidden_visual_children() {
        let parsed = parse(
            r#"<opencat>
  <div id="root">
    <canvas id="stage">
      <text id="hidden">Hidden</text>
    </canvas>
  </div>
</opencat>"#,
        )
        .expect("canvas hidden children should parse");

        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn markup_rejects_audio_inside_canvas_hidden_subtree() {
        let err = parse(
            r#"<opencat><canvas id="stage"><audio id="bad" path="/tmp/a.wav" /></canvas></opencat>"#,
        )
        .expect_err("audio inside canvas should fail");

        assert!(
            err.to_string()
                .contains("audio is not allowed inside canvas")
        );
    }

    #[test]
    fn rejects_bad_markup_transitions() {
        let cases = [
            (
                r#"<opencat><div id="root"><transition from="a" to="b" effect="fade" duration="1" /></div></opencat>"#,
                "transition must be a direct child of <tl>",
            ),
            (
                r#"<opencat><tl id="tl"><div id="a" duration="1" /><div id="b" duration="1" /></tl></opencat>"#,
                "missing transition",
            ),
            (
                r#"<opencat><tl id="tl"><div id="a" /><transition from="a" to="b" effect="fade" duration="1" /><div id="b" duration="1" /></tl></opencat>"#,
                "missing a duration",
            ),
            (
                r#"<opencat><tl id="tl"><div id="a" duration="1" /><transition from="a" to="a" effect="fade" duration="1" /><div id="b" duration="1" /></tl></opencat>"#,
                "distinct",
            ),
            (
                r#"<opencat><tl id="tl"><div id="a" duration="1" /><transition from="a" to="b" effect="fade" duration="0" /><div id="b" duration="1" /></tl></opencat>"#,
                "duration",
            ),
        ];

        for (input, expected) in cases {
            let err = parse(input).expect_err(input);
            assert!(err.to_string().contains(expected), "{input}: {err}");
        }
    }

    #[test]
    fn public_markup_parse_entrypoint_works() {
        let parsed = crate::parse::markup::parse(r#"<opencat><div id="root" /></opencat>"#)
            .expect("markup should parse");

        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn rejects_strict_numeric_violations() {
        let cases = [
            r#"<opencat width=" 1"><div id="root" /></opencat>"#,
            r#"<opencat width="+1"><div id="root" /></opencat>"#,
            r#"<opencat width="１２"><div id="root" /></opencat>"#,
            r#"<opencat><div id="root" duration="1.5" /></opencat>"#,
            r#"<opencat><image id="img" query="cat" queryCount="0" /></opencat>"#,
            r#"<opencat><transition from="a" to="b" effect="fade" duration="999999999999999999999" /></opencat>"#,
        ];

        for input in cases {
            assert!(parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn validates_xml_text_and_processing_instructions() {
        assert!(parse(r#"<opencat><div id="root">bad</div></opencat>"#).is_err());
        assert!(parse(r#"<opencat><?bad test?><div id="root" /></opencat>"#).is_err());
        assert!(parse(r#"<opencat><text id="t"><span /></text></opencat>"#).is_err());

        let parsed = parse(
            r#"<opencat><text id="t"> ordinary <![CDATA[cdata]]> &amp; entity </text></opencat>"#,
        )
        .expect("text content should parse");
        assert_eq!(parsed.root.style_ref().id, "t");
    }

    #[test]
    fn rejects_empty_required_markup_attributes() {
        for input in [
            r#"<opencat><image id="img" path="" /></opencat>"#,
            r#"<opencat><video id="vid" url="" /></opencat>"#,
            r#"<opencat><audio id="aud" path="" /></opencat>"#,
            r#"<opencat><caption id="cap" path="" /></opencat>"#,
            r#"<opencat><path id="path" d="" /></opencat>"#,
            r#"<opencat><icon id="icon" icon="" /></opencat>"#,
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }
}
