# Div / Timeline Unified Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `layer` and root-inferred timeline parsing with explicit `tl` nodes plus `transition.parentId`, while preserving the Rust `timeline().sequence(...).transition(...).sequence(...)` API and lowering `caption` into ordinary text before final display item construction.

**Architecture:** Parse `type: "tl"` as an explicit node in the JSONL tree, then build it into the existing `TimelineNode` runtime shape from its direct children plus transitions scoped by `parentId`. Remove `LayerNode` from runtime and migrate all overlay compositions to ordinary sibling nodes under a shared parent. Reuse the existing `ScriptFrameCtx` propagation so captions resolve SRT text from the nearest inherited time context and fall back to global composition time automatically.

**Tech Stack:** Rust, serde JSON parsing, OpenCat scene graph/runtime, `cargo test`, Markdown docs, JSONL fixtures

---

## File Map

- Modify: `src/jsonl.rs`
  - Add explicit `tl` parsing, require `transition.parentId`, delete `layer` parsing, replace parser tests.
- Modify: `src/jsonl/builder.rs`
  - Build timeline nodes from a `tl` element's direct children and local transitions; delete `build_layer_root(...)` and root-sequence tracing helpers.
- Modify: `src/scene/node.rs`
  - Remove `NodeKind::Layer` and all `From<LayerNode>` conversions.
- Delete: `src/scene/layer.rs`
  - Remove the old parallel-composition node entirely.
- Modify: `src/scene/mod.rs`
  - Stop exporting the deleted `layer` module.
- Modify: `src/lib.rs`
  - Stop re-exporting `LayerNode` / `layer()`.
- Modify: `src/scene/time.rs`
  - Remove layer-specific frame state logic and replace tests with ordinary sibling-node coverage.
- Modify: `src/element/resolve.rs`
  - Remove `resolve_layer_as_div(...)`; resolve captions using inherited `ScriptFrameCtx.current_frame`.
- Modify: `src/inspect.rs`
  - Remove `Layer` metadata traversal.
- Modify: `src/runtime/preflight.rs`
  - Remove `Layer` source collection recursion.
- Modify: `src/render.rs`
  - Rewrite layer-based render regression tests to use `div(root).child(timeline).child(caption)`.
- Modify: `json/the-boys-layer-caption-15s.jsonl`
  - Rewrite the sample to explicit `tl` + sibling `caption`.
- Modify: `opencat.md`
  - Remove `layer` schema and replace examples with explicit `tl`.
- Modify: `opencat.zh.md`
  - Remove `layer` schema and replace examples with explicit `tl`.

## Task 1: Introduce Explicit `tl` Schema and Remove Legacy `layer` Input

**Files:**
- Modify: `src/jsonl.rs`
- Test: `src/jsonl.rs`

- [ ] **Step 1: Write the failing parser tests for explicit `tl` and legacy `layer` rejection**

```rust
#[test]
fn parser_accepts_explicit_tl_root_and_local_transition() {
    let parsed = parse(
        r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":25}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":25}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":10}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":10}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":5}"#,
    )
    .expect("explicit tl jsonl should parse");

    assert_eq!(parsed.frames, 25);
    assert!(matches!(parsed.root.kind(), NodeKind::Div(_)));
}

#[test]
fn parser_requires_transition_parent_id() {
    let err = parse(
        r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":25}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":25}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":10}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":10}
{"type":"transition","from":"scene-a","to":"scene-b","effect":"fade","duration":5}"#,
    )
    .err()
    .expect("missing transition parentId should fail");

    assert!(err.to_string().contains("parentId"));
}

#[test]
fn parser_rejects_legacy_layer_records() {
    let err = parse(
        r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":25}
{"id":"scene-a","parentId":null,"type":"div","className":"","duration":10}
{"id":"subs","parentId":null,"type":"caption","className":"absolute","path":"sub.srt"}
{"type":"layer","children":["scene-a","subs"]}"#,
    )
    .err()
    .expect("legacy layer input should fail");

    assert!(err.to_string().contains("layer"));
}
```

