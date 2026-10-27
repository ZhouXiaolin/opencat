//! JSX frontend checks: entry extraction, per-frame driver, relative imports,
//! and the single-file multi-segment bundle the vite plugin emits.

use std::path::Path;

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

/// `gsap` 的协议桩（与 script_runtime_tests 里的同类桩同形）。bundle 里的
/// 真包无法在测试里打包（没有 esbuild），extract 也从不跑动画 hook，所以
/// 段式 bundle 只带桩即可锁住「一个文件多个模块段」这条契约。
const GSAP_STUB: &str = r#"
var __ocStubGsap = {
    timeline: function (vars) {
        if (!vars || vars.paused !== true) throw new Error('timeline must be created paused');
        var tweens = [];
        var tl = {
            to: function (target) { tweens.push(String(target)); return tl; },
            time: function () { return 0; },
        };
        return tl;
    },
    to: function () {},
};
__ocStubGsap.default = __ocStubGsap;
module.exports = { gsap: __ocStubGsap, default: __ocStubGsap, TimelineLite: function () {} };
"#;

/// 把 `source` 与 gsap 桩拼成单文件多段 bundle，形状与打包器产物一致。
fn with_gsap_segment(source: &str) -> String {
    format!("/*@@oc-module:gsap@@*/{GSAP_STUB}\n/*@@oc-module:<entry>@@*/\n{source}")
}

#[test]
fn segmented_bundle_supplies_gsap_as_its_own_module() {
    // 段式 bundle：entry 只写 `import { gsap } from 'gsap'`，gsap 的源码在
    // 同文件的另一个段里。锁住的是切段 + 模块表灌入这条链路——没有它，
    // `gsap` 会落到 resolveSpec 报缺包。
    let dir = std::env::temp_dir().join(format!("opencat-jsx-seg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let entry = r#"
import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 100, height: 100, fps: 30, duration: 1 };

export default function Scene() {
  useGSAP(() => {
    gsap.timeline({ paused: true }).to('#root', { opacity: 0 });
  });
  return <div id="root" />;
}
"#;
    let parsed = parse_with_base_dir(&with_gsap_segment(entry), Some(&dir)).expect("jsx parse");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(parsed.width, 100);
    assert_eq!(parsed.root.style_ref().id, "root");

    // 同名 gsap 段缺失时，缺包错误必须仍然指向项目 bundle 的段约定。
    let err = parse_with_base_dir(entry, Some(&dir)).expect_err("missing gsap segment");
    assert!(err.to_string().contains("unknown module"), "unexpected error: {err}");
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
  return <p id={props.id} class="text-[16px]">{props.label}</p>;
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
fn html_markup_rules_apply() {
    // 闭合标签必须匹配、void 元素不吞兄弟、实体解码、`<img>` 别名。
    let dir = std::env::temp_dir().join(format!("opencat-jsx-html-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");

    let parse = |source: &str| parse_with_base_dir(source, Some(&dir));

    let err = parse(
        r#"export const meta = { width: 10, height: 10, fps: 30, duration: 1 };
export default function A() { return <div id="root"><p>a</span></div>; }"#,
    )
    .expect_err("mismatched closing tag must fail");
    assert!(
        err.to_string().contains("Expected corresponding JSX closing tag"),
        "unexpected error: {err}"
    );

    // 标准 JSX 语法：`}` 在 JSXText 里必须转义（`{'}'}` / `&rbrace;`）——
    // 缺闭合标签的树尾随 `}` 时，oxc 先报这条规范错误。
    let err = parse(
        r#"export const meta = { width: 10, height: 10, fps: 30, duration: 1 };
export default function A() {
  return (
    <div id="root">
      <p>a</p>
  );
}"#,
    )
    .expect_err("stray `}` in JSX text must fail");
    assert!(
        err.to_string().contains("Unexpected token"),
        "unexpected error: {err}"
    );

    let source = r#"export const meta = { width: 10, height: 10, fps: 30, duration: 1 };
export default function A() {
  return <div id="root">
    <p id="t">a &amp; b &lt;c&gt;</p>
    <img id="i" src="a.png" />
    <div id="after" />
  </div>;
}"#;
    let parsed = parse(source).expect("html-style markup parses");
    fn find<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
        if node.style_ref().id == id {
            return Some(node);
        }
        match node.kind() {
            NodeKind::Div(div) => div.children_ref().iter().find_map(|c| find(c, id)),
            _ => None,
        }
    }
    let root = &parsed.root;
    let NodeKind::Text(t) = find(root, "t").expect("text node").kind() else {
        panic!("t must be a text node");
    };
    assert_eq!(t.content(), "a & b <c>", "实体应解码");
    let image = find(root, "i").expect("<img> 应映射为 image 节点");
    assert!(matches!(image.kind(), NodeKind::Image(_)), "kind: {:?}", image.kind());
    assert!(find(root, "after").is_some(), "void 元素不应吞掉兄弟节点");

    let err = parse(
        r#"export const meta = { width: 10, height: 10, fps: 30, duration: 1 };
export default function A() { return <section id="root" />; }"#,
    )
    .expect_err("unsupported tag must fail");
    assert!(
        err.to_string().contains("unsupported element <section>"),
        "unexpected error: {err}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn variables_port_manifests_document_fonts() {
    // `export const fonts` is a top-level export, separate from `meta` — this
    // locks that it reaches the manifest (the XML `<fonts>` block equivalent).
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/variables-launch/variables-launch.jsx")
        .canonicalize()
        .expect("example file");
    let source = std::fs::read_to_string(&path).expect("read example");
    let parsed = parse_with_base_dir(&with_gsap_segment(&source), path.parent()).expect("jsx parse");

    let manifest = &parsed.font_manifest;
    assert_eq!(manifest.default_face_id.as_deref(), Some("inter"));
    assert_eq!(manifest.faces.len(), 5, "faces: {:?}", manifest.faces);
    for id in ["inter", "inter400", "inter800", "inter900", "mono500"] {
        assert!(manifest.face_by_id(id).is_some(), "missing face `{id}`");
    }
}

