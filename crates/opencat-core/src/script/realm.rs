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
    ANIMATION_RUNTIME, CANVAS_API_RUNTIME, JSX_RUNTIME, NODE_STYLE_RUNTIME, THREE_API_RUNTIME,
};
use crate::script::{
    ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource, driver_id_from_source,
};

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
        self.ctx.eval(ANIMATION_RUNTIME)?;
        // JSX frontend: h() + use* registry + module loader. Pure-XML
        // compositions simply never call into it.
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
    ctx.set_ctx_field(
        "__targetRegistry",
        json!({
            "visual": visual,
            "canvas": canvas,
            "nonVisual": non_visual,
        }),
    )
}