- [ ] **Step 2: Run the targeted parser tests and confirm they fail for the expected reasons**

Run:

```bash
rtk cargo test parser_accepts_explicit_tl_root_and_local_transition
rtk cargo test parser_requires_transition_parent_id
rtk cargo test parser_rejects_legacy_layer_records
```

Expected:

```text
FAIL: unknown variant `tl`
FAIL: missing field `parentId`
FAIL: legacy `layer` still parses
```

- [ ] **Step 3: Update `JsonLine`, parsed structs, and `parse_with_base_dir(...)` to treat `tl` as a first-class node and reject `layer`**

```rust
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
enum JsonLine {
    #[serde(rename = "tl")]
    Tl {
        id: String,
        #[serde(rename = "parentId")]
        parent_id: Option<String>,
        #[serde(rename = "className")]
        class_name: Option<String>,
    },
    #[serde(rename = "transition")]
    Transition {
        #[serde(rename = "parentId")]
        parent_id: String,
        from: String,
        to: String,
        effect: String,
        duration: u32,
        direction: Option<String>,
        timing: Option<String>,
        damping: Option<f32>,
        stiffness: Option<f32>,
        mass: Option<f32>,
        seed: Option<f32>,
        #[serde(rename = "hueShift")]
        hue_shift: Option<f32>,
        #[serde(rename = "maskScale")]
        mask_scale: Option<f32>,
    },
    // remove JsonLine::Layer entirely
}

#[derive(Debug, Clone)]
enum ParsedElementKind {
    Div,
    Text { content: String },
    Canvas,
    Image { source: ImageSource },
    Icon { name: String },
    Video { path: PathBuf },
    Caption { path: PathBuf },
    Timeline,
}

#[derive(Debug, Clone)]
struct ParsedTransition {
    parent_id: String,
    from: String,
    to: String,
    effect: String,
    duration: u32,
    direction: Option<String>,
    timing: Option<String>,
    damping: Option<f32>,
    stiffness: Option<f32>,
    mass: Option<f32>,
    seed: Option<f32>,
    hue_shift: Option<f32>,
    mask_scale: Option<f32>,
}
```

- [ ] **Step 4: Re-run the parser tests until the new schema is accepted and legacy `layer` is rejected**

Run:

```bash
rtk cargo test parser_accepts_explicit_tl_root_and_local_transition
rtk cargo test parser_requires_transition_parent_id
rtk cargo test parser_rejects_legacy_layer_records
```

Expected:

```text
PASS
PASS
PASS
```

- [ ] **Step 5: Commit the schema conversion checkpoint**

```bash
rtk git add src/jsonl.rs
rtk git commit -m "refactor: add explicit tl jsonl schema"
```

## Task 2: Build Timeline Nodes from `tl` Children and Local Transitions

**Files:**
- Modify: `src/jsonl.rs`
- Modify: `src/jsonl/builder.rs`
- Test: `src/jsonl.rs`

- [ ] **Step 1: Write the failing builder/parser tests for local transition scoping and adjacency validation**

```rust
#[test]
fn parser_builds_tl_node_from_direct_children() {
    let parsed = parse(
        r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":25}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":25}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":10}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":10}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-b","effect":"fade","duration":5}"#,
    )
    .expect("tl jsonl should parse");

    let NodeKind::Div(root) = parsed.root.kind() else {
        panic!("root should be div");
    };
    let NodeKind::Timeline(tl) = root.children_ref()[0].kind() else {
        panic!("child should be timeline");
    };
    assert_eq!(tl.duration_in_frames(), 25);
}

#[test]
fn parser_rejects_transition_that_targets_non_adjacent_tl_child() {
    let err = parse(
        r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":35}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":35}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":10}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":10}
{"id":"scene-c","parentId":"main-tl","type":"div","className":"","duration":10}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"scene-c","effect":"fade","duration":5}"#,
    )
    .err()
    .expect("non-adjacent transition should fail");

    assert!(err.to_string().contains("adjacent"));
}

#[test]
fn parser_rejects_transition_to_non_direct_tl_descendant() {
    let err = parse(
        r#"{"type":"composition","width":640,"height":360,"fps":30,"frames":25}
{"id":"root","parentId":null,"type":"div","className":"relative","duration":25}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"main-tl","type":"div","className":"","duration":10}
{"id":"scene-b","parentId":"main-tl","type":"div","className":"","duration":10}
{"id":"nested","parentId":"scene-b","type":"div","className":"","duration":10}
{"type":"transition","parentId":"main-tl","from":"scene-a","to":"nested","effect":"fade","duration":5}"#,
    )
    .err()
    .expect("transition to nested descendant should fail");

    assert!(err.to_string().contains("direct child"));
}
```

