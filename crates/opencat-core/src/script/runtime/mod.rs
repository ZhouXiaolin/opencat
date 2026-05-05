//! JS runtime source files used by both engine and web.
//!
//! These are evaluated in-order by the script host to set up the
//! user-facing API (`ctx.getNode()`, `ctx.animate()`, etc.).
//!
//! ## Evaluation order
//! 1. `ANIMATION_BOOTSTRAP` — global `__opencatAnimation` + plugin registry
//! 2. `ANIMATION_CORE` — timeline engine, property interpolation
//! 3. Plugin files (style_props, color, text, split_text, motion_path, morph_svg, utils)
//! 4. `ANIMATION_FACADE` — `ctx.set()`, `ctx.animate()`, `ctx.timeline()`, `flushTimelines()`
//! 5. `NODE_STYLE_RUNTIME` — `ctx.getNode(id).prop(val)` chainable API
//! 6. `CANVAS_API_RUNTIME` — `ctx.canvas(id).fillRect(...)` CanvasKit subset
//!
//! All files are IIFE-wrapped and assume the existence of native
//! `__record_*`, `__canvas_*`, `__animate_*`, `__text_source_*`
//! global functions provided by the host environment.

pub const NODE_STYLE_RUNTIME: &str = include_str!("node_style.js");
pub const CANVAS_API_RUNTIME: &str = include_str!("canvas_api.js");

pub const ANIMATION_BOOTSTRAP: &str = include_str!("animation/bootstrap.js");
pub const ANIMATION_CORE: &str = include_str!("animation/core.js");
pub const ANIMATION_FACADE: &str = include_str!("animation/facade.js");

// Plugin files (loaded after bootstrap + core, before facade)
pub const PLUGIN_STYLE_PROPS: &str = include_str!("animation/plugins/style_props.js");
pub const PLUGIN_COLOR: &str = include_str!("animation/plugins/color.js");
pub const PLUGIN_TEXT: &str = include_str!("animation/plugins/text.js");
pub const PLUGIN_SPLIT_TEXT: &str = include_str!("animation/plugins/split_text.js");
pub const PLUGIN_MOTION_PATH: &str = include_str!("animation/plugins/motion_path.js");
pub const PLUGIN_MORPH_SVG: &str = include_str!("animation/plugins/morph_svg.js");
pub const PLUGIN_UTILS: &str = include_str!("animation/plugins/utils.js");

/// All plugin runtime sources in the canonical load order.
pub const PLUGIN_RUNTIMES: &[&str] = &[
    PLUGIN_STYLE_PROPS,
    PLUGIN_COLOR,
    PLUGIN_TEXT,
    PLUGIN_SPLIT_TEXT,
    PLUGIN_MOTION_PATH,
    PLUGIN_MORPH_SVG,
    PLUGIN_UTILS,
];

/// Complete animation runtime: bootstrap → core → plugins → facade
pub const ANIMATION_RUNTIME_PARTS: &[&str] = &[
    ANIMATION_BOOTSTRAP,
    ANIMATION_CORE,
    PLUGIN_STYLE_PROPS,
    PLUGIN_COLOR,
    PLUGIN_TEXT,
    PLUGIN_SPLIT_TEXT,
    PLUGIN_MOTION_PATH,
    PLUGIN_MORPH_SVG,
    PLUGIN_UTILS,
    ANIMATION_FACADE,
];

/// Pre-concatenated animation runtime, ready for `eval()`.
pub const ANIMATION_RUNTIME: &str = concat!(
    include_str!("animation/bootstrap.js"), "\n",
    include_str!("animation/core.js"), "\n",
    include_str!("animation/plugins/style_props.js"), "\n",
    include_str!("animation/plugins/color.js"), "\n",
    include_str!("animation/plugins/text.js"), "\n",
    include_str!("animation/plugins/split_text.js"), "\n",
    include_str!("animation/plugins/motion_path.js"), "\n",
    include_str!("animation/plugins/morph_svg.js"), "\n",
    include_str!("animation/plugins/utils.js"), "\n",
    include_str!("animation/facade.js"),
);
