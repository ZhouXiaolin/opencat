//! JSX frontend checks: entry extraction, per-frame driver, relative imports.
//!
//! The acceptance bar for the JSX experiment is "the ported example produces
//! the same composition" — `examples/dribbble-logo-animated.jsx` is the
//! pixel-verified port of `examples/dribbble-logo-animated.xml` (verified via
//! `opencat render <file> out.png <frame>` frame-by-frame). These tests lock
//! the structural contract that makes that port work.

use std::path::{Path, PathBuf};

use opencat_core::parse::node::{Node, NodeKind};
use opencat_engine::jsx::parse_with_base_dir;

fn collect_ids(node: &Node, out: &mut Vec<String>) {
    let id = node.style_ref().id.clone();
    if !id.is_empty() {
        out.push(id);
    }
    match node.kind() {
        NodeKind::Div(div) => {
            for child in div.children_ref() {
                collect_ids(child, out);
            }
        }
        NodeKind::Canvas(canvas) => {
            for child in canvas.hidden_children_ref() {
                collect_ids(child, out);
            }
        }
        NodeKind::Video(video) => {
            for child in video.children_ref() {
                collect_ids(child, out);
            }
        }
        _ => {}
    }
}

fn parse_ids(source: &str, base_dir: &Path) -> (Vec<String>, String) {
    let parsed = parse_with_base_dir(source, Some(base_dir)).expect("jsx parse");
    let mut ids = Vec::new();
    collect_ids(&parsed.root, &mut ids);
    let driver = parsed.script.clone().unwrap_or_default();
    (ids, driver)
}

#[test]
fn dribbble_port_extracts_tree_and_frame_driver() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/dribbble-logo-animated.jsx")
        .canonicalize()
        .expect("example file");
    let source = std::fs::read_to_string(&path).expect("read example");
    let parsed = parse_with_base_dir(&source, path.parent()).expect("jsx parse");

    assert_eq!((parsed.width, parsed.height, parsed.fps), (1920, 1080, 30));
    assert!((parsed.duration - 6.0).abs() < 1e-9);

    let (ids, driver) = parse_ids(&source, path.parent().unwrap());
    for expected in ["root", "pink-glow", "logo-container", "logo-path", "particle-canvas"] {
        assert!(ids.iter().any(|id| id == expected), "missing id `{expected}` in {ids:?}");
    }
    // Per-frame driver: reset, re-render, replay `use*` hooks.
    assert!(driver.contains("__ocResetFrame()"), "driver: {driver}");
    assert!(driver.contains("__ocRunFrameHooks()"), "driver: {driver}");
    assert!(driver.contains("__ocModuleSources"), "driver: {driver}");
}

#[test]
fn relative_imports_resolve_across_modules() {
    let dir = std::env::temp_dir().join(format!("opencat-jsx-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");

    std::fs::write(
        dir.join("main.jsx"),
        r#"
import { Box } from './lib.js';

export const meta = { width: 100, height: 100, fps: 30, duration: 1 };

export default function Scene() {
  return (
    <div id="root" class="w-[100px] h-[100px]">
      <Box id="box" label="hi" />
    </div>
  );
}
"#,
    )
    .expect("write main");

    std::fs::write(
        dir.join("lib.js"),
        r#"
export function Box(props) {
  return <text id={props.id} class="text-[16px]">{props.label}</text>;
}
"#,
    )
    .expect("write lib");

    let source = std::fs::read_to_string(dir.join("main.jsx")).expect("read main");
    let (ids, _driver) = parse_ids(&source, &dir);

    let _ = std::fs::remove_dir_all(&dir);
    assert!(ids.iter().any(|id| id == "root"), "ids: {ids:?}");
    assert!(ids.iter().any(|id| id == "box"), "ids: {ids:?}");
}
