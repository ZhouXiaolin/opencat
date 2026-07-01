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

use std::{collections::HashMap, path::PathBuf};

use crate::parse::document::{
    BuildOptions, CanvasChildrenMode, ParsedAudioElement, ParsedComposition, ParsedDocumentParts,
    ParsedElement, ParsedElementKind, ParsedTransition, build_parsed_document,
};
use crate::parse::primitives::{AudioSource, ImageSource, OpenverseQuery, VideoSource};
use crate::resource::fonts::{FontFaceDecl, FontManifest, FontRole, FontSource};
use crate::resource::types::VideoFrameTiming;

pub fn parse(input: &str) -> anyhow::Result<ParsedComposition> {
    parse_with_base_dir(input, None)
}

/// Parse markup into document parts (no scene tree yet).
pub fn parse_parts_with_base_dir(
    input: &str,
    base_dir: Option<&std::path::Path>,
) -> anyhow::Result<ParsedDocumentParts> {
    let extracted = extract_raw_script(input)?;
    let expanded_xml = expand_markup_templates(&extracted.xml)?;
    let doc = roxmltree::Document::parse(&expanded_xml)?;
    let root = doc.root_element();
    if root.tag_name().name() != "opencat" {
        anyhow::bail!("markup document root must be <opencat>");
    }
    ensure_allowed_attrs(root, &["width", "height", "fps", "duration"])?;
    let mut parts = ParsedDocumentParts {
        width: parse_positive_i32_attr(root, "width", 1920)?,
        height: parse_positive_i32_attr(root, "height", 1080)?,
        fps: parse_positive_i32_attr(root, "fps", 30)?,
        duration: parse_positive_f64_attr(root, "duration", 3.0)?,
        markup_root_script: extracted.script,
        ..Default::default()
    };
    parse_opencat_children(root, &doc, base_dir, &mut parts)?;
    Ok(parts)
}

pub fn parse_with_base_dir(
    input: &str,
    base_dir: Option<&std::path::Path>,
) -> anyhow::Result<ParsedComposition> {
    let parts = parse_parts_with_base_dir(input, base_dir)?;
    build_parsed_document(
        parts,
        BuildOptions {
            canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree,
        },
        None,
    )
}

const DIV_ATTRS: &[&str] = &["id", "class", "duration"];
const TEXT_ATTRS: &[&str] = &["id", "class", "duration", "data-text"];
const PSEUDO_TEXT_ATTRS: &[&str] = &["id", "class", "duration", "content"];
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
const FONTS_ATTRS: &[&str] = &["default"];
const FONT_ATTRS: &[&str] = &["id", "family", "path", "url", "role"];
const AUDIO_ATTRS: &[&str] = &["id", "duration", "path", "url", "attach"];
const LOTTIE_ATTRS: &[&str] = &[
    "id",
    "class",
    "duration",
    "path",
    "url",
    "data-start",
    "data-duration",
    "data-media-start",
    "loop",
];
const VIDEO_ATTRS: &[&str] = &[
    "id",
    "class",
    "duration",
    "path",
    "url",
    "data-start",
    "data-duration",
    "data-media-start",
    "loop",
];
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

const BUILTIN_TAGS: &[&str] = &[
    "opencat",
    "template",
    "slot",
    "script",
    "before",
    "after",
    "soundtrack",
    "audio",
    "fonts",
    "font",
    "div",
    "text",
    "canvas",
    "image",
    "lottie",
    "video",
    "icon",
    "path",
    "caption",
    "tl",
    "transition",
];

pub(crate) fn expand_markup_templates(input: &str) -> anyhow::Result<String> {
    let doc = roxmltree::Document::parse(input)?;
    let root = doc.root_element();
    if root.tag_name().name() != "opencat" {
        return Ok(input.to_string());
    }

    let templates = collect_templates(root)?;
    if templates.is_empty() {
        return Ok(input.to_string());
    }

    let mut out = String::new();
    write_element_open(&mut out, root, "opencat", None);
    for child in root.children() {
        if child.is_element() && child.tag_name().name() == "template" {
            continue;
        }
        serialize_template_node(
            &mut out,
            child,
            &templates,
            &HashMap::new(),
            &HashMap::new(),
            false,
            &mut Vec::new(),
        )?;
    }
    out.push_str("</opencat>");
    Ok(out)
}

fn collect_templates<'a, 'input>(
    root: roxmltree::Node<'a, 'input>,
) -> anyhow::Result<HashMap<String, roxmltree::Node<'a, 'input>>> {
    let mut templates = HashMap::new();
    for child in root.children().filter(|child| child.is_element()) {
        if child.tag_name().name() != "template" {
            continue;
        }
        for attr in child.attributes() {
            if attr.name() != "name" {
                anyhow::bail!("<template> only accepts `name`");
            }
        }
        let name = required_non_empty_attr(child, "name")?;
        if BUILTIN_TAGS.contains(&name) {
            anyhow::bail!("<template name=\"{name}\"> conflicts with a built-in tag");
        }
        if templates.insert(name.to_string(), child).is_some() {
            anyhow::bail!("duplicate template `{name}`");
        }
    }
    Ok(templates)
}

