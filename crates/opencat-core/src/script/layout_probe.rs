//! Absolute element geometry for the script engine.
//!
//! `getBoundingClientRect` must report a viewport-relative rect (CSSOM View
//! §7.2.1). The style store only knows declared `left`/`top`, and an inline
//! text element declares neither — its position comes from flex layout. So the
//! answer has to be produced by a layout pass, and scripts run *before* the
//! frame's layout pass exists.
//!
//! This module breaks that cycle by running one on-demand resolve+layout for
//! the scope's own subtree. The pass must not re-run the scope's scripts: it
//! uses [`PrecomputedScriptHost`], whose `run_frame` applies no mutations, so
//! the measured tree is the "raw" one the browser measures in `getTitleOffset`.
//!
//! The probe is a scoped thread-local (the same shape as
//! [`crate::text::scope_font_db`]) because the JS binding can only reach a
//! `&MutationStore`, and the store cannot own the resolver state.

use std::cell::{Cell, RefCell};

use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::parse::node::Node;
use crate::script::precomputed_host::PrecomputedScriptHost;

/// Viewport-space rect of an element, as CSSOM View reports it.
///
/// Padding-relative units (`em`, `%`) would need the padding box, which the
/// layout tree does not expose; they read `0` here.
#[derive(Clone, Debug)]
pub struct AbsBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

struct ProbeScope {
    boxes: RefCell<std::collections::HashMap<String, AbsBox>>,
}

thread_local! {
    static LIVE: RefCell<Vec<ProbeScope>> = const { RefCell::new(Vec::new()) };
    /// A nested resolve re-enters `scope_boxes`; only the outermost may measure.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Publish `boxes` to the scripts of this scope for the duration of `f`.
pub(crate) fn with_footprint<R>(
    boxes: std::collections::HashMap<String, AbsBox>,
    f: impl FnOnce() -> R,
) -> R {
    if let Some(Ok(json)) = std::env::var_os("OPENCAT_DUMP_LAYOUT").map(|_| {
        serde_json::to_string(
            &boxes
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::json!([v.x, v.y, v.width, v.height])))
                .collect::<std::collections::BTreeMap<_, _>>(),
        )
    }) {
        let _ = std::fs::create_dir_all("/tmp/opencat-layout");
        let _ = std::fs::write("/tmp/opencat-layout/boxes.json", json);
    }
    LIVE.with(|live| live.borrow_mut().push(ProbeScope { boxes: RefCell::new(boxes) }));
    let out = f();
    LIVE.with(|live| {
        live.borrow_mut().pop();
    });
    out
}

pub(crate) fn box_for(id: &str) -> Option<AbsBox> {
    LIVE.with(|live| live.borrow().last()?.boxes.borrow().get(id).cloned())
}

/// Does this source ever ask for geometry? Under-reporting only makes the
/// platform API return a declared-style box; the scan exists so compositions
/// that never measure pay no second layout pass.
pub(crate) fn wants_geometry(source: &str) -> bool {
    const NEEDLES: [&str; 5] = [
        "getBoundingClientRect",
        "offsetWidth",
        "offsetHeight",
        "offsetLeft",
        "offsetTop",
    ];
    NEEDLES.iter().any(|needle| source.contains(needle))
}

/// Walk the subtree and record the absolute rect of every style id.
///
/// `LayoutNode::rect` is parent-relative; CSSOM View is viewport-relative, so
/// the accumulation is the whole point of this walk.
fn collect(
    node: &crate::layout::tree::LayoutNode,
    origin: (f32, f32),
    out: &mut std::collections::HashMap<String, AbsBox>,
) {
    let x = origin.0 + node.rect.x;
    let y = origin.1 + node.rect.y;
    if !node.id.is_empty() {
        out.insert(
            node.id.clone(),
            AbsBox { x, y, width: node.rect.width, height: node.rect.height },
        );
    }
    for child in &node.children {
        collect(child, (x, y), out);
    }
}

/// Resolve `node`'s subtree with a no-op host and lay it out.
pub(crate) fn scope_boxes(
    node: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    assets: &mut crate::probe::PreparedResourceCatalog,
) -> Option<std::collections::HashMap<String, AbsBox>> {
    if DEPTH.with(|depth| depth.get()) > 0 {
        return None;
    }
    let fonts = crate::text::script_font_db();
    let mut host = PrecomputedScriptHost::new();
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    let resolved = crate::text::scope_font_db(&fonts, || {
        crate::resolve::resolve::resolve_ui_tree_with_script_cache(
            node,
            frame_ctx,
            script_frame_ctx,
            assets,
            None,
            &mut host,
        )
    });
    DEPTH.with(|depth| depth.set(depth.get() - 1));
    let element = resolved.ok()?;
    let provider = crate::text::DefaultFontProvider::from_arc(fonts);
    let mut session = crate::layout::LayoutSession::new();
    let (tree, _) = session
        .compute_layout_with_provider(&element, frame_ctx, &provider)
        .ok()?;
    let mut out = std::collections::HashMap::default();
    collect(&tree.root, (0.0, 0.0), &mut out);
    Some(out)
}