- [ ] **Step 2: Run the failing tests to confirm builder logic still depends on root-sequence inference**

Run:

```bash
rtk cargo test parser_builds_tl_node_from_direct_children
rtk cargo test parser_rejects_transition_that_targets_non_adjacent_tl_child
rtk cargo test parser_rejects_transition_to_non_direct_tl_descendant
```

Expected:

```text
FAIL: timeline child is not built from tl
FAIL: non-adjacent transition still slips through or misreports
FAIL: nested descendant is not rejected as invalid transition target
```

- [ ] **Step 3: Rewrite `src/jsonl/builder.rs` so timeline construction is driven by explicit `tl` nodes**

```rust
fn build_node(
    el: &ParsedElement,
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    transitions_by_parent: &HashMap<&str, Vec<&ParsedTransition>>,
    scripts_by_parent: &HashMap<String, Vec<String>>,
    fps: u32,
) -> anyhow::Result<Node> {
    match &el.kind {
        ParsedElementKind::Timeline => build_tl_node(
            el,
            children_map,
            transitions_by_parent,
            scripts_by_parent,
            fps,
        ),
        ParsedElementKind::Div => {
            let mut div_node = div();
            div_node.style = style;
            if let Some(children) = children_map.get(el.id.as_str()) {
                for child in children {
                    let child_node = build_node(
                        child,
                        children_map,
                        transitions_by_parent,
                        scripts_by_parent,
                        fps,
                    )?;
                    div_node = div_node.child(child_node);
                }
            }
            Ok(Node::new(div_node))
        }
        ParsedElementKind::Text { content } => {
            let mut text_node = text(content);
            text_node.style = style;
            Ok(Node::new(text_node))
        }
        ParsedElementKind::Caption { path } => {
            let entries = std::fs::read_to_string(path)
                .ok()
                .and_then(|content| parse_srt(&content, fps).ok())
                .unwrap_or_default();
            let mut caption_node = caption().path(path).entries(entries);
            caption_node.style = style;
            Ok(Node::new(caption_node))
        }
        ParsedElementKind::Canvas => {
            let mut canvas_node = canvas();
            canvas_node.style = style;
            Ok(Node::new(canvas_node))
        }
        ParsedElementKind::Image { source } => {
            let mut image_node = image();
            image_node.style = style;
            Ok(Node::new(match source {
                ImageSource::Path(path) => image_node.path(path),
                ImageSource::Url(url) => image_node.url(url.clone()),
                ImageSource::Query(query) => image_node.query(query.query.clone()),
                ImageSource::Unset => anyhow::bail!("image node requires one of: path, url, query"),
            }))
        }
        ParsedElementKind::Icon { name } => {
            let mut icon_node = lucide(name.clone());
            icon_node.style = style;
            Ok(Node::new(icon_node))
        }
        ParsedElementKind::Video { path } => {
            let mut video_node = video(path);
            video_node.style = style;
            Ok(Node::new(video_node))
        }
    }
}

fn build_tl_node(
    el: &ParsedElement,
    children_map: &HashMap<&str, Vec<&ParsedElement>>,
    transitions_by_parent: &HashMap<&str, Vec<&ParsedTransition>>,
    scripts_by_parent: &HashMap<String, Vec<String>>,
    fps: u32,
) -> anyhow::Result<Node> {
    let children = children_map.get(el.id.as_str()).cloned().unwrap_or_default();
    let child_order = children
        .iter()
        .map(|child| child.id.as_str())
        .collect::<Vec<_>>();
    let child_positions = child_order
        .iter()
        .enumerate()
        .map(|(index, id)| (*id, index))
        .collect::<HashMap<_, _>>();
    let mut transitions_by_pair = HashMap::new();

    for transition in transitions_by_parent
        .get(el.id.as_str())
        .into_iter()
        .flatten()
    {
        let Some(&from_index) = child_positions.get(transition.from.as_str()) else {
            anyhow::bail!("transition `{}` -> `{}` must reference tl direct child", transition.from, transition.to);
        };
        let Some(&to_index) = child_positions.get(transition.to.as_str()) else {
            anyhow::bail!("transition `{}` -> `{}` must reference tl direct child", transition.from, transition.to);
        };
        if to_index != from_index + 1 {
            anyhow::bail!("transition `{}` -> `{}` must connect adjacent tl children", transition.from, transition.to);
        }
        transitions_by_pair.insert((transition.from.as_str(), transition.to.as_str()), transition);
    }

    let mut builder = timeline();
    for (index, child) in children.iter().enumerate() {
        let duration = child
            .duration
            .ok_or_else(|| anyhow::anyhow!("timeline child `{}` is missing duration", child.id))?;
        builder = builder.sequence(duration, build_node(child, children_map, transitions_by_parent, scripts_by_parent, fps)?);
        if let Some(next) = children.get(index + 1) {
            if let Some(transition) = transitions_by_pair.get(&(child.id.as_str(), next.id.as_str())) {
                builder = builder.transition(build_transition(transition)?);
            }
        }
    }

    let mut node: Node = builder.into();
    node.kind().style_mut().id = el.id.clone();
    node.kind().style_mut().class_name = el.style.class_name.clone();
    Ok(node)
}
```

