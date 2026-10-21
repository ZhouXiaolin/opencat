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

use anyhow::{Result, anyhow};
use serde_json::json;

use crate::frame_ctx::ScriptFrameCtx;
use crate::script::dispatch::{binding_shim_js, dispatch_binding};
use crate::script::js_context::JsContext;
use crate::script::recorder::MutationRecorder;
use crate::script::runtime::{
    ANIMATION_RUNTIME, CANVAS_API_RUNTIME, CONTEXT2D_RUNTIME, DOM_API_RUNTIME, G2_API_RUNTIME,
    G2_BUNDLE, JSX_RUNTIME, NODE_STYLE_RUNTIME, THREE_API_RUNTIME,
};
use crate::script::{
    ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource, driver_id_from_source,
};

// JS platform globals V8 does not ship, needed by the G2 bundle at
// load time (TextEncoder) and by G2's font loading at render time. Public
// because the JSX extract realm (a throwaway realm built before the pipeline
// realm) needs the same `requestAnimationFrame` queue: the gsap bundle calls
// `_wake()` at eval time, which schedules one tick.
pub const JS_PLATFORM_SHIM: &str = r#"
(function () {
    if (typeof globalThis.TextEncoder === 'undefined') {
        globalThis.TextEncoder = function TextEncoder() {};
        globalThis.TextEncoder.prototype.encode = function (input) {
            var out = [];
            var text = String(input == null ? '' : input);
            for (var i = 0; i < text.length; i++) {
                var code = text.charCodeAt(i);
                if (code < 0x80) out.push(code);
                else if (code < 0x800) out.push(0xc0 | (code >> 6), 0x80 | (code & 0x3f));
                else out.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 0x3f), 0x80 | (code & 0x3f));
            }
            return new Uint8Array(out);
        };
    }
    if (typeof globalThis.TextDecoder === 'undefined') {
        globalThis.TextDecoder = function TextDecoder() {};
        globalThis.TextDecoder.prototype.decode = function (bytes) {
            if (!bytes) return '';
            var out = '';
            for (var i = 0; i < bytes.length;) {
                var b = bytes[i++];
                if (b < 0x80) out += String.fromCharCode(b);
                else if (b < 0xe0) out += String.fromCharCode(((b & 0x1f) << 6) | (bytes[i++] & 0x3f));
                else out += String.fromCharCode(((b & 0x0f) << 12) | ((bytes[i++] & 0x3f) << 6) | (bytes[i++] & 0x3f));
            }
            return out;
        };
    }
    if (typeof globalThis.Blob === 'undefined') {
        globalThis.Blob = function Blob(parts) {
            this.parts = parts || [];
            this.size = this.parts.reduce(function (sum, part) {
                return sum + (part && part.length ? part.length : 0);
            }, 0);
            this.type = '';
        };
    }
    // A synchronous frame queue. g's `Canvas` takes `requestAnimationFrame`
    // from `raf.bind(root)` at bundle-eval time (see its raf-polyfill), so the
    // implementation has to be on the global *before* the bundle runs: the
    // bundle caches the reference and re-assigns it to itself. This is the
    // same queue the G2 host runtime drains, so a queued callback runs at the
    // next frame boundary and never on a host timer.
    // The bundle's raf-polyfill resolves its `root` through `self.self === self`
    // (then `global`) at bundle-eval time and CACHES the reference. The DOM
    // shims install later, inside the first frame, so without these aliases
    // `root` lands on `{}` and the polyfill's setTimeout variant is what g's
    // Canvas captures — which never fires in this realm.
    if (typeof globalThis.self !== 'object' || globalThis.self === null) globalThis.self = globalThis;
    if (typeof globalThis.global !== 'object' || globalThis.global === null) globalThis.global = globalThis;
    if (typeof globalThis.__ocG2RafQueue !== 'object' || globalThis.__ocG2RafQueue === null) {
        globalThis.__ocG2RafQueue = [];
    }
    if (typeof globalThis.__ocG2RafNextId !== 'number') {
        globalThis.__ocG2RafNextId = 1;
    }
    globalThis.requestAnimationFrame = function (callback) {
        globalThis.__ocG2RafQueued = (globalThis.__ocG2RafQueued || 0) + 1;
        globalThis.__ocG2RafQueueLenAtRegister = globalThis.__ocG2RafQueue.length;
        globalThis.__ocG2RafQueue.push(callback);
        return globalThis.__ocG2RafNextId++;
    };
    globalThis.cancelAnimationFrame = function (id) {
        var queue = globalThis.__ocG2RafQueue;
        for (var i = 0; i < queue.length; i++) {
            if (queue[i].__ocG2RafId === id) { queue.splice(i, 1); return true; }
        }
        return false;
    };
    if (typeof globalThis.navigator !== 'object' || globalThis.navigator === null) {
        globalThis.navigator = {};
    }
    // g-plugin-dragndrop branches on this at init; the offline frame has no
    // pointer input, so the mouse path is the one that fits.
    if (typeof globalThis.navigator.msPointerEnabled !== 'boolean') {
        globalThis.navigator.msPointerEnabled = false;
    globalThis.console = globalThis.console || {
        log: function () { var k = globalThis.__ocLogSink; if (k) { k('log ' + Array.prototype.join.call(arguments, ' ')); } else { (globalThis.__ocG2Logs = globalThis.__ocG2Logs || []).push('console.log ' + Array.prototype.join.call(arguments, ' ')); } },
        warn: function () { var k = globalThis.__ocLogSink; if (k) { k('warn ' + Array.prototype.join.call(arguments, ' ')); } else { (globalThis.__ocG2Logs = globalThis.__ocG2Logs || []).push('console.warn ' + Array.prototype.join.call(arguments, ' ')); } },
        error: function () { var k = globalThis.__ocLogSink; if (k) { k('error ' + Array.prototype.join.call(arguments, ' ')); } else { (globalThis.__ocG2Logs = globalThis.__ocG2Logs || []).push('console.error ' + Array.prototype.join.call(arguments, ' ')); } },
    };
    globalThis.__ocInstallLogSink = function (fn) { globalThis.__ocLogSink = fn; };
    }
    if (typeof globalThis.navigator.userAgent !== 'string') {
        globalThis.navigator.userAgent = 'opencat';
    }
    if (typeof globalThis.URL !== 'object' || globalThis.URL === null) globalThis.URL = {};
    if (typeof globalThis.URL.createObjectURL !== 'function') {
        globalThis.URL.createObjectURL = function () { return 'blob:opencat'; };
        globalThis.URL.revokeObjectURL = function () {};
    }
})();
"#;