fn serialize_template_node<'a, 'input>(
    out: &mut String,
    node: roxmltree::Node<'a, 'input>,
    templates: &HashMap<String, roxmltree::Node<'a, 'input>>,
    params: &HashMap<String, String>,
    slots: &HashMap<String, Vec<roxmltree::Node<'a, 'input>>>,
    allow_slot: bool,
    stack: &mut Vec<String>,
) -> anyhow::Result<()> {
    match node.node_type() {
        roxmltree::NodeType::Element => {
            let tag = node.tag_name().name();
            if tag == "template" {
                anyhow::bail!("<template> must be a direct child of <opencat>");
            }
            if tag == "slot" {
                if !allow_slot {
                    anyhow::bail!("<slot> can only appear inside a <template> body");
                }
                let name = required_non_empty_attr(node, "name")?;
                if let Some(children) = slots.get(name) {
                    for child in children {
                        serialize_template_node(
                            out, *child, templates, params, slots, false, stack,
                        )?;
                    }
                }
                return Ok(());
            }
            if let Some(template) = templates.get(tag) {
                expand_template_call(out, node, *template, templates, params, stack)?;
                return Ok(());
            }

            write_element_open(out, node, tag, Some(params));
            for child in node.children() {
                serialize_template_node(out, child, templates, params, slots, allow_slot, stack)?;
            }
            write_element_close(out, tag);
        }
        roxmltree::NodeType::Text => {
            out.push_str(&escape_text(&substitute_template_vars(
                node.text().unwrap_or(""),
                params,
            )));
        }
        roxmltree::NodeType::Comment => {}
        _ => anyhow::bail!("processing instructions are not allowed in templates"),
    }
    Ok(())
}

fn expand_template_call<'a, 'input>(
    out: &mut String,
    call: roxmltree::Node<'a, 'input>,
    template: roxmltree::Node<'a, 'input>,
    templates: &HashMap<String, roxmltree::Node<'a, 'input>>,
    parent_params: &HashMap<String, String>,
    stack: &mut Vec<String>,
) -> anyhow::Result<()> {
    let name = call.tag_name().name();
    if stack.iter().any(|item| item == name) {
        anyhow::bail!("recursive template call `{name}` is not allowed");
    }

    let params = call
        .attributes()
        .map(|attr| {
            (
                attr.name().to_string(),
                substitute_template_vars(attr.value(), parent_params),
            )
        })
        .collect::<HashMap<_, _>>();
    let slots = collect_slot_values(call)?;

    stack.push(name.to_string());
    for child in template.children() {
        serialize_template_node(out, child, templates, &params, &slots, true, stack)?;
    }
    stack.pop();

    Ok(())
}

fn collect_slot_values<'a, 'input>(
    call: roxmltree::Node<'a, 'input>,
) -> anyhow::Result<HashMap<String, Vec<roxmltree::Node<'a, 'input>>>> {
    let mut slots = HashMap::new();
    for child in call.children() {
        match child.node_type() {
            roxmltree::NodeType::Element => {
                if child.tag_name().name() != "slot" {
                    anyhow::bail!(
                        "<{}> children must be <slot name=\"...\"> blocks",
                        call.tag_name().name()
                    );
                }
                for attr in child.attributes() {
                    if attr.name() != "name" {
                        anyhow::bail!("<slot> only accepts `name`");
                    }
                }
                let name = required_non_empty_attr(child, "name")?.to_string();
                let children = child.children().collect::<Vec<_>>();
                if slots.insert(name.clone(), children).is_some() {
                    anyhow::bail!("duplicate slot `{name}` in <{}>", call.tag_name().name());
                }
            }
            roxmltree::NodeType::Text => {
                if !child.text().unwrap_or("").trim().is_empty() {
                    anyhow::bail!(
                        "non-whitespace text is not allowed directly inside <{}>",
                        call.tag_name().name()
                    );
                }
            }
            roxmltree::NodeType::Comment => {}
            _ => anyhow::bail!("processing instructions are not allowed inside template calls"),
        }
    }
    Ok(slots)
}

fn write_element_open(
    out: &mut String,
    node: roxmltree::Node<'_, '_>,
    tag: &str,
    params: Option<&HashMap<String, String>>,
) {
    out.push('<');
    out.push_str(tag);
    for attr in node.attributes() {
        out.push(' ');
        out.push_str(attr.name());
        out.push_str("=\"");
        let value = params
            .map(|params| substitute_template_vars(attr.value(), params))
            .unwrap_or_else(|| attr.value().to_string());
        out.push_str(&escape_attr(&value));
        out.push('"');
    }
    out.push('>');
}