- [ ] **Step 4: Remove root-sequence / layer-only branches from `parse_with_base_dir(...)` and verify explicit `tl` now drives construction**

Run:

```bash
rtk cargo test parser_builds_tl_node_from_direct_children
rtk cargo test parser_rejects_transition_that_targets_non_adjacent_tl_child
rtk cargo test parser_rejects_transition_to_non_direct_tl_descendant
```

Expected:

```text
PASS
PASS
PASS
```

- [ ] **Step 5: Commit the explicit timeline-builder rewrite**

```bash
rtk git add src/jsonl.rs src/jsonl/builder.rs
rtk git commit -m "refactor: build timelines from explicit tl nodes"
```

## Task 3: Remove `LayerNode` from the Runtime and Replace It with Ordinary Sibling Composition

**Files:**
- Delete: `src/scene/layer.rs`
- Modify: `src/scene/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/scene/node.rs`
- Modify: `src/scene/time.rs`
- Modify: `src/inspect.rs`
- Modify: `src/runtime/preflight.rs`
- Test: `src/scene/time.rs`
- Test: `src/runtime/preflight.rs`

- [ ] **Step 1: Write failing runtime tests that prove ordinary sibling nodes replace `LayerNode`**

```rust
#[test]
fn frame_state_handles_div_root_with_timeline_and_caption_siblings() {
    let root = div()
        .id("root")
        .child(
            timeline()
                .sequence(10, div().id("scene-a").into())
                .transition(slide().timing(Easing::Linear, 5))
                .sequence(10, div().id("scene-b").into()),
        )
        .child(caption().id("subs").path("sub.srt").entries(vec![]));

    let frame_ctx = FrameCtx {
        frame: 12,
        fps: 30,
        width: 320,
        height: 180,
        frames: 25,
    };

    let state = super::frame_state_for_root(&root.into(), &frame_ctx);
    let FrameState::Scene { scene, .. } = state else {
        panic!("root div should still resolve as scene");
    };
    let NodeKind::Div(scene_div) = scene.kind() else {
        panic!("scene should remain a div");
    };
    assert_eq!(scene_div.children_ref().len(), 2);
}

#[test]
fn collect_sources_walks_div_children_without_layer_nodes() {
    let root = div()
        .id("root")
        .child(image().id("hero").url("https://example.com/a.png"))
        .child(timeline().sequence(10, div().id("scene-a").into()));
    let frame_ctx = FrameCtx {
        frame: 0,
        fps: 30,
        width: 320,
        height: 180,
        frames: 10,
    };
    let mut image_sources = HashSet::new();

    collect_sources(&root.into(), &frame_ctx, &mut image_sources);

    assert_eq!(image_sources.len(), 1);
}
```