/// Upper bound on promise-continuation drains after a driver frame. One frame
/// of a chart rebuild resolves a handful of chained promises (fonts, the first
/// draw, the animation clock); this only has to cover that, and stops the
/// realm from spinning if a user script loops on promises.
const MICROTASK_DRAIN_PASSES: usize = 32;

/// Pipeline-owned script realm: one JS context, many drivers, shared state.
pub struct ScriptRealm<C: JsContext> {
    ctx: C,
    runtime_installed: bool,
    /// Drivers whose source has been installed as `globalThis.__opencatDriver_<id>`.
    installed: HashMap<u64, ()>,
    text_sources: HashMap<String, ScriptTextSource>,
    target_registry: Option<ScriptTargetRegistry>,
    /// 本帧已导出的 3D 场景 IR 段（`export_scene3_commits` 产出，
    /// pipeline 经 `take_scene3_commits` 取走装入 `RenderFrame.scene3`）。
    pending_scene3: Option<crate::ir::draw_frame::Scene3Section>,
    /// 文档 base 目录（宿主 = XML 所在目录）：`GLTFGeometry` 等相对路径的
    /// join 基准——three.js `GLTFLoader` 相对 URL 对 import.meta.url 解析
    /// （参考 js:40 `new URL(prismUrl, import.meta.url)`）的 opencat 对应物。
    document_base: Option<String>,
}

impl<C: JsContext> ScriptRealm<C> {
    pub fn new(ctx: C) -> Result<Self> {
        Ok(Self {
            ctx,
            runtime_installed: false,
            installed: HashMap::new(),
            text_sources: HashMap::new(),
            target_registry: None,
            pending_scene3: None,
            document_base: None,
        })
    }

    /// 注入文档 base 目录（XML 所在目录）：脚本内相对资产路径
    /// （`GLTFGeometry` 等）对它 join。见 [`Self::document_base`]。
    pub fn set_document_base(&mut self, base: &str) {
        self.document_base = Some(base.to_string());
        // runtime 已装则立即生效；未装则 ensure_runtime 时统一注入。
        if self.runtime_installed {
            let _ = self.ctx.set_ctx_field("__documentBase", json!(base));
        }
    }

