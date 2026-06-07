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
//! 7. `NODE_STYLE_RUNTIME` — `ctx.getNode(id).prop(val)` chainable API
//! 8. `CANVAS_API_RUNTIME` — `ctx.canvas(id).fillRect(...)` CanvasKit subset
//!
//! All files are IIFE-wrapped and assume the existence of native
//! `__record_*`, `__canvas_*`, `__animate_*`, `__text_source_*`
//! global functions provided by the host environment.

pub const NODE_STYLE_RUNTIME: &str = include_str!("node_style.js");
pub const CANVAS_API_RUNTIME: &str = include_str!("canvas_api.js");

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