fn write_element_close(out: &mut String, tag: &str) {
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn substitute_template_vars(input: &str, params: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '$' {
            out.push(ch);
            continue;
        }
        let mut name = String::new();
        while let Some((_, next)) = chars.peek().copied() {
            if next.is_ascii_alphanumeric() || next == '_' {
                name.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if name.is_empty() {
            out.push('$');
        } else if let Some(value) = params.get(&name) {
            out.push_str(value);
        }
    }
    out
}

fn escape_attr(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

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
    doc: &roxmltree::Document<'_>,
    base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    let mut visual_root: Option<String> = None;

    for child in root.children() {
        match child.node_type() {
            roxmltree::NodeType::Element => {
                let tag = child.tag_name().name();
                match tag {
                    "soundtrack" => {
                        parse_soundtrack(child, base_dir, parts)?;
                    }
                    "fonts" => {
                        if !parts.font_manifest.faces.is_empty() {
                            anyhow::bail!("multiple <fonts> blocks are not allowed");
                        }
                        parse_fonts(child, base_dir, &mut parts.font_manifest)?;
                    }
                    "div" | "text" | "canvas" | "image" | "lottie" | "video" | "icon" | "path"
                    | "caption" | "tl" => {
                        let id = required_attr(child, "id")?;
                        if visual_root.is_some() {
                            anyhow::bail!("multiple visual root elements found");
                        }
                        visual_root = Some(id.to_string());
                        parse_visual_node(child, doc, None, base_dir, parts, ParentContext::Root)?;
                    }
                    "audio" => {
                        anyhow::bail!("<audio> must be inside <soundtrack>");
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
    doc: &roxmltree::Document<'_>,
    parent_id: Option<&str>,
    base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
    _parent_context: ParentContext,
) -> anyhow::Result<()> {
    let tag = node.tag_name().name();
    let id = required_attr(node, "id")?;
    let mut style = crate::style::NodeStyle::default();
    if let Some(class) = node.attribute("class") {
        let line_number = doc.text_pos_at(node.range().start).row as usize;
        style =
            crate::parse::jsonl::tailwind::parse_class_name_with_context(class, id, line_number);
    }
    let duration = parse_optional_f64_positive(node, "duration")?;

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
                            "div" | "text" | "canvas" | "image" | "lottie" | "video" | "icon"
                            | "path" | "caption" | "tl" => {
                                parse_visual_node(
                                    child,
                                    doc,
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
            let mut before_nodes = Vec::new();
            let mut after_nodes = Vec::new();
            for child in node.children() {
                match child.node_type() {
                    roxmltree::NodeType::Text => {
                        content.push_str(child.text().unwrap_or(""));
                    }
                    roxmltree::NodeType::Element => {
                        let child_tag = child.tag_name().name();
                        match child_tag {
                            "before" => before_nodes.push(child),
                            "after" => after_nodes.push(child),
                            other => anyhow::bail!(
                                "<text> cannot contain child element <{other}>; only <before> and <after> are allowed"
                            ),
                        }
                    }
                    roxmltree::NodeType::Comment => {}
                    _ => {}
                }
            }
            let parent_id = parent_id.map(|s| s.to_string());
            if before_nodes.is_empty() && after_nodes.is_empty() {
                let content = decode_text_escapes(&content);
                parts.elements.push(ParsedElement {
                    id: id.to_string(),
                    parent_id,
                    duration,
                    style,
                    kind: ParsedElementKind::Text { content },
                });
            } else {
                if before_nodes.len() > 1 {
                    anyhow::bail!("<text id=\"{id}\"> can contain at most one <before>");
                }
                if after_nodes.len() > 1 {
                    anyhow::bail!("<text id=\"{id}\"> can contain at most one <after>");
                }
                let content = decode_text_escapes(content.trim());
                parts.elements.push(ParsedElement {
                    id: id.to_string(),
                    parent_id,
                    duration,
                    style: style.clone(),
                    kind: ParsedElementKind::Div,
                });

                for before in before_nodes {
                    parse_pseudo_text_node(
                        before,
                        doc,
                        "before",
                        id,
                        &content,
                        node.attribute("data-text"),
                        parts,
                    )?;
                }

                let main_id = generated_text_content_id(id);
                let main_style = crate::style::NodeStyle {
                    text_shadows: style.text_shadows.clone(),
                    drop_shadow: style.drop_shadow.clone(),
                    ..Default::default()
                };
                parts.elements.push(ParsedElement {
                    id: main_id,
                    parent_id: Some(id.to_string()),
                    duration: None,
                    style: main_style,
                    kind: ParsedElementKind::Text {
                        content: content.clone(),
                    },
                });

                for after in after_nodes {
                    parse_pseudo_text_node(
                        after,
                        doc,
                        "after",
                        id,
                        &content,
                        node.attribute("data-text"),
                        parts,
                    )?;
                }
            }
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
                            "div" | "text" | "canvas" | "image" | "lottie" | "video" | "icon"
                            | "path" | "caption" | "tl" => {
                                parse_visual_node(
                                    child,
                                    doc,
                                    Some(&id),
                                    base_dir,
                                    parts,
                                    ParentContext::Canvas,
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
        "image" => {
            ensure_allowed_attrs(node, IMAGE_ATTRS)?;
            let source = parse_image_source(node, base_dir)?;
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
        "lottie" => {
            ensure_allowed_attrs(node, LOTTIE_ATTRS)?;
            let source = parse_lottie_source(node, base_dir)?;
            let timing = parse_video_timing(node)?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Lottie { source, timing },
            });
            validate_no_element_children(node, "lottie")?;
        }
        "video" => {
            ensure_allowed_attrs(node, VIDEO_ATTRS)?;
            let source = parse_video_source(node, base_dir)?;
            let timing = parse_video_timing(node)?;
            let parent_id = parent_id.map(|s| s.to_string());
            parts.elements.push(ParsedElement {
                id: id.to_string(),
                parent_id,
                duration,
                style,
                kind: ParsedElementKind::Video { source, timing },
            });
            for child in node.children() {
                match child.node_type() {
                    roxmltree::NodeType::Element => {
                        let child_tag = child.tag_name().name();
                        match child_tag {
                            "div" | "text" | "canvas" | "image" | "lottie" | "video" | "icon"
                            | "path" | "caption" | "tl" => {
                                parse_visual_node(
                                    child,
                                    doc,
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
            let path = resolve_local_path(path_str, base_dir);
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
                            "div" | "text" | "canvas" | "image" | "lottie" | "video" | "icon"
                            | "path" | "caption" | "tl" => {
                                parse_visual_node(
                                    child,
                                    doc,
                                    Some(&id),
                                    base_dir,
                                    parts,
                                    ParentContext::Timeline,
                                )?;
                            }
                            "transition" => {
                                parse_transition_node(child, &id, parts)?;
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
        _ => anyhow::bail!("unknown element <{tag}>"),
    }

    Ok(())
}

fn generated_text_content_id(id: &str) -> String {
    format!("__opencat_{id}_text")
}

fn parse_pseudo_text_node(
    node: roxmltree::Node<'_, '_>,
    doc: &roxmltree::Document<'_>,
    tag: &str,
    parent_id: &str,
    host_content: &str,
    data_text: Option<&str>,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    ensure_allowed_attrs(node, PSEUDO_TEXT_ATTRS)?;
    validate_no_element_children(node, tag)?;
    let id = required_attr(node, "id")?;
    let mut style = crate::style::NodeStyle::default();
    if let Some(class) = node.attribute("class") {
        let line_number = doc.text_pos_at(node.range().start).row as usize;
        style =
            crate::parse::jsonl::tailwind::parse_class_name_with_context(class, id, line_number);
    }
    if style.position.is_none() {
        style.position = Some(crate::style::Position::Absolute);
    }
    if style.inset_top.is_none() {
        style.inset_top = Some(crate::style::LengthPercentageAuto::length(0.0));
    }
    if style.inset_left.is_none() {
        style.inset_left = Some(crate::style::LengthPercentageAuto::length(0.0));
    }
    if style.width.is_none() && style.width_percent.is_none() && !style.width_full {
        style.width_full = true;
    }
    if style.height.is_none() && style.height_percent.is_none() && !style.height_full {
        style.height_full = true;
    }
    let duration = parse_optional_f64_positive(node, "duration")?;
    let content = match node.attribute("content").unwrap_or("self").trim() {
        "self" => host_content.to_string(),
        "attr(data-text)" => data_text.unwrap_or(host_content).to_string(),
        raw => decode_text_escapes(raw),
    };
    parts.elements.push(ParsedElement {
        id: id.to_string(),
        parent_id: Some(parent_id.to_string()),
        duration,
        style,
        kind: ParsedElementKind::Text { content },
    });
    Ok(())
}

fn parse_fonts(
    node: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
    manifest: &mut FontManifest,
) -> anyhow::Result<()> {
    ensure_allowed_attrs(node, FONTS_ATTRS)?;
    if let Some(default_id) = node.attribute("default") {
        if default_id.is_empty() {
            anyhow::bail!("<fonts default=\"\"> must be non-empty");
        }
        manifest.default_face_id = Some(default_id.to_string());
    }

    for child in node.children() {
        match child.node_type() {
            roxmltree::NodeType::Element => {
                if child.tag_name().name() != "font" {
                    anyhow::bail!(
                        "unknown element <{}> inside <fonts>",
                        child.tag_name().name()
                    );
                }
                ensure_allowed_attrs(child, FONT_ATTRS)?;
                let id = required_non_empty_attr(child, "id")?;
                if manifest.face_by_id(id).is_some() {
                    anyhow::bail!("duplicate font id `{id}`");
                }
                let family = child
                    .attribute("family")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                let role = child.attribute("role").map(parse_font_role).transpose()?;
                let source = parse_font_source(child, base_dir)?;
                manifest.faces.push(FontFaceDecl {
                    id: id.to_string(),
                    family,
                    source,
                    role,
                });
            }
            roxmltree::NodeType::Comment => {}
            roxmltree::NodeType::Text => {
                if !child.text().unwrap_or("").trim().is_empty() {
                    anyhow::bail!("non-whitespace text is not allowed inside <fonts>");
                }
            }
            _ => anyhow::bail!("processing instructions not allowed inside <fonts>"),
        }
    }

    if let Some(default_id) = manifest.default_face_id.as_deref() {
        if manifest.face_by_id(default_id).is_none() {
            anyhow::bail!("<fonts default=\"{default_id}\"> references unknown font id");
        }
    }

    Ok(())
}

fn parse_font_role(value: &str) -> anyhow::Result<FontRole> {
    match value {
        "sans" => Ok(FontRole::Sans),
        "emoji" => Ok(FontRole::Emoji),
        "mono" => Ok(FontRole::Mono),
        other => anyhow::bail!("unknown font role `{other}`; expected sans, emoji, or mono"),
    }
}

fn parse_font_source(
    node: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
) -> anyhow::Result<FontSource> {
    let path = node.attribute("path");
    let url = node.attribute("url");
    match (path, url) {
        (Some(p), None) => {
            if p.is_empty() {
                anyhow::bail!("<font> path must be non-empty");
            }
            let resolved = crate::resource::fonts::resolve_font_source_path(p, base_dir)?;
            Ok(FontSource::Path(resolved))
        }
        (None, Some(u)) => {
            if u.is_empty() {
                anyhow::bail!("<font> url must be non-empty");
            }
            Ok(FontSource::Url(u.to_string()))
        }
        (Some(_), Some(_)) => anyhow::bail!("<font> accepts only one of: path, url"),
        (None, None) => anyhow::bail!("<font> requires one of: path, url"),
    }
}

fn parse_soundtrack(
    node: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    for child in node.children() {
        match child.node_type() {
            roxmltree::NodeType::Element => {
                if child.tag_name().name() == "audio" {
                    parse_audio_element_in_soundtrack(child, base_dir, parts)?;
                } else {
                    anyhow::bail!(
                        "unknown element <{}> inside <soundtrack>",
                        child.tag_name().name()
                    );
                }
            }
            roxmltree::NodeType::Comment => {}
            roxmltree::NodeType::Text => {
                if !child.text().unwrap_or("").trim().is_empty() {
                    anyhow::bail!("non-whitespace text is not allowed inside <soundtrack>");
                }
            }
            _ => anyhow::bail!("processing instructions not allowed inside <soundtrack>"),
        }
    }
    Ok(())
}

fn parse_audio_element_in_soundtrack(
    node: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    let id = required_non_empty_attr(node, "id")?;
    let attach = required_non_empty_attr(node, "attach")?;
    let duration = parse_optional_f64_positive(node, "duration")?;

    let source = match (node.attribute("path"), node.attribute("url")) {
        (Some(p), None) => {
            if p.is_empty() {
                anyhow::bail!("<audio> `path` must not be empty");
            }
            AudioSource::Path(resolve_local_path(p, base_dir))
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

    ensure_allowed_attrs(node, AUDIO_ATTRS)?;

    parts.audio_elements.push(ParsedAudioElement {
        id: id.to_string(),
        attach: attach.to_string(),
        duration,
        source,
    });

    validate_no_element_children(node, "audio")?;
    Ok(())
}

fn resolve_local_path(raw: &str, base_dir: Option<&std::path::Path>) -> PathBuf {
    let path = PathBuf::from(raw);
    let Some(base_dir) = base_dir else {
        return path;
    };
    if path.is_absolute() {
        return path;
    }

    let joined = base_dir.join(path);
    if joined.is_absolute() {
        joined
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&joined))
            .unwrap_or(joined)
    }
}

fn parse_lottie_source(
    node: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
) -> anyhow::Result<crate::parse::primitives::LottieSource> {
    let path = node.attribute("path");
    let url = node.attribute("url");
    let count = [path.is_some(), url.is_some()]
        .iter()
        .filter(|&&b| b)
        .count();
    if count == 0 {
        anyhow::bail!("<lottie> requires one of: path, url");
    }
    if count > 1 {
        anyhow::bail!("<lottie> requires only one of: path, url");
    }
    if let Some(p) = path {
        if p.is_empty() {
            anyhow::bail!("<lottie> `path` must not be empty");
        }
        return Ok(crate::parse::primitives::LottieSource::Path(
            resolve_local_path(p, base_dir),
        ));
    }
    if let Some(u) = url {
        if u.is_empty() {
            anyhow::bail!("<lottie> `url` must not be empty");
        }
        return Ok(crate::parse::primitives::LottieSource::Url(u.to_string()));
    }
    Ok(crate::parse::primitives::LottieSource::Unset)
}

fn parse_image_source(
    node: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
) -> anyhow::Result<ImageSource> {
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
        return Ok(ImageSource::Path(resolve_local_path(p, base_dir)));
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

fn decode_text_escapes(content: &str) -> String {
    content.replace("\\n", "\n")
}

fn parse_video_source(
    node: roxmltree::Node<'_, '_>,
    base_dir: Option<&std::path::Path>,
) -> anyhow::Result<VideoSource> {
    let path = node.attribute("path");
    let url = node.attribute("url");
    let count = [path.is_some(), url.is_some()]
        .iter()
        .filter(|&&b| b)
        .count();

    if count == 0 {
        anyhow::bail!("<video> requires one of: path, url");
    }
    if count > 1 {
        anyhow::bail!("<video> requires only one of: path, url");
    }

    match (path, url) {
        (Some(p), None) => {
            if p.is_empty() {
                anyhow::bail!("<video> `path` must not be empty");
            }
            Ok(VideoSource::Path(resolve_local_path(p, base_dir)))
        }
        (None, Some(u)) => {
            if u.is_empty() {
                anyhow::bail!("<video> `url` must not be empty");
            }
            Ok(VideoSource::Url(u.to_string()))
        }
        _ => unreachable!("source count validated above"),
    }
}

fn parse_video_timing(node: roxmltree::Node<'_, '_>) -> anyhow::Result<VideoFrameTiming> {
    Ok(VideoFrameTiming {
        timeline_start_secs: parse_optional_f64_non_negative(node, "data-start")?.unwrap_or(0.0),
        timeline_duration_secs: parse_optional_f64_positive(node, "data-duration")?,
        media_start_secs: parse_optional_f64_non_negative(node, "data-media-start")?.unwrap_or(0.0),
        playback_rate: 1.0,
        looping: parse_optional_bool(node, "loop")?.unwrap_or(false),
    })
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
    let duration = parse_required_f64_positive(node, "duration")?;

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

fn parse_optional_f64(node: roxmltree::Node<'_, '_>, name: &str) -> anyhow::Result<Option<f64>> {
    match node.attribute(name) {
        None => Ok(None),
        Some(val) => {
            if val.is_empty() {
                anyhow::bail!("`{name}` must not be empty");
            }
            let n: f64 = val.parse().map_err(|e| anyhow::anyhow!("`{name}`: {e}"))?;
            if !n.is_finite() {
                anyhow::bail!("`{name}` must be finite");
            }
            Ok(Some(n))
        }
    }
}

fn parse_optional_f64_non_negative(
    node: roxmltree::Node<'_, '_>,
    name: &str,
) -> anyhow::Result<Option<f64>> {
    match parse_optional_f64(node, name)? {
        None => Ok(None),
        Some(val) if val < 0.0 => {
            anyhow::bail!("`{name}` must be non-negative")
        }
        Some(val) => Ok(Some(val)),
    }
}

fn parse_optional_f64_positive(
    node: roxmltree::Node<'_, '_>,
    name: &str,
) -> anyhow::Result<Option<f64>> {
    match parse_optional_f64(node, name)? {
        None => Ok(None),
        Some(val) if val <= 0.0 => {
            anyhow::bail!("`{name}` must be positive")
        }
        Some(val) => Ok(Some(val)),
    }
}

fn parse_optional_bool(node: roxmltree::Node<'_, '_>, name: &str) -> anyhow::Result<Option<bool>> {
    match node.attribute(name) {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some("1") => Ok(Some(true)),
        Some("0") => Ok(Some(false)),
        Some(value) => anyhow::bail!("`{name}` must be true or false (got `{value}`)"),
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

fn parse_required_f64_positive(node: roxmltree::Node<'_, '_>, name: &str) -> anyhow::Result<f64> {
    let value = required_non_empty_attr(node, name)?;
    let n: f64 = value
        .parse()
        .map_err(|e| anyhow::anyhow!("`{name}`: {e}"))?;
    if !n.is_finite() {
        anyhow::bail!("`{name}` must be finite");
    }
    if n <= 0.0 {
        anyhow::bail!("`{name}` must be positive");
    }
    Ok(n)
}

fn parse_positive_f64_attr(
    node: roxmltree::Node<'_, '_>,
    name: &str,
    default: f64,
) -> anyhow::Result<f64> {
    Ok(parse_optional_f64_positive(node, name)?.unwrap_or(default))
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

/// Attribute names the parser hard-rejects (not merely ignored). Shared with the lint
/// pass so the two views of "forbidden" never drift.
pub(crate) const FORBIDDEN_MARKUP_ATTRS: &[&str] = &["className", "parentId", "style"];

/// The set of attributes each markup tag accepts. Returns `None` for tags the parser
/// does not enforce a whitelist on (e.g. `<soundtrack>`), matching the lenient
/// "unknown attributes are silently ignored" behavior in [`ensure_allowed_attrs`].
pub(crate) fn allowed_attributes(tag: &str) -> Option<&'static [&'static str]> {
    match tag {
        "opencat" => Some(&["width", "height", "fps", "duration"]),
        "div" => Some(DIV_ATTRS),
        "text" => Some(TEXT_ATTRS),
        "before" | "after" => Some(PSEUDO_TEXT_ATTRS),
        "canvas" => Some(CANVAS_ATTRS),
        "image" => Some(IMAGE_ATTRS),
        "lottie" => Some(LOTTIE_ATTRS),
        "video" => Some(VIDEO_ATTRS),
        "icon" => Some(ICON_ATTRS),
        "path" => Some(PATH_ATTRS),
        "caption" => Some(CAPTION_ATTRS),
        "tl" => Some(TL_ATTRS),
        "audio" => Some(AUDIO_ATTRS),
        "fonts" => Some(FONTS_ATTRS),
        "font" => Some(FONT_ATTRS),
        "transition" => Some(TRANSITION_ATTRS),
        _ => None,
    }
}

fn ensure_allowed_attrs(node: roxmltree::Node<'_, '_>, allowed: &[&str]) -> anyhow::Result<()> {
    for attr in node.attributes() {
        let name = attr.name();
        if FORBIDDEN_MARKUP_ATTRS.contains(&name) {
            anyhow::bail!("attribute `{name}` is not allowed in markup");
        }
        // Unknown attributes are silently ignored — only known attributes are consumed.
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::node::NodeKind;
    use crate::resolve::{resolve::resolve_ui_tree, tree::ElementKind};
    use crate::test_support::{MockScriptHost, TestCatalog};

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
            "<opencat><soundtrack><audio id=\"bgm\" url=\"x.mp3\" attach=\"main-tl\" /></soundtrack><script>ctx.doSomething();</script><div id=\"root\" /></opencat>",
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
    fn parses_fonts_block() {
        let input = r#"<opencat width="320" height="180" fps="30" duration="0.03333333333333333">
  <fonts default="sans">
    <font id="sans" family="Noto Sans SC" url="https://example.com/NotoSansSC.otf" />
  </fonts>
  <div id="root" />
</opencat>"#;
        let parts = super::parse_parts_with_base_dir(input, None).expect("parts should parse");
        assert_eq!(parts.font_manifest.default_face_id.as_deref(), Some("sans"));
        assert_eq!(parts.font_manifest.faces.len(), 1);
        assert_eq!(parts.font_manifest.faces[0].id, "sans");
    }

    fn parses_opencat_defaults() {
        let parsed = parse(r#"<opencat><div id="root" /></opencat>"#).expect("markup should parse");

        assert_eq!(parsed.width, 1920);
        assert_eq!(parsed.height, 1080);
        assert_eq!(parsed.fps, 30);
        assert_eq!(parsed.duration, 3.0);
        assert_eq!(parsed.root.style_ref().id, "root");
    }

    #[test]
    fn parses_explicit_positive_integer_envelope() {
        let parsed = parse(r#"<opencat width="640" height="360" fps="24" duration="5"><div id="root" /></opencat>"#)
            .expect("markup should parse");

        assert_eq!(
            (parsed.width, parsed.height, parsed.fps, parsed.duration),
            (640, 360, 24, 5.0)
        );
    }

    #[test]
    fn rejects_invalid_envelope_numbers() {
        for attr in [
            "width=\"0\"",
            "height=\"-1\"",
            "fps=\"30.0\"",
            "duration=\"90px\"",
        ] {
            let input = format!(r#"<opencat {attr}><div id="root" /></opencat>"#);
            assert!(parse(&input).is_err(), "{attr} should fail");
        }
    }

    #[test]
    fn ignores_unknown_opencat_attribute() {
        let result = parse(r#"<opencat foo="bar"><div id="root" /></opencat>"#);
        assert!(
            result.is_ok(),
            "unknown attribute should be ignored, got {:?}",
            result.err()
        );
    }

    #[test]
    fn parses_nested_visual_nodes_and_xml_text_content() {
        let parsed = parse(
            r#"<opencat width="320" height="180" fps="30" duration="0.03333333333333333">
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
    fn expands_template_custom_elements_with_params_and_slots() {
        let parsed = parse(
            r#"<opencat width="320" height="180" fps="30" duration="1">
  <template name="deck-thumb">
    <div id="$id" class="flex $state">
      <text id="$id-num" class="$numTone">$num</text>
      <div id="$id-frame" class="$frameTone">
        <slot name="overlay" />
      </div>
    </div>
  </template>
  <div id="root">
    <deck-thumb id="thumb-1" state="opacity-80" num="1" numTone="text-white" frameTone="bg-white">
      <slot name="overlay">
        <text id="thumb-1-label">Active</text>
      </slot>
    </deck-thumb>
  </div>
</opencat>"#,
        )
        .expect("template should expand before parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Div(thumb) = root.children_ref()[0].kind() else {
            panic!("thumb should expand to div");
        };
        assert_eq!(thumb.style_ref().id, "thumb-1");
        assert_eq!(thumb.children_ref().len(), 2);

        let NodeKind::Text(num) = thumb.children_ref()[0].kind() else {
            panic!("num should be text");
        };
        assert_eq!(num.style_ref().id, "thumb-1-num");
        assert_eq!(num.content(), "1");

        let NodeKind::Div(frame) = thumb.children_ref()[1].kind() else {
            panic!("frame should be div");
        };
        let NodeKind::Text(label) = frame.children_ref()[0].kind() else {
            panic!("slot content should be inserted");
        };
        assert_eq!(label.style_ref().id, "thumb-1-label");
    }

    #[test]
    fn expands_nested_templates_with_parent_params() {
        let parsed = parse(
            r#"<opencat>
  <template name="label-row">
    <div id="$id-row">
      <text id="$id-label">$label</text>
    </div>
  </template>
  <template name="card-box">
    <div id="$id">
      <label-row id="$id-main" label="$label" />
    </div>
  </template>
  <card-box id="card" label="Nested" />
</opencat>"#,
        )
        .expect("nested templates should expand");

        let NodeKind::Div(card) = parsed.root.kind() else {
            panic!("root should be card div");
        };
        let NodeKind::Div(row) = card.children_ref()[0].kind() else {
            panic!("nested template should expand to row");
        };
        assert_eq!(row.style_ref().id, "card-main-row");
        let NodeKind::Text(label) = row.children_ref()[0].kind() else {
            panic!("row child should be text");
        };
        assert_eq!(label.style_ref().id, "card-main-label");
        assert_eq!(label.content(), "Nested");
    }

    #[test]
    fn rejects_bad_template_definitions_and_recursive_calls() {
        let bad_cases = [
            r#"<opencat><template name="div"><div id="x" /></template><div id="root" /></opencat>"#,
            r#"<opencat><template name="x"><x id="loop" /></template><x id="root" /></opencat>"#,
            r#"<opencat><template name="x" extra="bad"><div id="$id" /></template><x id="root" /></opencat>"#,
            r#"<opencat><template name="x"><div id="$id" /></template><div id="root"><slot name="bad" /></div></opencat>"#,
        ];

        for input in bad_cases {
            assert!(parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn parses_backslash_n_in_text_as_newline() {
        let parsed = parse(
            r#"<opencat width="320" height="180" fps="30" duration="0.03333333333333333">
  <div id="root">
    <text id="headline">Real-time\nrendering</text>
  </div>
</opencat>"#,
        )
        .expect("markup should parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Text(headline) = root.children_ref()[0].kind() else {
            panic!("child should be text");
        };
        assert_eq!(headline.content(), "Real-time\nrendering");
    }

    #[test]
    fn parses_text_pseudo_elements_with_attr_content_and_text_shadow() {
        let parsed = parse(
            r##"<opencat width="320" height="180" fps="30" duration="1">
  <div id="root">
    <text id="hero" data-text="Transform" class="relative [text-shadow:0_0_10px_rgba(0,255,136,0.3)]">
      Transform
      <before id="hero-before" content="attr(data-text)" class="left-[2px] [text-shadow:-1px_0_#ff00ff]" />
      <after id="hero-after" content="attr(data-text)" class="left-[-2px] [text-shadow:-1px_0_#00d4ff]" />
    </text>
  </div>
</opencat>"##,
        )
        .expect("pseudo text should parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Div(hero) = root.children_ref()[0].kind() else {
            panic!("text with pseudo elements should lower to a div wrapper");
        };
        assert_eq!(hero.style_ref().id, "hero");
        assert_eq!(hero.children_ref().len(), 3);

        let NodeKind::Text(before) = hero.children_ref()[0].kind() else {
            panic!("before should lower to text");
        };
        let NodeKind::Text(main) = hero.children_ref()[1].kind() else {
            panic!("main content should lower to generated text");
        };
        let NodeKind::Text(after) = hero.children_ref()[2].kind() else {
            panic!("after should lower to text");
        };

        assert_eq!(before.style_ref().id, "hero-before");
        assert_eq!(before.content(), "Transform");
        assert_eq!(
            before.style_ref().position,
            Some(crate::style::Position::Absolute)
        );
        assert!(before.style_ref().width_full);
        assert!(before.style_ref().height_full);
        assert_eq!(before.style_ref().text_shadows.len(), 1);
        assert_eq!(
            before.style_ref().text_shadows[0].color,
            crate::style::ColorToken::Custom(255, 0, 255, 255)
        );
        assert_eq!(before.style_ref().text_shadows[0].offset_x, -1.0);

        assert_eq!(main.style_ref().id, "__opencat_hero_text");
        assert_eq!(main.content(), "Transform");
        assert_eq!(main.style_ref().text_shadows.len(), 1);
        assert_eq!(
            main.style_ref().text_shadows[0].color,
            crate::style::ColorToken::Custom(0, 255, 136, 77)
        );
        assert!((main.style_ref().text_shadows[0].blur_sigma - (10.0 / 6.0)).abs() < 1e-6);

        assert_eq!(after.style_ref().id, "hero-after");
        assert_eq!(after.content(), "Transform");
        assert_eq!(after.style_ref().text_shadows.len(), 1);
        assert_eq!(
            after.style_ref().text_shadows[0].color,
            crate::style::ColorToken::Custom(0, 212, 255, 255)
        );
    }

    #[test]
    fn parses_video_timeline_and_media_timing_attrs() {
        let parsed = parse(
            r#"<opencat width="320" height="180" fps="30" duration="0.03333333333333333">
  <div id="root">
    <video id="vid" path="clip.mp4" data-start="3" data-duration="18" data-media-start="12" loop="true" />
  </div>
</opencat>"#,
        )
        .expect("markup should parse");

        let NodeKind::Div(root) = parsed.root.kind() else {
            panic!("root should be div");
        };
        let NodeKind::Video(video) = root.children_ref()[0].kind() else {
            panic!("child should be video");
        };

        assert_eq!(
            video.source(),
            &VideoSource::Path(PathBuf::from("clip.mp4"))
        );
        assert_eq!(
            video.timing(),
            VideoFrameTiming {
                timeline_start_secs: 3.0,
                timeline_duration_secs: Some(18.0),
                media_start_secs: 12.0,
                playback_rate: 1.0,
                looping: true,
            }
        );
    }

    #[test]
    fn video_allows_overlay_children() {
        let parsed = parse(
            r#"<opencat width="320" height="180" fps="30" duration="0.03333333333333333">
  <div id="root">
    <video id="vid" class="relative w-[160px] h-[90px]" path="clip.mp4">
      <div id="badge" class="absolute left-[8px] top-[8px]">
        <text id="label">TL</text>
      </div>
    </video>
  </div>
</opencat>"#,
        )
        .expect("video overlay children should parse");

        let frame_ctx = crate::FrameCtx {
            frame: 0,
            fps: parsed.fps as u32,
            width: parsed.width,
            height: parsed.height,
            frames: crate::frame_ctx::duration_secs_to_frames(parsed.duration, parsed.fps as u32),
        };
        let mut catalog = TestCatalog::new();
        let mut script_host = MockScriptHost::default();
        let resolved = resolve_ui_tree(
            &parsed.root,
            &frame_ctx,
            &mut catalog,
            None,
            &mut script_host,
        )
        .expect("tree should resolve");

        let video = &resolved.children[0];
        assert!(
            matches!(video.kind, ElementKind::Bitmap(_)),
            "video should resolve to bitmap"
        );
        assert_eq!(video.children.len(), 1);
        assert_eq!(video.children[0].style.id, "badge");
    }

    #[test]
    fn parses_soundtrack_audio() {
        let parsed = parse(
            r#"<opencat>
  <soundtrack>
    <audio id="music" path="/tmp/music.wav" attach="tl-1" duration="30" />
    <audio id="scene-audio" url="https://example.test/a.wav" attach="scene-1" />
  </soundtrack>
  <tl id="tl-1">
    <div id="scene-1" duration="1" />
    <transition from="scene-1" to="scene-2" effect="fade" duration="0.3333333333333333" />
    <div id="scene-2" duration="1" />
  </tl>
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
            (
                r#"<opencat><soundtrack><audio id="a" attach="x" /></soundtrack></opencat>"#,
                "requires one of",
            ),
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
    <div id="scene-a" duration="1" />
    <transition from="scene-a" to="scene-b" effect="fade" duration="0.3333333333333333" timing="linear" />
    <div id="scene-b" duration="1" />
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

        assert!(err.to_string().contains("unknown element <audio>"));
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
            r#"<opencat><div id="root" duration="0" /></opencat>"#,
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
            r#"<opencat><soundtrack><audio id="aud" path="" attach="main-tl" /></soundtrack></opencat>"#,
            r#"<opencat><caption id="cap" path="" /></opencat>"#,
            r#"<opencat><path id="path" d="" /></opencat>"#,
            r#"<opencat><icon id="icon" icon="" /></opencat>"#,
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }
}
