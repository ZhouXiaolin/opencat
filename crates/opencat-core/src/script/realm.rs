//! One script realm per pipeline.
//!
//! Converges the former per-source runners and caches
//! split into a single core scheduler:
//!
//! - one [`JsContext`] (engine QuickJS isolate or web realm backend)
//! - many drivers installed as named globals, sharing that realm's state
//! - unified frame context, binding dispatch, mutation snapshot, run order
//!
//! Hosts only implement [`JsContext`] primitives. Correctness must not depend on
//! rebinding a shared `globalThis` across pipelines.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde_json::json;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::dispatch::{binding_shim_js, dispatch_binding};
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::runtime::{ANIMATION_RUNTIME, CANVAS_API_RUNTIME, NODE_STYLE_RUNTIME};
use crate::script::{
    driver_id_from_source, ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource,
};

/// Pipeline-owned script realm: one JS context, many drivers, shared state.
pub struct ScriptRealm<C: JsContext> {
    ctx: C,
    runtime_installed: bool,
    /// Drivers whose source has been installed as `globalThis.__opencatDriver_<id>`.
    installed: HashMap<u64, ()>,
    text_sources: HashMap<String, ScriptTextSource>,
    target_registry: Option<ScriptTargetRegistry>,
    /// Tracks the last frame number that had global cleanup applied, so
    /// cross-frame mutable global state cannot accumulate. All drivers within
    /// the same frame number share globals; crossing a frame boundary deletes
    /// any non-system global keys added by user scripts in the prior frame.
    last_cleanup_frame: Option<u32>,
}

impl<C: JsContext> ScriptRealm<C> {
    pub fn new(ctx: C) -> Result<Self> {
        Ok(Self {
            ctx,
            runtime_installed: false,
            installed: HashMap::new(),
            text_sources: HashMap::new(),
            target_registry: None,
            last_cleanup_frame: None,
        })
    }

    /// Build a realm from a freshly constructed backend context.
    pub fn open() -> Result<Self> {
        Self::new(C::new()?)
    }

    pub fn ctx(&self) -> &C {
        &self.ctx
    }

    fn ensure_runtime(&mut self) -> Result<()> {
        if self.runtime_installed {
            return Ok(());
        }

        // Realm-local bootstrap. Backend `JsContext` implementations must keep
        // these symbols private to this context (QuickJS isolate / web realm),
        // not rely on clobbering a process-wide globalThis for multi-pipeline
        // correctness.
        self.ctx.eval(
            "globalThis.ctx = globalThis.ctx || {\
             frame:0, fps:0, time:0, duration:0, totalDuration:0, currentTime:0, sceneDuration:0, totalFrames:0, currentFrame:0, sceneFrames:0, \
             __currentCanvasTarget:'',\
             __targetRegistry:{visual:Object.create(null),canvas:Object.create(null),nonVisual:Object.create(null)}\
         };\
         globalThis.__opencatDrivers = globalThis.__opencatDrivers || Object.create(null);",
        )?;
        self.ctx.install_dispatcher(dispatch_binding)?;
        self.ctx.eval(&binding_shim_js())?;
        self.ctx.eval(NODE_STYLE_RUNTIME)?;
        self.ctx.eval(CANVAS_API_RUNTIME)?;
        self.ctx.eval(ANIMATION_RUNTIME)?;
        self.ctx.eval(
            "globalThis.__opencatFlushTimelines = function() {\
             if (globalThis.ctx && globalThis.ctx.__flushTimelines) \
             globalThis.ctx.__flushTimelines();\
         };",
        )?;
        if let Some(registry) = &self.target_registry {
            apply_target_registry(&self.ctx, registry)?;
        }
        // Record the baseline system keys after all runtime components are
        // installed. Every subsequent install() adds its driver function to
        // this allowlist so frame-boundary cleanup never removes runtime or
        // driver globals — only user-created cross-frame state.
        self.record_system_keys()?;
        self.runtime_installed = true;
        Ok(())
    }