- [ ] **Step 2: Run the failing runtime tests to confirm `LayerNode` is still part of the dispatch graph**

Run:

```bash
rtk cargo test frame_state_handles_div_root_with_timeline_and_caption_siblings
rtk cargo test collect_sources_walks_div_children_without_layer_nodes
```

Expected:

```text
FAIL: tests do not compile or runtime still expects LayerNode branches
```

- [ ] **Step 3: Delete `LayerNode`, remove all exports/conversions, and simplify runtime traversal**

```rust
// src/scene/mod.rs
pub mod composition;
pub mod easing;
pub mod node;
pub mod primitives;
pub mod script;
pub mod time;
pub mod transition;

// src/lib.rs
pub use scene::time::TimelineNode;
pub use scene::transition::{clock_wipe, fade, iris, light_leak, slide, timeline, wipe};

// src/scene/node.rs
pub enum NodeKind {
    Component(ComponentNode),
    Div(Div),
    Canvas(Canvas),
    Text(Text),
    Image(Image),
    Lucide(Lucide),
    Video(Video),
    Timeline(TimelineNode),
    Caption(CaptionNode),
}

// src/scene/time.rs
pub(crate) fn frame_state_for_root(root: &Node, ctx: &FrameCtx) -> FrameState {
    match root.kind() {
        NodeKind::Component(component) => frame_state_for_root(&component.render(ctx), ctx),
        NodeKind::Timeline(timeline) => frame_state_for_timeline(timeline, ctx),
        _ => FrameState::Scene {
            scene: root.clone(),
            script_frame_ctx: ScriptFrameCtx::global(ctx),
        },
    }
}

// src/runtime/preflight.rs
match node.kind() {
    NodeKind::Div(div) => {
        for child in div.children_ref() {
            collect_sources(child, frame_ctx, image_sources);
        }
    }
    NodeKind::Timeline(_) => collect_sources_from_frame_state(
        &frame_state_for_root(node, frame_ctx),
        frame_ctx,
        image_sources,
    ),
    NodeKind::Text(_) | NodeKind::Lucide(_) | NodeKind::Video(_) | NodeKind::Caption(_) => {}
    // remove NodeKind::Layer entirely
}
```

- [ ] **Step 4: Run the runtime tests and one existing timeline regression to verify removal is complete**

Run:

```bash
rtk cargo test frame_state_handles_div_root_with_timeline_and_caption_siblings
rtk cargo test collect_sources_walks_div_children_without_layer_nodes
rtk cargo test frame_state_uses_scene_local_progress_inside_timeline
```

Expected:

```text
PASS
PASS
PASS
```

- [ ] **Step 5: Commit the runtime cleanup**

```bash
rtk git add src/scene/mod.rs src/lib.rs src/scene/node.rs src/scene/time.rs src/inspect.rs src/runtime/preflight.rs
rtk git add src/scene/layer.rs
rtk git commit -m "refactor: remove runtime layer node"
```

## Task 4: Resolve `caption` from Inherited Time Context and Lower It to Ordinary Text

**Files:**
- Modify: `src/element/resolve.rs`
- Test: `src/element/resolve.rs`

- [ ] **Step 1: Write the failing caption resolution tests for nearest timeline time and global fallback**