    /// Realm 级 3D 几何注册表快照（Arc 克隆）：`RenderFrame.scene3` 里
    /// `Geometry3::Asset { id }` 的解析来源。注册表随 pipeline 常驻
    /// （不逐帧清空），web 宿主可按 id 懒取。
    pub fn geometry_registry(&mut self) -> crate::ir::scene3::GeometryRegistry {
        self.ctx.with_store_mut(|s| s.geometry_registry())
    }

    /// Build a realm from a freshly constructed backend context.
    pub fn open() -> Result<Self> {
        Self::new(C::new()?)
    }

    /// Evaluate an expression in this realm's context and return it as JSON.
    ///
    /// Test and diagnostic surface; the trait's default returns `Null` for
    /// backends without a probe path.
    pub fn eval_json_for_test(&self, code: &str) -> serde_json::Value {
        self.ctx.eval_json(code).unwrap_or(serde_json::Value::Null)
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
             global:{time:0, duration:0, frame:0}, current:{time:0, duration:0, frame:0}, \
             __currentCanvasTarget:'',\
             __targetRegistry:{visual:Object.create(null),canvas:Object.create(null),nonVisual:Object.create(null)}\
         };\
         globalThis.__opencatDrivers = globalThis.__opencatDrivers || Object.create(null);",
        )?;
        self.ctx.install_dispatcher(dispatch_binding)?;
        self.ctx.eval(&binding_shim_js())?;

        // Override Math.random with a seeded deterministic version sourced
        // from the scene-local frame number (set via apply_frame_ctx before
        // user code runs). This ensures cross-frame determinism — same frame
        // index always produces the same random sequence — and, because the
        // seed is scene-local, a scene's sequence does not change when the
        // scene moves on the timeline.
        // Must be done AFTER the binding shim is installed so
        // __util_random_seeded is available.
        self.ctx.eval(
            "var __opencatOriginalRandom = Math.random;\
             Math.random = function() {\
               var seed = (typeof ctx !== 'undefined' && ctx && ctx.current) ? Number(ctx.current.frame || 0) : 0;\
               return __util_random_seeded(seed + __opencatRandomCallIndex++);\
             };\
             globalThis.__opencatRandomCallIndex = 0;",
        )?;

        // Disable real clock: Date.now always returns 0 so scripts cannot
        // depend on wall-clock time. The only time a script should read is
        // ctx.global / ctx.current, both sourced from FrameCtx.
        self.ctx.eval(
            "var _Date = Date;\
             Date = function() { return new _Date(0); };\
             Date.now = function() { return 0; };",
        )?;

        self.ctx.eval(NODE_STYLE_RUNTIME)?;
        self.ctx.eval(CANVAS_API_RUNTIME)?;
        self.ctx.eval(THREE_API_RUNTIME)?;
        // G2 charts. The bundle is a browser IIFE: it needs the JS-platform
        // globals first and it must see the CanvasKit facade
        // `CANVAS_API_RUNTIME` installs — both are satisfied here.
        self.ctx.eval(JS_PLATFORM_SHIM)?;
        self.ctx.eval(G2_BUNDLE)?;
        self.ctx.eval(G2_API_RUNTIME)?;
        // DOM globals: `document`/`window` + timer guards. After the G2 shim
        // (its rAF polyfill owns `requestAnimationFrame`; the guard only
        // fills the gap) and after NODE_STYLE_RUNTIME (getElementById hands
        // out the same node handles).
        self.ctx.eval(DOM_API_RUNTIME)?;
        // 2D-context adapter (`node.getContext('2d')`): after NODE_STYLE_RUNTIME,
        // whose `getContext` branch forwards here.
        self.ctx.eval(CONTEXT2D_RUNTIME)?;
        self.ctx.eval(ANIMATION_RUNTIME)?;
        // JSX frontend: h() + use* registry + module loader. Pure-XML
        // compositions simply never call into it. The `gsap` package is NOT
        // installed here: it is project-supplied (dist/gsap.bundle.mjs) and
        // the per-composition driver injects it into __ocModuleSources.
        self.ctx.eval(JSX_RUNTIME)?;
        self.ctx.eval(
            "globalThis.__opencatFlushTimelines = function() {\
             if (globalThis.ctx && globalThis.ctx.__flushTimelines) \
             globalThis.ctx.__flushTimelines();\
         };",
        )?;
        if let Some(registry) = &self.target_registry {
            apply_target_registry(&self.ctx, registry)?;
        }
        if let Some(base) = &self.document_base {
            self.ctx.set_ctx_field("__documentBase", json!(base))?;
        }
        // Debug escape hatch: OC_DBG_NAN=1 arms the NaN guards in
        // animation/core.js (createTween / __animate_value inputs). Unset on
        // web (std::env::var is always Err there) and zero-cost when unset.
        if matches!(std::env::var("OC_DBG_NAN"), Ok(v) if v == "1") {
            self.ctx.eval("globalThis.__OC_DBG_NAN = true;")?;
        }
        // A-B timing / diagnostics escape hatch for the G2 static-build cache
        // (g2_api.js reads it before each chart frame).
        if matches!(std::env::var("OC_NO_G2_CACHE"), Ok(v) if v == "1") {
            self.ctx.eval("globalThis.OC_NO_G2_CACHE = '1';")?;
        }
        // User-created globalThis keys persist for the realm lifetime: there
        // is no frame-boundary cleanup. The realm lives and dies with one
        // pipeline, so session-level state is bounded by the composition, and
        // user scripts can memoize deterministic tables across frames instead
        // of recomputing them every frame.
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

        // Reset the Math.random call index so each frame starts with the same
        // random sequence at the same frame number.
        self.ctx.eval("globalThis.__opencatRandomCallIndex = 0;")?;

        // Text sources are realm-scoped and reapplied every run so callers can
        // clear/register between nodes without leaking into other pipelines.
        self.ctx.with_store_mut(|s| {
            s.clear_text_sources();
            for (id, src) in &self.text_sources {
                s.register_text_source(id, src.clone());
            }
        });

        // Two clocks, two namespaces. `ctx.global` is the whole composition;
        // `ctx.current` is the scene this script belongs to. Outside a timeline
        // the two are equal. JSX hooks overwrite `ctx.current` per hook (see
        // jsx_runtime.js) so a hook always reads its own scene's clock even on
        // a frame that renders a different scene.
        self.ctx.set_ctx_field(
            "global",
            json!({
                "time": frame_ctx.time_secs,
                "duration": frame_ctx.total_duration_secs,
                "frame": frame_ctx.frame as i64,
                "width": frame_ctx.width,
                "height": frame_ctx.height,
            }),
        )?;
        self.ctx.set_ctx_field(
            "current",
            json!({
                "time": frame_ctx.current_time_secs,
                "duration": frame_ctx.scene_duration_secs,
                "frame": frame_ctx.current_frame as i64,
            }),
        )?;
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
}