    fn apply_frame_ctx(
        &self,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
    ) -> Result<()> {
        self.ctx
            .with_store_mut(|s| s.reset_for_frame(frame_ctx.current_frame, frame_ctx.fps));

        // Text sources are realm-scoped and reapplied every run so callers can
        // clear/register between nodes without leaking into other pipelines.
        self.ctx.with_store_mut(|s| {
            s.clear_text_sources();
            for (id, src) in &self.text_sources {
                s.register_text_source(id, src.clone());
            }
        });

        self.ctx
            .set_ctx_field("frame", json!(frame_ctx.frame as i64))?;
        self.ctx.set_ctx_field("fps", json!(frame_ctx.fps as i64))?;
        self.ctx.set_ctx_field("time", json!(frame_ctx.time_secs))?;
        self.ctx
            .set_ctx_field("duration", json!(frame_ctx.total_duration_secs))?;
        self.ctx
            .set_ctx_field("totalDuration", json!(frame_ctx.total_duration_secs))?;
        self.ctx
            .set_ctx_field("currentTime", json!(frame_ctx.current_time_secs))?;
        self.ctx
            .set_ctx_field("sceneDuration", json!(frame_ctx.scene_duration_secs))?;
        self.ctx
            .set_ctx_field("totalFrames", json!(frame_ctx.total_frames as i64))?;
        self.ctx
            .set_ctx_field("currentFrame", json!(frame_ctx.current_frame as i64))?;
        self.ctx
            .set_ctx_field("sceneFrames", json!(frame_ctx.scene_frames as i64))?;
        self.ctx.set_ctx_field(
            "__currentCanvasTarget",
            json!(current_node_id.unwrap_or("")),
        )?;

        if let Some(registry) = &self.target_registry {
            apply_target_registry(&self.ctx, registry)?;
        }

        Ok(())
    }

    fn driver_fn_name(id: ScriptDriverId) -> String {
        // u64 decimal keeps the identifier a valid JS property name without hex
        // prefixes that some engines treat specially.
        format!("__opencatDriver_{}", id.0)
    }

    /// Snapshot current own-property names of `globalThis` as the baseline
    /// system-key allowlist. Uses `getOwnPropertyNames` (not `keys`) so
    /// non-enumerable but non-configurable built-in globals are captured and
    /// never targeted by frame-boundary cleanup.
    ///
    /// Must be called once after runtime installation is complete, and again
    /// after each `install()` so driver functions are protected.
    fn record_system_keys(&self) -> Result<()> {
        self.ctx.eval(
            "var __k = Object.getOwnPropertyNames(globalThis);\
             __k.push('__opencatSystemKeys');\
             globalThis.__opencatSystemKeys = __k;",
        )
    }

    /// Add a single key to the system-key allowlist (e.g. a newly installed
    /// driver function). Called from `install()` to prevent frame-boundary
    /// cleanup from removing the driver's global function.
    fn add_system_key(&self, key: &str) -> Result<()> {
        let js = format!("globalThis.__opencatSystemKeys.push('{key}');");
        self.ctx.eval(&js)
    }

    /// Delete every own-property on `globalThis` that is not in the system-key
    /// allowlist. This prevents user scripts from accumulating mutable state on
    /// the global object across frame boundaries. Drivers within the same frame
    /// (same `current_frame`) still share globals — only frame transitions
    /// trigger cleanup, preserving intra-frame cross-driver communication.
    ///
    /// Uses `getOwnPropertyNames` (consistent with `record_system_keys`) and
    /// only attempts deletion on configurable properties, so built-in
    /// non-configurable globals are never touched.
    fn cleanup_globals(&self) -> Result<()> {
        self.ctx.eval(
            "var __sk = globalThis.__opencatSystemKeys;\
             if (__sk) {\
               __sk = __sk.slice();\
               __sk.push('__opencatSystemKeys');\
               var __keys = Object.getOwnPropertyNames(globalThis);\
               for (var __i = 0; __i < __keys.length; __i++) {\
                 if (__sk.indexOf(__keys[__i]) < 0) {\
                   var __desc = Object.getOwnPropertyDescriptor(globalThis, __keys[__i]);\
                   if (__desc && __desc.configurable) {\
                     delete globalThis[__keys[__i]];\
                   }\
                 }\
               }\
             }",
        )
    }
}

