//! JS runtime source files used by both engine and web.
//!
//! These are evaluated in-order by the script host to set up the
//! user-facing API (`ctx.getNode()`, `ctx.animate()`, etc.).
//!
//! ## Evaluation order
//! 1. `ANIMATION_BOOTSTRAP` — global `__opencatAnimation` + plugin registry
//! 2. `ANIMATION_CORE` — tween engine, property interpolation
//! 3. Core property files (style_props, color, filter) — built-in animatable properties
//! 4. Core utility (utils) — `ctx.utils.*` helpers
//! 5. Plugin files (text, scramble_text, split_text, motion_path, morph_svg) — extension plugins
//! 6. `ANIMATION_FACADE` — `ctx.from/to/fromTo/set()`, `ctx.timeline()`, `flushTimelines`
//! 7. `NODE_STYLE_RUNTIME` — `ctx.getNode(id).prop(val)` chainable API + DOM property facade
//! 8. `CANVAS_API_RUNTIME` — `ctx.canvas(id).fillRect(...)` CanvasKit subset
//! 9. `THREE_API_RUNTIME` — `ctx.three` three.js 风格 3D 场景图 + `Renderer.render()`
//! 10. `G2_BUNDLE` + `G2_API_RUNTIME` — `useChart(ref, build)` AntV G2 图表，
//! 11. `DOM_API_RUNTIME` — `document`/`window` 全局与越界 API 报错
//!
//! All files are IIFE-wrapped and assume the existence of native
//! `__record_*`, `__canvas_*`, `__animate_*`, `__text_source_*`
//! global functions provided by the host environment.

pub const NODE_STYLE_RUNTIME: &str = include_str!("node_style.js");
pub const CANVAS_API_RUNTIME: &str = include_str!("canvas_api.js");
pub const THREE_API_RUNTIME: &str = include_str!("three_api.js");
// DOM globals (`document`/`window`/timer guards). Loaded after
// `NODE_STYLE_RUNTIME` (getElementById hands out the same node handles) and
// after `JS_PLATFORM_SHIM` in the realm bootstrap (the G2 rAF polyfill must
// claim `requestAnimationFrame` first; the guard here only fills the gap).
pub const DOM_API_RUNTIME: &str = include_str!("dom_api.js");
// 2D-context adapter for `node.getContext('2d')` (see context2d.js header).
// Loaded after `NODE_STYLE_RUNTIME` — the node facade's `getContext` branch
// calls `ctx.getContext` defined here.
pub const CONTEXT2D_RUNTIME: &str = include_str!("context2d.js");
// AntV G2 chart runtime. `G2_BUNDLE` is the built @antv/g2 +
// @antv/g-canvaskit IIFE (built by the same esbuild chain as g2-rs);
// `G2_API_RUNTIME` is the host side that binds G2 to an OpenCat <canvas>
// through the existing `__canvas_*` bindings.
pub const G2_BUNDLE: &str = include_str!("g2/g2.bundle.js");
pub const G2_API_RUNTIME: &str = include_str!("g2/g2_api.js");

/// Real gsap package (npm `gsap`, core + CSSPlugin), bundled no-window build.
/// The package self-detects the missing DOM and never engages rAF/ticker;
/// timelines are seek-driven (`paused` + `tl.time`). Evaluated before
/// `JSX_RUNTIME`, whose module registry exposes it as the `gsap` builtin.
pub const GSAP_BUNDLE: &str = include_str!("gsap/gsap.bundle.js");

// JSX frontend (`parse::jsx`): element factory + `use*` registry + module
// loader, installed in every script realm (the per-frame driver produced by
// `parse::jsx` depends on them). The JSX->h() transform lives in Rust
// (`parse::jsx_transform`, oxc); the old JS scanner is retired.
pub const JSX_RUNTIME: &str = include_str!("jsx_runtime.js");

pub const ANIMATION_BOOTSTRAP: &str = include_str!("animation/bootstrap.js");
pub const ANIMATION_CORE: &str = include_str!("animation/core.js");
pub const ANIMATION_FACADE: &str = include_str!("animation/facade.js");

// Core property files — built-in animatable properties, not plugins.
// These register properties directly via `animation.registerProperty()`.
pub const CORE_STYLE_PROPS: &str = include_str!("animation/properties/style_props.js");
pub const CORE_COLOR: &str = include_str!("animation/properties/color.js");
pub const CORE_FILTER: &str = include_str!("animation/properties/filter.js");
pub const CORE_UTILS: &str = include_str!("animation/properties/utils.js");

// Plugin files — extension plugins registered via `animation.registerPlugin()`.
pub const PLUGIN_TEXT: &str = include_str!("animation/plugins/text.js");
pub const PLUGIN_SCRAMBLE_TEXT: &str = include_str!("animation/plugins/scramble_text.js");
pub const PLUGIN_SPLIT_TEXT: &str = include_str!("animation/plugins/split_text.js");
pub const PLUGIN_MOTION_PATH: &str = include_str!("animation/plugins/motion_path.js");
pub const PLUGIN_MORPH_SVG: &str = include_str!("animation/plugins/morph_svg.js");

/// Core property + utility sources in the canonical load order.
pub const CORE_RUNTIMES: &[&str] = &[CORE_STYLE_PROPS, CORE_COLOR, CORE_FILTER, CORE_UTILS];

/// Plugin runtime sources in the canonical load order.
pub const PLUGIN_RUNTIMES: &[&str] = &[
    PLUGIN_TEXT,
    PLUGIN_SCRAMBLE_TEXT,
    PLUGIN_SPLIT_TEXT,
    PLUGIN_MOTION_PATH,
    PLUGIN_MORPH_SVG,
];

/// Complete animation runtime: bootstrap → core → core properties → plugins → facade
pub const ANIMATION_RUNTIME_PARTS: &[&str] = &[
    ANIMATION_BOOTSTRAP,
    ANIMATION_CORE,
    CORE_STYLE_PROPS,
    CORE_COLOR,
    CORE_FILTER,
    CORE_UTILS,
    PLUGIN_TEXT,
    PLUGIN_SCRAMBLE_TEXT,
    PLUGIN_SPLIT_TEXT,
    PLUGIN_MOTION_PATH,
    PLUGIN_MORPH_SVG,
    ANIMATION_FACADE,
];

/// Pre-concatenated animation runtime, ready for `eval()`.
pub const ANIMATION_RUNTIME: &str = concat!(
    include_str!("animation/bootstrap.js"),
    "\n",
    include_str!("animation/core.js"),
    "\n",
    include_str!("animation/properties/style_props.js"),
    "\n",
    include_str!("animation/properties/color.js"),
    "\n",
    include_str!("animation/properties/filter.js"),
    "\n",
    include_str!("animation/properties/utils.js"),
    "\n",
    include_str!("animation/plugins/text.js"),
    "\n",
    include_str!("animation/plugins/scramble_text.js"),
    "\n",
    include_str!("animation/plugins/split_text.js"),
    "\n",
    include_str!("animation/plugins/motion_path.js"),
    "\n",
    include_str!("animation/plugins/morph_svg.js"),
    "\n",
    include_str!("animation/facade.js"),
);
