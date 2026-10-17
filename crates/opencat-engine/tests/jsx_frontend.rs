//! JSX frontend checks: entry extraction, per-frame driver, relative imports.
//!
//! The acceptance bar for the JSX experiment is "the ported example produces
//! the same composition" — `examples/dribbble-logo-animated.jsx` is the
//! pixel-verified port of `examples/dribbble-logo-animated.jsx` (verified via
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

#[test]
fn inline_style_attribute_reaches_node_style() {
    // `style` is the standard HTML attribute. Both spellings must work:
    // a declaration string, and the React object form (camelCase keys,
    // unitless numbers). Inline declarations win over class rules.
    let dir = std::env::temp_dir().join(format!("opencat-jsx-style-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(
        dir.join("main.jsx"),
        r#"
export const meta = { width: 100, height: 100, fps: 30, duration: 1 };

export default function Scene() {
  return (
    <div id="root">
      <div id="str" style="width: 120px; opacity: 0.25" />
      <div id="obj" style={{ height: 40, backgroundColor: '#ff0000' }} />
      <div id="override" class="w-[10px]" style="width: 70px" />
    </div>
  );
}
"#,
    )
    .expect("write main");

    let source = std::fs::read_to_string(dir.join("main.jsx")).expect("read main");
    let parsed = parse_with_base_dir(&source, Some(&dir)).expect("jsx parse");
    let _ = std::fs::remove_dir_all(&dir);

    fn find<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
        if node.style_ref().id == id {
            return Some(node);
        }
        match node.kind() {
            NodeKind::Div(div) => div.children_ref().iter().find_map(|c| find(c, id)),
            NodeKind::Canvas(canvas) => canvas.hidden_children_ref().iter().find_map(|c| find(c, id)),
            _ => None,
        }
    }

    let root = &parsed.root;
    let str_style = find(root, "str").expect("str node");
    assert_eq!(str_style.style_ref().width, Some(120.0));
    assert_eq!(str_style.style_ref().opacity, Some(0.25));

    let obj_style = find(root, "obj").expect("obj node");
    assert_eq!(obj_style.style_ref().height, Some(40.0), "对象形态数字带 px");
    assert_eq!(
        obj_style.style_ref().bg_color,
        Some(opencat_core::style::ColorToken::Custom(255, 0, 0, 255)),
        "camelCase 键转 kebab 后应命中"
    );

    let override_style = find(root, "override").expect("override node");
    assert_eq!(override_style.style_ref().width, Some(70.0), "内联应覆盖类规则");
}

#[test]
fn nexus_port_manifests_document_fonts() {
    // `export const fonts` is a top-level export, separate from `meta` — this
    // locks that it reaches the manifest (the XML `<fonts>` block equivalent).
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/nexus7-cyberpunk.jsx")
        .canonicalize()
        .expect("example file");
    let source = std::fs::read_to_string(&path).expect("read example");
    let parsed = parse_with_base_dir(&source, path.parent()).expect("jsx parse");

    let manifest = &parsed.font_manifest;
    assert_eq!(manifest.default_face_id.as_deref(), Some("noto-sans"));
    assert_eq!(manifest.faces.len(), 5, "faces: {:?}", manifest.faces);
    for id in ["noto-sans", "orbitron", "jmono", "jmono-bold", "emoji"] {
        assert!(manifest.face_by_id(id).is_some(), "missing face `{id}`");
    }
}

#[test]
fn liquid_port_parses_full_three_script_body() {
    // Pixel parity for this port is pending a machine with a working wgpu
    // adapter (the XML original fails to render here too). What IS locked:
    // the whole `<script>` body survives the transform and compiles — module
    // evaluation wraps every module source in `new Function(...)`, so a
    // mangled `<` comparison or template literal would fail right here.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/liquid-brand-refraction/liquid-brand-refraction.jsx")
        .canonicalize()
        .expect("example file");
    let source = std::fs::read_to_string(&path).expect("read example");
    let (ids, driver) = parse_ids(&source, path.parent().unwrap());

    assert!(ids.iter().any(|id| id == "root"), "ids: {ids:?}");
    assert!(
        ids.iter().any(|id| id == "threecanvas"),
        "ids: {ids:?}"
    );
    // The verbatim script body, intact after the transform.
    for marker in [
        "T3.Renderer(",
        "scene3d.add(bgPlane3d)",
        "renderer3d.render(scene3d, camera3d)",
        "gl_FragColor",
        "137.508 * Math.PI",
    ] {
        assert!(driver.contains(marker), "driver missing `{marker}`");
    }
}