impl<C: JsContext> ScriptHost for ScriptRealm<C> {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        self.ensure_runtime()?;
        let id = driver_id_from_source(source);
        let fn_name = Self::driver_fn_name(id);
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

        {
            #[cfg(feature = "profile")]
            let _span = tracing::span!(
                target: "render.scene",
                tracing::Level::TRACE,
                "script_apply_frame_ctx"
            )
            .entered();
            self.apply_frame_ctx(frame_ctx, current_node_id)?;
        }

        let fn_name = Self::driver_fn_name(driver);
        {
            #[cfg(feature = "profile")]
            let _span = tracing::span!(
                target: "render.scene",
                tracing::Level::TRACE,
                "script_driver_fn"
            )
            .entered();
            self.ctx.call_global_fn(&fn_name)?;
        }
        // A promise continuation only queues; it does not draw yet. The G2
        // render loop starts its draw pass after a `Promise.all` resolves, so
        // one checkpoint after the driver is not enough. The queue itself is
        // NOT a usable exit signal: g's canvas re-registers its draw loop at
        // the end of every rAF tick, so it never drains empty, and polling
        // `queued == 0` here ran all 32 passes every frame — each pass
        // executing another identical G2 redraw at the same frozen clock.
        // `__ocG2Flush` bounds itself internally (FLUSH_PASSES); a fixed small
        // number of host passes covers the promise resolution plus the two
        // nested rAF steps g-canvaskit's ready protocol needs.
        {
            #[cfg(feature = "profile")]
            let _span = tracing::span!(
                target: "render.scene",
                tracing::Level::TRACE,
                "script_drain_microtasks"
            )
            .entered();
            for _ in 0..MICROTASK_DRAIN_PASSES.min(3) {
                self.ctx.run_microtasks();
                self.ctx.eval("__ocG2Flush();")?;
            }
        }
        {
            #[cfg(feature = "profile")]
            let _span = tracing::span!(
                target: "render.scene",
                tracing::Level::TRACE,
                "script_flush_timelines"
            )
            .entered();
            self.ctx.call_global_fn("__opencatFlushTimelines")?;
        }