impl<C: JsContext> ScriptHost for ScriptRealm<C> {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        self.ensure_runtime()?;
        let id = driver_id_from_source(source);
        let fn_name = Self::driver_fn_name(id);
        let is_new = matches!(self.installed.entry(id.0), std::collections::hash_map::Entry::Vacant(_));
        if let std::collections::hash_map::Entry::Vacant(e) = self.installed.entry(id.0) {
            // Install once; subsequent installs of the same source are no-ops so
            // realm-local JS state set by prior frames is preserved.
            let run_fn = format!("globalThis.{fn_name} = function() {{\n{source}\n}};");
            self.ctx.eval(&run_fn)?;
            // Also keep a stable alias used only for the "last installed" debug
            // surface; production dispatch always uses the per-driver name.
            self.ctx.eval(&format!(
                "globalThis.__opencatDrivers['{}'] = globalThis.{fn_name};",
                id.0
            ))?;
            e.insert(());
        }
        // Register the driver function as a system key so frame-boundary cleanup
        // preserves it. Done after the entry scope to avoid mutable borrow conflict
        // with self.installed. Re-registration (no-op) is harmless — push adds a
        // duplicate that indexOf still matches.
        if is_new {
            self.add_system_key(&fn_name)?;
        }
        Ok(id)
    }

    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource) {
        self.text_sources.insert(node_id.to_string(), source);
    }

    fn clear_text_sources(&mut self) {
        self.text_sources.clear();
    }

    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
        current_node_id: Option<&str>,
        recorder: &mut dyn MutationRecorder,
    ) -> Result<()> {
        if !self.installed.contains_key(&driver.0) {
            return Err(anyhow!("script driver {} not installed in realm", driver.0));
        }
        self.ensure_runtime()?;

        // Frame-boundary global-state cleanup: when the frame number changes,
        // delete any non-system globalThis keys added by user scripts in the
        // previous frame. Drivers within the same frame (e.g. scene + node
        // drivers) share globals; only crossing a frame number triggers cleanup.
        let frame = frame_ctx.current_frame;
        if self.last_cleanup_frame != Some(frame) {
            self.cleanup_globals()?;
            self.last_cleanup_frame = Some(frame);
        }

        self.apply_frame_ctx(frame_ctx, current_node_id)?;

        let fn_name = Self::driver_fn_name(driver);
        self.ctx.call_global_fn(&fn_name)?;
        self.ctx.call_global_fn("__opencatFlushTimelines")?;

        let snap = self.ctx.with_store_mut(|s| s.snapshot_mutations());
        snap.apply_to_recorder(recorder);
        Ok(())
    }

    fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.target_registry = Some(registry);
    }

    fn set_style_defaults(
        &mut self,
        defaults: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) {
        if let Err(err) = self.ensure_runtime() {
            // Style defaults are best-effort seeding; a missing runtime is
            // reported on the next install/run_frame.
            let _ = err;
            return;
        }
        self.ctx.with_store_mut(|s| {
            for (id, props) in defaults {
                for (prop, val) in props {
                    s.set_initial_style(id, prop, val.clone());
                }
            }
        });
    }

    fn set_initial_style_from_node(&mut self, id: &str, style: &crate::style::NodeStyle) {
        if self.ensure_runtime().is_err() {
            return;
        }
        self.ctx.with_store_mut(|s| {
            s.set_initial_style_from_node(id, style);
        });
    }
}

pub fn apply_target_registry<C: JsContext>(
    ctx: &C,
    registry: &ScriptTargetRegistry,
) -> Result<()> {
    let visual: serde_json::Map<String, serde_json::Value> = registry
        .visual_ids
        .iter()
        .map(|k| (k.clone(), serde_json::Value::Bool(true)))
        .collect();
    let canvas: serde_json::Map<String, serde_json::Value> = registry
        .canvas_ids
        .iter()
        .map(|k| (k.clone(), serde_json::Value::Bool(true)))
        .collect();
    let non_visual: serde_json::Map<String, serde_json::Value> = registry
        .non_visual_ids
        .iter()
        .map(|k| (k.clone(), serde_json::Value::Bool(true)))
        .collect();
    ctx.set_ctx_field(
        "__targetRegistry",
        json!({
            "visual": visual,
            "canvas": canvas,
            "nonVisual": non_visual,
        }),
    )
}