```rust
#[test]
fn resolve_caption_uses_scene_local_time_inside_timeline() {
    let caption_node = caption()
        .id("subs")
        .path("sub.srt")
        .entries(vec![
            SrtEntry { index: 1, start_frame: 0, end_frame: 5, text: "Local A".into() },
            SrtEntry { index: 2, start_frame: 5, end_frame: 10, text: "Local B".into() },
        ]);
    let root = timeline()
        .sequence(10, div().id("scene-a").child(caption_node).into());
    let frame_ctx = FrameCtx { frame: 7, fps: 30, width: 320, height: 180, frames: 10 };
    let script_frame_ctx = ScriptFrameCtx::for_segment(&frame_ctx, 0, 10);
    let mut media = MediaContext::default();
    let mut assets = AssetsMap::default();
    let mut runtime = ScriptRuntimeCache::default();

    let tree = resolve_ui_tree_with_script_cache(
        &root.into(),
        &frame_ctx,
        &script_frame_ctx,
        &mut media,
        &mut assets,
        None,
        &mut runtime,
    )
    .expect("caption tree should resolve");

    assert!(format!("{tree:?}").contains("Local B"));
}

#[test]
fn resolve_caption_falls_back_to_global_time_when_no_nearer_time_context_exists() {
    let root = div().id("root").child(
        caption()
            .id("subs")
            .path("sub.srt")
            .entries(vec![
                SrtEntry { index: 1, start_frame: 0, end_frame: 5, text: "Global A".into() },
                SrtEntry { index: 2, start_frame: 5, end_frame: 10, text: "Global B".into() },
            ]),
    );
    let frame_ctx = FrameCtx { frame: 7, fps: 30, width: 320, height: 180, frames: 10 };
    let mut media = MediaContext::default();
    let mut assets = AssetsMap::default();
    let mut runtime = ScriptRuntimeCache::default();

    let tree = resolve_ui_tree(&root.into(), &frame_ctx, &mut media, &mut assets, None)
        .expect("root caption should resolve from global time");

    assert!(format!("{tree:?}").contains("Global B"));
}
```

- [ ] **Step 2: Run the failing caption tests and confirm `resolve_caption(...)` still uses `cx.frame_ctx.frame` directly**

Run:

```bash
rtk cargo test resolve_caption_uses_scene_local_time_inside_timeline
rtk cargo test resolve_caption_falls_back_to_global_time_when_no_nearer_time_context_exists
```

Expected:

```text
FAIL: local timeline caption still resolves from global frame
FAIL: global fallback coverage not yet explicit
```

- [ ] **Step 3: Switch caption text selection to inherited `ScriptFrameCtx` and keep the final node as ordinary text**

```rust
fn resolve_caption(
    caption: &CaptionNode,
    cx: &mut ResolveContext<'_>,
) -> Result<Option<ElementNode>> {
    let pushed = push_script_scope(caption.style_ref(), cx)?;
    let result = (|| {
        let mut style = caption.style_ref().clone();
        ensure!(
            !style.id.is_empty(),
            "node id is required for caption nodes before rendering"
        );
        apply_mutation_stack(&mut style, cx.mutation_stack);

        let caption_frame = cx.script_frame_ctx.current_frame;
        let content = text_content_from_stack(cx.mutation_stack, &style.id).or_else(|| {
            caption.active_text(caption_frame).map(|s| s.to_string())
        });

        let content = match content {
            Some(content) => content,
            None => return Ok(None),
        };

        let computed = compute_style(&style, cx.inherited_style);

        Ok(Some(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Text(ElementText {
                text: content,
                text_style: computed.text.clone(),
            }),
            style: computed,
            children: Vec::new(),
        }))
    })();
    if pushed {
        cx.mutation_stack.pop();
    }
    result
}
```

- [ ] **Step 4: Re-run the caption resolution tests and the existing inactive-caption regression**

Run:

```bash
rtk cargo test resolve_caption_uses_scene_local_time_inside_timeline
rtk cargo test resolve_caption_falls_back_to_global_time_when_no_nearer_time_context_exists
rtk cargo test resolve_caption_omits_inactive_entry
```

Expected:

```text
PASS
PASS
PASS
```

- [ ] **Step 5: Commit the caption lowering/time-context fix**

```bash
rtk git add src/element/resolve.rs
rtk git commit -m "fix: resolve captions from inherited time context"
```

