//! Absolute element geometry for the script engine.
//!
//! `getBoundingClientRect` must report a viewport-relative rect (CSSOM View
//! §7.2.1). The style store only knows declared `left`/`top`, and an inline
//! text element declares neither — its position comes from flex layout. So the
//! answer has to be produced by a layout pass, and scripts run *before* the
//! frame's layout pass exists.
//!
//! This module breaks that cycle with a synchronous layout, as a browser does:
//! the first geometry read of a scope resolves+lays out the scope's own subtree
//! with the mutations the script has written so far this frame, and a later
//! text write invalidates it. `getTitleOffset` writes "HTML" into an empty span
//! and then measures it; the typing callbacks read `offsetWidth` right after
//! each letter. The pass must not re-run the scope's scripts: it uses
//! [`PrecomputedScriptHost`], which only replays the store snapshot.
//!
//! The probe is a scoped thread-local (the same shape as
//! [`crate::text::scope_font_db`]) because the JS binding can only reach a
//! `&MutationStore`, and the store cannot own the resolver state.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::parse::node::Node;
use crate::probe::PreparedResourceCatalog;
use crate::script::StyleMutations;
use crate::script::precomputed_host::PrecomputedScriptHost;
use crate::script::recorder::MutationStore;

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

/// Inputs of one scope's synchronous layout, plus the boxes of the last pass
/// keyed by the store's text-write count.
struct ProbeScope {
    node: Node,
    frame_ctx: FrameCtx,
    script_frame_ctx: ScriptFrameCtx,
    assets: PreparedResourceCatalog,
    cache: RefCell<Option<(u64, HashMap<String, AbsBox>)>>,
}

thread_local! {
    static LIVE: RefCell<Vec<Rc<ProbeScope>>> = const { RefCell::new(Vec::new()) };
    /// A measuring pass re-enters the resolver; only the outermost scope may
    /// measure, nested ones run without a probe.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Give the scripts run inside `f` a synchronous-layout probe over `node`.
pub(crate) fn with_scope<R>(
    node: &Node,
    frame_ctx: FrameCtx,
    script_frame_ctx: ScriptFrameCtx,
    assets: PreparedResourceCatalog,
    f: impl FnOnce() -> R,
) -> R {
    if DEPTH.with(|depth| depth.get()) > 0 {
        return f();
    }
    let scope = ProbeScope {
        node: node.clone(),
        frame_ctx,
        script_frame_ctx,
        assets,
        cache: RefCell::new(None),
    };
    LIVE.with(|live| live.borrow_mut().push(Rc::new(scope)));
    let out = f();
    LIVE.with(|live| {
        live.borrow_mut().pop();
    });
    out
}

/// Box of `id` in the current scope, laid out with the store's current state.
pub(crate) fn box_for(id: &str, store: &MutationStore) -> Option<AbsBox> {
    let scope = LIVE.with(|live| live.borrow().last().cloned())?;
    let key = store.text_writes();
    let fresh = scope
        .cache
        .borrow()
        .as_ref()
        .is_some_and(|(cached, _)| *cached == key);
    if !fresh {
        let boxes = measure(&scope, store.style_snapshot())?;
        *scope.cache.borrow_mut() = Some((key, boxes));
    }
    scope
        .cache
        .borrow()
        .as_ref()
        .and_then(|(_, boxes)| boxes.get(id).cloned())
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
    out: &mut HashMap<String, AbsBox>,
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

/// Resolve the scope's subtree with `mutations` replayed and lay it out.
fn measure(scope: &ProbeScope, mutations: StyleMutations) -> Option<HashMap<String, AbsBox>> {
    let fonts = crate::text::script_font_db();
    let mut host = PrecomputedScriptHost::from_single(mutations);
    let mut assets = scope.assets.clone();
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    let resolved = crate::text::scope_font_db(&fonts, || {
        crate::resolve::resolve::resolve_ui_tree_with_script_cache(
            &scope.node,
            &scope.frame_ctx,
            &scope.script_frame_ctx,
            &mut assets,
            None,
            &mut host,
        )
    });
    DEPTH.with(|depth| depth.set(depth.get() - 1));
    let element = resolved.ok()?;
    let provider = crate::text::DefaultFontProvider::from_arc(fonts);
    let mut session = crate::layout::LayoutSession::new();
    let (tree, _) = session
        .compute_layout_with_provider(&element, &scope.frame_ctx, &provider)
        .ok()?;
    let mut out = HashMap::default();
    collect(&tree.root, (0.0, 0.0), &mut out);
    Some(out)
}
