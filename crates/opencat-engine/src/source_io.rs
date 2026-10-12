use std::path::Path;

use anyhow::{Context, Result};

use opencat_core::parse::document::ParsedComposition;

/// JSX is the only text frontend. External script locators stay logical:
/// hosts fill HostInputs::insert_script_text (issue #20).
pub fn parse_file(path: impl AsRef<Path>) -> Result<ParsedComposition> {
    let path = path.as_ref();
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    crate::jsx::parse_with_base_dir(&input, Some(base_dir))
}

pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    crate::jsx::parse_with_base_dir(input, base_dir)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::parse_file;
    use opencat_core::lifecycle::{CompositionDraft, HostInputs, ResourceKind};
    
    #[test]
    fn jsx_import_resolves_relative_modules_from_base_dir() {
        let fixture_dir = unique_test_dir("jsx-script-path");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");

        let script_path = fixture_dir.join("scene.js");
        fs::write(&script_path, "export const nodeOpacity = 0.75;")
            .expect("script fixture should be written");

        let jsx_path = fixture_dir.join("scene.jsx");
        fs::write(
            &jsx_path,
            r#"import { nodeOpacity } from "./scene.js";
export const meta = { width: 640, height: 360, fps: 30, duration: 3 };
export default function App() {
  return <div id="root" class="flex" opacity={nodeOpacity} />;
}"#,
        )
        .expect("jsx fixture should be written");

        // Relative imports resolve against the document base_dir and the whole
        // module graph is inlined into the per-frame driver at parse time.
        let parsed = parse_file(&jsx_path).expect("jsx with script import should parse");
        assert!(
            parsed.script.as_deref().unwrap_or_default().contains("nodeOpacity"),
            "driver must inline the imported module graph"
        );

        fs::remove_dir_all(&fixture_dir).expect("fixture dir should be removed");
    }

    #[test]
    fn core_parse_keeps_script_path_as_external_without_host() {
        let extract = serde_json::json!({
            "meta": {"width": 640.0, "height": 360.0, "fps": 30.0, "duration": 3.0},
            "lines": [
                {"type":"div","id":"root","parentId":null,"className":"flex"},
                {"type":"script","path":"nonexistent.js"},
            ],
        });
        let parsed = opencat_core::parse::jsx::from_extract(&extract, "", None)
            .expect("extract accepts path as logical locator");

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
    fn parse_jsx_file_minimal() {
        let fixture_dir = unique_test_dir("jsx-parse");
        fs::create_dir_all(&fixture_dir).expect("fixture dir");
        let jsx_path = fixture_dir.join("test.jsx");
        fs::write(
            &jsx_path,
            r#"export const meta = { width: 320, height: 240, fps: 30, duration: 1 };
export default function App() {
  return <div id="root" class="w-[320px] h-[240px]" />;
}"#,
        )
        .expect("jsx fixture");
        let parsed = parse_file(&jsx_path).expect("jsx should parse");
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