## Task 5: Migrate Render Regressions, Fixtures, and Product Docs to the New Model

**Files:**
- Modify: `src/render.rs`
- Modify: `json/the-boys-layer-caption-15s.jsonl`
- Modify: `opencat.md`
- Modify: `opencat.zh.md`
- Test: `src/render.rs`

- [ ] **Step 1: Rewrite the render regression tests to use ordinary sibling composition and confirm they fail against old helpers**

```rust
#[test]
fn timeline_caption_sibling_renders_above_transition() {
    use crate::{Easing, SrtEntry, caption, fade, timeline};

    let composition = Composition::new("timeline_caption")
        .size(320, 180)
        .fps(30)
        .frames(25)
        .root(move |_| {
            div()
                .id("root")
                .child(
                    timeline()
                        .sequence(
                            10,
                            div()
                                .id("scene-a")
                                .bg(ColorToken::Black)
                                .child(text("A").id("a"))
                                .into(),
                        )
                        .transition(fade().timing(Easing::Linear, 5))
                        .sequence(
                            10,
                            div()
                                .id("scene-b")
                                .bg(ColorToken::Black)
                                .child(text("B").id("b"))
                                .into(),
                        ),
                )
                .child(
                    caption()
                        .id("subs")
                        .path("sub.srt")
                        .entries(vec![SrtEntry {
                            index: 1,
                            start_frame: 0,
                            end_frame: 25,
                            text: "Subtitle".into(),
                        }])
                        .text_color(ColorToken::White),
                )
                .into()
        })
        .build()
        .expect("composition should build");

    let mut session = RenderSession::new();
    let pixels = render_frame_rgba(&composition, 12, &mut session).expect("frame should render");

    assert!(pixels.iter().any(|&byte| byte > 0));
}
```

- [ ] **Step 2: Run the migrated render tests and confirm any remaining `layer()` references fail compilation or behavior checks**

Run:

```bash
rtk cargo test timeline_caption_sibling_renders_above_transition
rtk cargo test layered_single_scene_renders_bottom_scene_before_caption_overlay
rtk cargo test layered_root_caption_without_active_entry_does_not_fail_rendering
```

Expected:

```text
FAIL: old render tests still depend on layer()
```

- [ ] **Step 3: Migrate the render tests, JSON fixture, and Markdown docs to explicit `tl` plus sibling captions**

```rust
// src/render.rs
let composition = Composition::new("timeline_caption")
    .size(64, 64)
    .fps(30)
    .frames(1)
    .root(move |_| {
        div()
            .id("root")
            .child(
                div()
                    .id("scene")
                    .w_full()
                    .h_full()
                    .bg(ColorToken::Blue500),
            )
            .child(
                caption()
                    .id("subs")
                    .path("sub.srt")
                    .entries(vec![SrtEntry {
                        index: 1,
                        start_frame: 0,
                        end_frame: 1,
                        text: "Caption".into(),
                    }])
                    .absolute()
                    .left(8.0)
                    .top(8.0)
                    .text_color(ColorToken::White),
            )
            .into()
    })
    .build()
    .expect("composition should build");
```

```json
{"type":"composition","width":1280,"height":720,"fps":30,"frames":450}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1280px] h-[720px]"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene1","parentId":"main-tl","type":"div","className":"relative flex flex-col justify-between w-full h-full bg-[#0b1020] px-[72px] py-[56px]","duration":450}
{"id":"subline","parentId":"scene1","type":"text","className":"text-white","text":"Prime Video"}
{"id":"subs","parentId":"root","type":"caption","className":"absolute left-[64px] bottom-[40px] w-[1152px] px-[28px] py-[18px] rounded-[20px] bg-[#000000b8] text-[34px] leading-[44px] font-semibold text-center text-white","path":"subtitles.utf8.srt"}
```

```md
### Multi Scene via Explicit `tl`

    {"type":"composition","width":390,"height":844,"fps":30,"frames":162}
    {"id":"root","parentId":null,"type":"div","className":"relative w-[390px] h-[844px]"}
    {"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
    {"id":"scene1","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-white","duration":60}
    {"id":"scene2","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-slate-900","duration":90}
    {"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"fade","duration":12}
```

