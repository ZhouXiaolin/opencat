#![cfg(feature = "host-default")]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use opencat_core::jsonl::{JsonLine, ParsedComposition, parse};

pub fn parse_file(path: impl AsRef<Path>) -> Result<ParsedComposition> {
    let path = path.as_ref();
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read jsonl file: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    parse_with_base_dir(&input, Some(base_dir))
}

pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let mut rewritten = String::new();
    for (idx, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            rewritten.push('\n');
            continue;
        }
        let parsed: JsonLine = serde_json::from_str(trimmed)
            .with_context(|| format!("line {}: invalid json", idx + 1))?;
        let resolved = match parsed {
            JsonLine::Script {
                parent_id,
                src: None,
                path: Some(p),
            } => {
                let resolved_path = if Path::new(&p).is_absolute() {
                    PathBuf::from(&p)
                } else if let Some(b) = base_dir {
                    b.join(&p)
                } else {
                    PathBuf::from(&p)
                };
                let src =
                    std::fs::read_to_string(&resolved_path).with_context(|| {
                        format!("failed to read script file: {}", resolved_path.display())
                    })?;
                JsonLine::Script {
                    parent_id,
                    src: Some(src),
                    path: None,
                }
            }
            other => other,
        };
        rewritten.push_str(&serde_json::to_string(&resolved)?);
        rewritten.push('\n');
    }
    parse(&rewritten)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{parse_file, parse_with_base_dir};
    use opencat_core::jsonl::parse;

    #[test]
    fn resolves_script_path_relative_to_jsonl_file() {
        let fixture_dir = unique_test_dir("jsonl-script-path");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");

        let script_path = fixture_dir.join("scene.js");
        fs::write(&script_path, "ctx.getNode('root').opacity(0.75);")
            .expect("script fixture should be written");

        let jsonl_path = fixture_dir.join("scene.jsonl");
        fs::write(
            &jsonl_path,
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":90}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","path":"scene.js"}"#,
        )
        .expect("jsonl fixture should be written");

        let parsed = parse_file(&jsonl_path).expect("jsonl with script path should parse");

        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('root').opacity(0.75);")
        );

        fs::remove_dir_all(&fixture_dir).expect("fixture dir should be removed");
    }

    #[test]
    fn resolves_absolute_script_path_directly() {
        let fixture_dir = unique_test_dir("jsonl-absolute-script-path");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");

        let script_path = fixture_dir.join("scene.js");
        fs::write(&script_path, "ctx.getNode('root').opacity(0.25);")
            .expect("script fixture should be written");

        let jsonl_path = fixture_dir.join("scene.jsonl");
        fs::write(
            &jsonl_path,
            format!(
                "{{\"type\":\"composition\",\"width\":640,\"height\":360,\"fps\":30,\"frames\":90}}\n\
{{\"id\":\"root\",\"parentId\":null,\"type\":\"div\",\"className\":\"flex\",\"text\":null}}\n\
{{\"type\":\"script\",\"path\":\"{}\"}}",
                script_path.display()
            ),
        )
        .expect("jsonl fixture should be written");

        let parsed = parse_file(&jsonl_path).expect("jsonl with absolute script path should parse");

        assert_eq!(
            parsed.script.as_deref(),
            Some("ctx.getNode('root').opacity(0.25);")
        );

        fs::remove_dir_all(&fixture_dir).expect("fixture dir should be removed");
    }

    #[test]
    fn core_parse_rejects_script_with_unresolved_path() {
        let err = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":90}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","path":"nonexistent.js"}"#,
        )
        .err()
        .expect("core parse should reject script with path");

        assert!(err.to_string().contains("must be parsed via host"));
    }

    fn unique_test_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("opencat-{name}-{nanos}"))
    }
}
