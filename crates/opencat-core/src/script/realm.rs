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
use crate::script::runtime::install_runtime;
use crate::script::{
    ScriptDriverId, ScriptHost, ScriptTargetRegistry, ScriptTextSource, StyleMutations,
    driver_id_from_source,
};


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
    /// 文档 base 目录（宿主 = XML 所在目录）：`GLTFGeometry` 等相对路径的
    /// join 基准——three.js `GLTFLoader` 相对 URL 对 import.meta.url 解析
    /// （参考 js:40 `new URL(prismUrl, import.meta.url)`）的 opencat 对应物。
    document_base: Option<String>,
    /// 本帧已导出的 GPU 命令流段（`export_gpu_streams` 产出，pipeline 经
    /// `take_gpu_streams` 取走装入 `RenderFrame.gpu`）。
    pending_gpu: Option<crate::ir::GpuSection>,
}

impl<C: JsContext> ScriptRealm<C> {
    pub fn new(ctx: C) -> Result<Self> {
        Ok(Self {
            ctx,
            runtime_installed: false,
            installed: HashMap::new(),
            text_sources: HashMap::new(),
            target_registry: None,
            document_base: None,
            pending_gpu: None,
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

        // Runtime 安装序唯一来自 manifest（runtime::install_runtime）：渲染
        // realm 与提取 realm 同序，顺序不变量登记在清单条目注释里。
        install_runtime(&self.ctx)?;
        if let Some(registry) = &self.target_registry {
            apply_target_registry(&self.ctx, registry)?;
        }
        if let Some(base) = &self.document_base {
            self.ctx.set_ctx_field("__documentBase", json!(base))?;
        }
        // Renderer selector for chart compositions ('2d' = official
        // @antv/g-canvas); the project's `g2` bundle segment reads it when
        // injecting the renderer.
        if let Ok(v) = std::env::var("OC_G2_RENDERER") {
            self.ctx
                .eval(&format!("globalThis.OC_G2_RENDERER = {:?};", v))?;
        }
        // Demo slug for the g2 example-batch runner composition.
        if let Ok(v) = std::env::var("OC_G2_DEMO") {
            self.ctx
                .eval(&format!("globalThis.OC_G2_DEMO = {:?};", v))?;
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
        // 帧级 canvas 命令流刚被清空：context2d 的下发镜像随流作废（镜像以
        // 「流里已有该值」为前提跳过同值下发）。
        self.ctx
            .eval("if (globalThis.__ocContext2dNewFrame) globalThis.__ocContext2dNewFrame();")?;

        // 脚本侧 DOM 覆盖层：结构是帧级的，节点身份是 realm 级的。这里按
        // 本帧声明树重建默认子级视图（动态节点表保持不动——插件会复用
        // 模块级草稿元素）。
        if let Some(registry) = &self.target_registry {
            let declared = registry.declared_nodes.clone();
            let text_styles = registry.text_styles.clone();
            self.ctx.with_store_mut(|s| {
                if let Some(overlay) = s.tree_overlay_mut() {
                    overlay.begin_frame(&declared);
                }
                s.set_text_styles(&text_styles);
            });
        }
        // 容器查询单位（`cqw`/`cqh`）写入折 px 的视口基准（合成帧尺寸）。
        self.ctx
            .with_store_mut(|s| s.set_viewport(frame_ctx.width as f32, frame_ctx.height as f32));

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

        // `ctx.global` is the composition clock. `ctx.current` is the active
        // scene clock. The HTML driver keeps both equal unless a composition
        // window binds a local clock.
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
    fn text_styles(&self) -> &HashMap<String, crate::style::ComputedTextStyle> {
        static EMPTY: std::sync::OnceLock<HashMap<String, crate::style::ComputedTextStyle>> =
            std::sync::OnceLock::new();
        self.target_registry
            .as_ref()
            .map(|registry| &registry.text_styles)
            .unwrap_or_else(|| EMPTY.get_or_init(Default::default))
    }

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
    ) -> Result<Vec<StyleMutations>> {
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
        // The official async entries resolve their promises in the microtask
        // drain: `CanvasKitInit(...).then(CK => ...)` stores the facade, and
        // the frame hooks draw with it. Draining BEFORE the hooks puts the
        // surface in place on the very first frame.
        self.ctx.run_microtasks();
        // A promise continuation only queues; it does not draw yet. The G2
        // render loop starts its draw pass after a `Promise.all` resolves, so
        // one checkpoint after the driver is not enough. The queue itself is
        // NOT a usable exit signal: g's canvas re-registers its draw loop at
        // the end of every rAF tick, so it never drains empty, and polling
        // `queued == 0` here ran all 32 passes every frame — each pass
        // executing another identical G2 redraw at the same frozen clock.
        // `__ocFlushRaf` bounds itself internally; a fixed small
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
            let drain_timing = std::env::var_os("OC_FRAME_TIMING").is_some();
            let d0 = drain_timing.then(std::time::Instant::now);
            let d_hooks = d0.map(|t| t.elapsed());
            for _ in 0..MICROTASK_DRAIN_PASSES.min(3) {
                self.ctx.run_microtasks();
                self.ctx.eval("__ocFlushRaf();")?;
            }
            let d_raf = d0.map(|t| t.elapsed());
            // hyperframes 播放器协议（协议事件面，非库级 seek 适配）：liquid
            // 等 WebGL 自绘语料把整帧重绘挂在 window 'hf-seek' 上——它们没有
            // 自己的 rAF 循环，rAF 直驱不经过它们。rAF flush 后按当前虚拟钟
            // 派发一次，使页面 adapter 在 frame-end 读回 GL 画之前完成本帧
            // 重绘。事件名与派发实现住扩展层（hyperframes_api.js 的
            // dispatchSeek，走共享层 createEventTarget 的 window 事件面），
            // 这里只调入口、不写事件名字面量。静默吞掉 handler 异常会让每帧
            // 停在首帧且无据可查——dispatchSeek 内部留痕 __ocLogs。
            self.ctx.eval(
                "globalThis.__ocHyperframes.dispatchSeek((globalThis.__ocClockMs() || 0) / 1000);",
            )?;
            let d_seek = d0.map(|t| t.elapsed());
            // Native web GL: rAF callbacks have issued the real WebGL draw
            // calls; read the canvas drawing buffers into the generated-image
            // channel before the frame's media plan is built.
            self.ctx.eval(
                "if (globalThis.__ocNativeCanvasFrameEnd) globalThis.__ocNativeCanvasFrameEnd();",
            )?;
            if let (Some(t0), Some(hooks), Some(raf), Some(seek)) = (d0, d_hooks, d_raf, d_seek) {
                let end = t0.elapsed();
                eprintln!(
                    "[timing-drain] hooks={:.1}ms raf-flush={:.1}ms hf-seek={:.1}ms frame-end={:.1}ms total={:.1}ms",
                    hooks.as_secs_f64() * 1e3,
                    (raf - hooks).as_secs_f64() * 1e3,
                    (seek - raf).as_secs_f64() * 1e3,
                    (end - seek).as_secs_f64() * 1e3,
                    end.as_secs_f64() * 1e3,
                );
            }
        }
        // SVG-subtree capture: SVG-family chart renderers (g-svg) paint no
        // pixels — their output is an `<svg>` subtree under the container,
        // built in promise continuations. The shadow-DOM layer marks such
        // containers via `__ocMarkSvgSubtree`; here (microtasks drained, the
        // full tree exists) each marked node's subtree serializes into the
        // canvas entry's `svg` slot. Canvas-family renderers never mark, so
        // the queue is empty for them.
        let queued = self.ctx.eval_json("(globalThis.__ocSvgSubtrees || [])")?;
        if let serde_json::Value::Array(nodes) = queued {
            let nodes: Vec<String> = nodes
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if !nodes.is_empty() {
                self.ctx.eval("globalThis.__ocSvgSubtrees = [];")?;
                for node in nodes {
                    let markup = self.ctx.eval_json(&format!(
                        "globalThis.__ocSvgSubtreeMarkup ? globalThis.__ocSvgSubtreeMarkup({}) : null",
                        serde_json::to_string(&node)?
                    ))?;
                    if let serde_json::Value::String(markup) = markup {
                        if !markup.is_empty() {
                            self.ctx.with_store_mut(|s| s.set_canvas_svg(&node, markup));
                        }
                    }
                }
            }
        }
        // GPU 命令流（getContext 录制面每帧全量重录）：导出为帧级 gpu IR
        // 段（随 `RenderFrame.gpu` 跨边界），并在目标 canvas 录入占位
        // `DrawOp::Image { ImageRef::Generated }`——z 序 = canvas 命令流位置，
        // 像素由执行宿主（engine = glow，web = 真 WebGL2）渲染流后回填进
        // `FrameMediaPlan::generated_images` 的同名占位条目。
        self.pending_gpu = self.export_gpu_streams()?;
        // 调试：OC_TRACE_JSLOGS 时把本帧 console 面（__ocLogs）尾部倒到
        // stderr——html 通道没有 __ocStderr 绑定，console.error 只进这个
        // 数组，页面 JS 的报错/探针输出要从这里取。
        if std::env::var_os("OC_TRACE_JSLOGS").is_some() {
            let logs = self
                .ctx
                .eval_json("(globalThis.__ocLogs || []).slice(-6)")?;
            eprintln!("[jslogs] {logs:?}");
            self.ctx
                .eval("if (typeof __ocLogs !== 'undefined') __ocLogs.length = 0;")?;
        }

        // canvas 批积攒的帧尾 flush：drawHalftone 这类「画完整帧不再触发
        // 其他 canvas 调用」的循环把点全憋在 JS 批数组里，批必须进本帧
        // op 流（take 之前）。无批积攒的工程下是空表遍历（无开销）。
        self.ctx
            .eval("if (globalThis.__ocFlushCanvasBatches) globalThis.__ocFlushCanvasBatches();")?;

        // Consuming transfer: the store's frame-level mutations move out
        // zero-copy (apply_frame_ctx's reset_for_frame would clear them at
        // the next run_frame anyway), so the per-frame deep clone of every
        // canvas DrawOp disappears here and at the resolve-side snapshot.
        let snap = self.ctx.with_store_mut(|s| s.take_mutations());
        if std::env::var_os("OC_FRAME_TIMING").is_some() {
            let canvas_cmds: usize = snap
                .canvas_mutations
                .values()
                .map(|m| m.commands.len())
                .sum();
            eprintln!(
                "[timing-ops] style-nodes={} canvas-ids={} canvas-cmds={}",
                snap.mutations.len(),
                snap.canvas_mutations.len(),
                canvas_cmds,
            );
        }
        // 层形态直取：store 的帧级 mutations 零拷贝移出，resolve 侧直接
        // 进 mutation_stack——不再 apply_to_recorder 逐字段重编码（有损：
        // backdrop_blur_sigma/fill_none/font_stack/transform_css 组合会丢）。
        // apply_frame_ctx 的 reset_for_frame 反正会在下一帧清掉它们。
        Ok(vec![snap])
    }

    fn set_target_registry(&mut self, registry: ScriptTargetRegistry) {
        self.target_registry = Some(registry);
    }

    fn take_frame_generated_images(&mut self) -> Vec<crate::ir::FrameGeneratedImage> {
        self.ctx
            .with_store_mut(|s| s.take_pending_generated_images())
    }

    fn take_gpu_streams(&mut self) -> Option<crate::ir::GpuSection> {
        self.pending_gpu.take()
    }

    fn set_initial_style_from_node(&mut self, id: &str, style: &crate::style::NodeStyle) {
        if self.ensure_runtime().is_err() {
            return;
        }
        self.ctx.with_store_mut(|s| {
            s.set_initial_style_from_node(id, style);
        });
    }

    fn text_leaf_ranges(&mut self, id: &str) -> Vec<(Vec<String>, usize, usize)> {
        self.ctx.with_store_mut(|s| s.text_leaf_ranges(id))
    }
}

impl<C: JsContext> ScriptRealm<C> {
    /// Export every GPU stream recorded this frame as the frame's gpu IR
    /// section, and record one placeholder `DrawOp::Image` on the target
    /// canvas (z-order = canvas command-stream position). Pixels are NOT
    /// produced here: the executing host renders the stream (engine → glow,
    /// web → real WebGL2) and patches the matching
    /// `FrameMediaPlan::generated_images` placeholder entries.
    ///
    /// Placeholder ids are IR-content-addressed (`gpu:{target}:{hash}` over
    /// the encoded command bytes): a static stream keeps its id across frames
    /// (table idempotency), an animated one gets a fresh id per content.
    fn export_gpu_streams(&mut self) -> Result<Option<crate::ir::GpuSection>> {
        let streams = self.ctx.with_store_mut(|s| s.take_gpu_streams());
        if streams.is_empty() {
            return Ok(None);
        }
        let mut section = Vec::with_capacity(streams.len());
        for stream in &streams {
            let mut hf = ahash::AHasher::default();
            use std::hash::{Hash, Hasher};
            stream.target_id.hash(&mut hf);
            stream.ops.hash(&mut hf);
            stream.bytes.hash(&mut hf);
            stream.byte_ranges.hash(&mut hf);
            let key = format!("gpu:{}:{:016x}", stream.target_id, hf.finish());
            let image_id = crate::ir::GeneratedImageId::from_key(&key);
            #[cfg(feature = "profile")]
            eprintln!(
                "OCEXPORT stream {} id {} ops_bytes {}",
                stream.target_id,
                image_id.0,
                stream.ops.len()
            );
            self.ctx.with_store_mut(|s| {
                s.record_frame_generated_image_deferred(
                    &stream.target_id,
                    image_id,
                    stream.x,
                    stream.y,
                    stream.width,
                    stream.height,
                );
            });
            section.push(crate::ir::GpuStream {
                image_id,
                target_id: stream.target_id.clone(),
                width: stream.width,
                height: stream.height,
                backend: if stream.backend == 1 {
                    crate::ir::GpuBackend::WebGPU
                } else {
                    crate::ir::GpuBackend::WebGL2
                },
                attrs: crate::ir::gpu_stream::ContextAttrs::unpack(stream.attrs_bits),
                ops: crate::ir::GpuOpFrame {
                    ops: stream.ops.clone(),
                    bytes: stream.bytes.clone(),
                    byte_ranges: stream.byte_ranges.clone(),
                },
            });
        }
        Ok(Some(crate::ir::GpuSection { streams: section }))
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
