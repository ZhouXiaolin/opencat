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

    if let Some(last_open) = before_script.rfind('<') {
        let preceding = &before_script[last_open..];
        if !preceding.starts_with("<?")
            && !preceding.starts_with("<![CDATA[")
            && !preceding.starts_with("<!--")
            && !preceding.starts_with("<opencat")
        {
            anyhow::bail!("<script> must be a direct child of <opencat>");
        }
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

use crate::parse::document::{
    BuildOptions, CanvasChildrenMode, ParsedComposition, ParsedDocumentParts, ParsedElement,
    ParsedElementKind, build_tree_with_options,
};

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
    parse_visual_children(root, base_dir, &mut parts)?;

    let built_root = build_tree_with_options(
        &parts.elements,
        &parts.scripts_by_parent,
        parts.fps as u32,
        BuildOptions { canvas_children_mode: CanvasChildrenMode::HiddenPictureSubtree },
    )?;

    Ok(ParsedComposition {
        width: parts.width,
        height: parts.height,
        fps: parts.fps,
        frames: parts.frames,
        root: built_root,
        script: None,
        audio_sources: Vec::new(),
    })
}

fn parse_visual_children(
    root: roxmltree::Node<'_, '_>,
    _base_dir: Option<&std::path::Path>,
    parts: &mut ParsedDocumentParts,
) -> anyhow::Result<()> {
    for child in root.children() {
        match child.node_type() {
            roxmltree::NodeType::Element => {
                let tag = child.tag_name().name();
                match tag {
                    "div" => {
                        let id = child
                            .attribute("id")
                            .ok_or_else(|| anyhow::anyhow!("<div> requires `id` attribute"))?
                            .to_string();
                        let mut style = crate::style::NodeStyle::default();
                        if let Some(class) = child.attribute("class") {
                            style = crate::parse::jsonl::tailwind::parse_class_name(class);
                        }
                        parts.elements.push(ParsedElement {
                            id,
                            parent_id: None,
                            duration: None,
                            style,
                            kind: ParsedElementKind::Div,
                        });
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
            _ => anyhow::bail!("processing instructions and other node types are not allowed"),
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
        Some(value) => {
            parse_positive_i32(value).map_err(|e| anyhow::anyhow!("`{name}`: {e}"))
        }
    }
}

fn ensure_allowed_attrs(
    node: roxmltree::Node<'_, '_>,
    allowed: &[&str],
) -> anyhow::Result<()> {
    for attr in node.attributes() {
        let name = attr.name();
        if matches!(name, "className" | "parentId" | "style") {
            anyhow::bail!("attribute `{name}` is not allowed in markup");
        }
        if !allowed.contains(&name) {
            anyhow::bail!(
                "unknown attribute `{name}` on <{}>",
                node.tag_name().name()
            );
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
        let err = extract_raw_script("<opencat><div id=\"root\"><script>x</script></div></opencat>")
            .expect_err("nested script should fail");

        assert!(err.to_string().contains("direct child of <opencat>"));
    }

    #[test]
    fn rejects_script_attributes_and_self_closing_script() {
        assert!(extract_raw_script(
            "<opencat><script type=\"js\">x</script><div id=\"root\" /></opencat>"
        )
        .is_err());
        assert!(
            extract_raw_script("<opencat><script /><div id=\"root\" /></opencat>").is_err()
        );
    }

    #[test]
    fn parses_opencat_defaults() {
        let parsed = parse(r#"<opencat><div id="root" /></opencat>"#)
            .expect("markup should parse");

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

        assert_eq!((parsed.width, parsed.height, parsed.fps, parsed.frames), (640, 360, 24, 120));
    }

    #[test]
    fn rejects_invalid_envelope_numbers() {
        for attr in ["width=\"0\"", "height=\"-1\"", "fps=\"30.0\"", "frames=\"90px\""] {
            let input = format!(r#"<opencat {attr}><div id="root" /></opencat>"#);
            assert!(parse(&input).is_err(), "{attr} should fail");
        }
    }

    #[test]
    fn rejects_unknown_opencat_attribute() {
        let err = parse(r#"<opencat foo="bar"><div id="root" /></opencat>"#)
            .expect_err("unknown root attribute should fail");

        assert!(err.to_string().contains("unknown attribute `foo` on <opencat>"));
    }
}
