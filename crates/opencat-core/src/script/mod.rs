//! Shared JS script runtime files.
//!
//! These JS files implement the user-facing animation and style API
//! (`ctx.getNode()`, `ctx.animate()`, `ctx.canvas()`, etc.) and are
//! evaluated by both the desktop engine (via QuickJS) and the web
//! frontend (via the browser's native JS engine).
//!
//! The files call into platform-specific "native" functions
//! (`__record_*`, `__canvas_*`, `__animate_*`) which are provided
//! by the host environment.

pub mod runtime;
