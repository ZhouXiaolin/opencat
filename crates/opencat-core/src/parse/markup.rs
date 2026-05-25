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
}