        // three-like shim 每帧全量重提交的 3D 快照：导出为帧级 scene3 IR 段
        // （纯声明，随 `RenderFrame.scene3` 跨边界），并在目标 canvas 录入占位
        // `DrawOp::Image { ImageRef::Generated }`——z 序 = canvas 命令流位置，
        // 像素/句柄由执行宿主（engine = opencat-three，web = 真 three.js）
        // 回填进 `FrameMediaPlan::generated_images` 的同名占位条目。
        self.pending_scene3 = self.export_scene3_commits()?;

        // Consuming transfer: the store's frame-level mutations move out
        // zero-copy (apply_frame_ctx's reset_for_frame would clear them at
        // the next run_frame anyway), so the per-frame deep clone of every
        // canvas DrawOp disappears here and at the resolve-side snapshot.
        let snap = self.ctx.with_store_mut(|s| s.take_mutations());
        snap.apply_to_recorder(recorder);
        Ok(())
    }

    fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.target_registry = Some(registry);
    }

    fn take_frame_generated_images(&mut self) -> Vec<crate::ir::FrameGeneratedImage> {
        self.ctx.with_store_mut(|s| s.take_pending_generated_images())
    }

    fn take_scene3_commits(&mut self) -> Option<crate::ir::draw_frame::Scene3Section> {
        self.pending_scene3.take()
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

impl<C: JsContext> ScriptRealm<C> {
    /// Export every scene3 snapshot committed this frame as the frame's 3D IR
    /// section, and record one placeholder `DrawOp::Image` on the target
    /// canvas (z-order = canvas command-stream position). Pixels are NOT
    /// produced here: the executing host rasterizes `RenderFrame.scene3`
    /// (engine → opencat-three, web → real three.js) and patches the matching
    /// `FrameMediaPlan::generated_images` placeholder entries.
    ///
    /// Placeholder ids are IR-content-addressed (`scene3:{target}:{hash}` of
    /// the canonical snapshot JSON): a static scene keeps its id across frames
    /// (table idempotency), an animated one gets a fresh id per content.
    fn export_scene3_commits(&mut self) -> Result<Option<crate::ir::draw_frame::Scene3Section>> {
        let scenes = self.ctx.with_store_mut(|s| s.take_scene3s());
        if scenes.is_empty() {
            return Ok(None);
        }
        // 几何注册表轻量快照（Arc 克隆）：`Asset { id }` 的解析来源。
        let geometries = self.ctx.with_store_mut(|s| s.geometry_registry());
        let mut commits = Vec::with_capacity(scenes.len());
        for (id, scene) in &scenes {
            let (width, height) = (scene.target[0], scene.target[1]);
            let canonical = serde_json::to_vec(scene)?;
            let mut hf = ahash::AHasher::default();
            use std::hash::{Hash, Hasher};
            id.hash(&mut hf);
            canonical.hash(&mut hf);
            let key = format!("scene3:{id}:{:016x}", hf.finish());
            let image_id = crate::ir::GeneratedImageId::from_key(&key);
            self.ctx.with_store_mut(|s| {
                s.record_frame_generated_image_deferred(id, image_id, 0.0, 0.0, width, height);
                // 快照保留：mutation 流仍携带 scene3（对齐 engine 既有测试
                // 的逐帧重提交语义；渲染只消费占位 DrawOp + IR 段）。
                s.record_scene3(id, scene.clone());
            });
            commits.push(crate::ir::draw_frame::Scene3Commit {
                image_id,
                target_id: id.clone(),
                scene: scene.clone(),
            });
        }
        Ok(Some(crate::ir::draw_frame::Scene3Section {
            commits,
            geometries,
        }))
    }
}

pub fn apply_target_registry<C: JsContext>(ctx: &C, registry: &ScriptTargetRegistry) -> Result<()> {
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
    // Static per-node facts: tag, parent id, path data. The DOM host needs
    // these because the style store only carries animatable values — an SVG
    // plugin asking `target.tagName` or `getAttribute('d')` has nowhere else
    // to look.
    let facts: serde_json::Map<String, serde_json::Value> = registry
        .facts
        .iter()
        .map(|(id, facts)| {
            (
                id.clone(),
                json!({
                    "tag": facts.tag,
                    "parent": facts.parent,
                    "d": facts.path_data,
                }),
            )
        })
        .collect();
    ctx.set_ctx_field(
        "__targetRegistry",
        json!({
            "visual": visual,
            "canvas": canvas,
            "nonVisual": non_visual,
            "facts": facts,
        }),
    )
}
