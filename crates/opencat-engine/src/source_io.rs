use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use opencat_core::ir::asset_id::AssetId;
use opencat_core::lifecycle::{HostInputs, ResourceKind};
use opencat_core::parse::document::ParsedComposition;
use opencat_core::parse::jsonl::parse_with_base_dir as core_parse_with_base_dir;
use opencat_core::script::asset_id_for_script_locator;

pub fn parse_file(path: impl AsRef<Path>) -> Result<ParsedComposition> {
    let path = path.as_ref();
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

    match path.extension().and_then(|ext| ext.to_str()) {
        Some("xml") => opencat_core::parse::markup::parse_with_base_dir(&input, Some(base_dir)),
        // Core parse keeps external script locators logical; host loads text later.
        _ => core_parse_with_base_dir(&input, Some(base_dir)),
    }
}

pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let trimmed = input.trim();
    if trimmed.starts_with('{') {
        // Do not rewrite script path lines into inline src. Core keeps logical
        // locators; hosts fill HostInputs::insert_script_text (issue #20).
        core_parse_with_base_dir(input, base_dir)
    } else {
        opencat_core::parse::markup::parse_with_base_dir(input, base_dir)
    }
}

/// Read external script files declared by draft requirements and insert them
/// into host inputs. `base_dir` resolves relative logical paths.
pub fn fill_script_texts_from_disk(
    inputs: &mut HostInputs,
    requirements: &opencat_core::lifecycle::HostRequirements,
    base_dir: Option<&Path>,
) -> Result<()> {
    for req in requirements.requests() {
        if req.kind != ResourceKind::Script {
            continue;
        }
        // AssetId key is "script:path:..." or "script:url:...".
        // Strip the prefix to recover the logical locator.
        let key = &req.asset_id.key;
        let locator = if let Some(path) = key.strip_prefix("script:path:") {
            path
        } else if key.starts_with("script:url:") {
            // Local host cannot fetch URLs; leave missing so prepare fails.
            continue;
        } else {
            continue;
        };
        let resolved = resolve_script_path(locator, base_dir);
        let text = std::fs::read_to_string(&resolved).with_context(|| {
            format!("failed to read script file: {}", resolved.display())
        })?;
        inputs
            .insert_script_text(req.asset_id.clone(), text)
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    Ok(())
}

fn resolve_script_path(locator: &str, base_dir: Option<&Path>) -> PathBuf {
    let path = Path::new(locator);
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(b) = base_dir {
        b.join(path)
    } else {
        path.to_path_buf()
    }
}

/// Convenience for hosts that already know a locator string (tests).
pub fn script_asset_id(locator: &str) -> AssetId {
    asset_id_for_script_locator(locator)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{fill_script_texts_from_disk, parse_file, parse_with_base_dir};
    use opencat_core::lifecycle::{CompositionDraft, HostInputs, ResourceKind};
    use opencat_core::parse::jsonl::parse;
    
    #[test]
    fn parse_keeps_external_script_as_requirement_not_inline() {
        let fixture_dir = unique_test_dir("jsonl-script-path");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");

        let script_path = fixture_dir.join("scene.js");
        fs::write(&script_path, "ctx.getNode('root').opacity(0.75);")
            .expect("script fixture should be written");

        let jsonl_path = fixture_dir.join("scene.jsonl");
        fs::write(
            &jsonl_path,
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","path":"scene.js"}"#,
        )
        .expect("jsonl fixture should be written");

        let parsed = parse_file(&jsonl_path).expect("jsonl with script path should parse");
        // Core must not inline file contents into ParsedComposition.script.
        assert!(
            parsed.script.is_none(),
            "external scripts must not rewrite the composition string field"
        );
        let draft = CompositionDraft::from_parsed(parsed);
        let scripts: Vec<_> = draft
            .requirements()
            .requests()
            .iter()
            .filter(|r| r.kind == ResourceKind::Script)
            .collect();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].asset_id.key, "script:path:scene.js");

        let mut inputs = HostInputs::empty();
        fill_script_texts_from_disk(&mut inputs, draft.requirements(), Some(&fixture_dir))
            .expect("host should load script text");
        let prepared = draft.prepare(inputs).expect("prepare with script text");
        let driver = prepared
            .parsed()
            .root
            .style_ref()
            .script_driver
            .as_ref()
            .expect("global script attached to root");
        assert_eq!(driver.source, "ctx.getNode('root').opacity(0.75);");

        fs::remove_dir_all(&fixture_dir).expect("fixture dir should be removed");
    }

    #[test]
    fn resolves_absolute_script_path_via_host_inputs() {
        let fixture_dir = unique_test_dir("jsonl-absolute-script-path");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");

        let script_path = fixture_dir.join("scene.js");
        fs::write(&script_path, "ctx.getNode('root').opacity(0.25);")
            .expect("script fixture should be written");

        let jsonl = format!(
            "{{\"type\":\"composition\",\"width\":640,\"height\":360,\"fps\":30,\"duration\":3}}\n\
{{\"id\":\"root\",\"parentId\":null,\"type\":\"div\",\"className\":\"flex\",\"text\":null}}\n\
{{\"type\":\"script\",\"path\":\"{}\"}}",
            script_path.display()
        );
        let parsed = parse_with_base_dir(&jsonl, Some(&fixture_dir))
            .expect("jsonl with absolute script path should parse");
        let draft = CompositionDraft::from_parsed(parsed);
        let mut inputs = HostInputs::empty();
        fill_script_texts_from_disk(&mut inputs, draft.requirements(), Some(&fixture_dir))
            .expect("load");
        let prepared = draft.prepare(inputs).expect("prepare");
        let driver = prepared
            .parsed()
            .root
            .style_ref()
            .script_driver
            .as_ref()
            .expect("driver");
        assert_eq!(driver.source, "ctx.getNode('root').opacity(0.25);");

        fs::remove_dir_all(&fixture_dir).expect("fixture dir should be removed");
    }

    #[test]
    fn core_parse_keeps_script_path_as_external_without_host() {
        let parsed = parse(
            r#"{"type":"composition","width":640,"height":360,"fps":30,"duration":3}
{"id":"root","parentId":null,"type":"div","className":"flex","text":null}
{"type":"script","path":"nonexistent.js"}"#,
        )
        .expect("core parse accepts path as logical locator");

        assert!(parsed.script.is_none());
        let draft = CompositionDraft::from_parsed(parsed);
        let scripts: Vec<_> = draft
            .requirements()
            .requests()
            .iter()
            .filter(|r| r.kind == ResourceKind::Script)
            .collect();
        assert_eq!(scripts.len(), 1);
        // Without host text, prepare fails.
        let err = draft
            .prepare(HostInputs::empty())
            .expect_err("missing script text must fail prepare");
        assert!(err.to_string().contains("script"));
    }

    #[test]
    fn parse_xml_file() {
        let fixture_dir = unique_test_dir("xml-parse");
        fs::create_dir_all(&fixture_dir).expect("fixture dir");
        let xml_path = fixture_dir.join("test.xml");
        fs::write(
            &xml_path,
            r#"<opencat width="320" height="240" fps="30" duration="0.033333333333"><div id="root" /></opencat>"#,
        )
        .expect("xml fixture");
        let parsed = parse_file(&xml_path).expect("xml should parse");
        assert_eq!(parsed.width, 320);
        assert_eq!(parsed.height, 240);
        fs::remove_dir_all(&fixture_dir).ok();
    }

    fn unique_test_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("opencat-{name}-{nanos}"))
    }
}