- [ ] **Step 4: Run the render regressions plus targeted parser tests and verify docs/fixtures are consistent**

Run:

```bash
rtk cargo test timeline_caption_sibling_renders_above_transition
rtk cargo test layered_single_scene_renders_bottom_scene_before_caption_overlay
rtk cargo test layered_root_caption_without_active_entry_does_not_fail_rendering
rtk cargo test parser_accepts_explicit_tl_root_and_local_transition
```

Expected:

```text
PASS
PASS
PASS
PASS
```

- [ ] **Step 5: Commit the fixture/doc migration**

```bash
rtk git add src/render.rs json/the-boys-layer-caption-15s.jsonl opencat.md opencat.zh.md
rtk git commit -m "docs: migrate examples to explicit tl nodes"
```

## Task 6: Run the Final Regression Sweep and Clean Up Residual `layer` References

**Files:**
- Modify: `src/jsonl.rs`
- Modify: `src/jsonl/builder.rs`
- Modify: `src/scene/node.rs`
- Modify: `src/scene/time.rs`
- Modify: `src/element/resolve.rs`
- Modify: `src/inspect.rs`
- Modify: `src/runtime/preflight.rs`
- Modify: `src/render.rs`

- [ ] **Step 1: Search for any leftover semantic `layer` references and make them fail the plan if they remain**

Run:

```bash
rtk grep 'NodeKind::Layer|LayerNode|build_layer_root|type":"layer"|layer\(' src json examples opencat.md opencat.zh.md
```

Expected:

```text
0 matches in scene/jsonl/render/docs code related to the removed layer concept
```

- [ ] **Step 2: If the search still returns semantic `layer` hits, remove them before running the full test sweep**

```text
Delete any semantic `layer` leftovers returned by Step 1, including exact hits such as:
- `pub use scene::layer::{LayerNode, layer};`
- `NodeKind::Layer(layer) =>`
- `{"type":"layer","children":[...]}`
- `build_layer_root(`
```

- [ ] **Step 3: Run the full targeted regression sweep for parser, runtime, resolve, and render**

Run:

```bash
rtk cargo test parser_accepts_explicit_tl_root_and_local_transition
rtk cargo test parser_builds_tl_node_from_direct_children
rtk cargo test frame_state_handles_div_root_with_timeline_and_caption_siblings
rtk cargo test resolve_caption_uses_scene_local_time_inside_timeline
rtk cargo test timeline_caption_sibling_renders_above_transition
rtk cargo test
```

Expected:

```text
PASS
PASS
PASS
PASS
PASS
PASS: full test suite green
```

- [ ] **Step 4: Record the final cleanup commit**

```bash
rtk git add src/jsonl.rs src/jsonl/builder.rs src/scene/node.rs src/scene/time.rs src/element/resolve.rs src/inspect.rs src/runtime/preflight.rs src/render.rs
rtk git add src/scene/mod.rs src/lib.rs json/the-boys-layer-caption-15s.jsonl opencat.md opencat.zh.md
rtk git commit -m "refactor: unify div and timeline node model"
```

## Self-Review Checklist

- Spec coverage:
  - Explicit `tl` node: Task 1, Task 2, Task 5
  - `transition.parentId`: Task 1, Task 2
  - Delete `layer`: Task 1, Task 3, Task 5, Task 6
  - Keep Rust `timeline()` API unchanged: Task 2, Task 3, Task 6
  - `caption` as special text with inherited time context: Task 4, Task 5
  - No old JSONL compatibility: Task 1, Task 5, Task 6
- Placeholder scan:
  - Verified: no placeholder markers remain in task steps or code snippets.
- Type consistency:
  - Use `type: "tl"` in JSONL, `transition.parentId` for transition scope, and `ScriptFrameCtx.current_frame` for caption-local time throughout all tasks.

Plan complete and saved to `docs/superpowers/plans/2026-04-21-div-timeline-unification.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
