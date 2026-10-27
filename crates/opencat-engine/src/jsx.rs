//! JSX frontend host side (engine): evaluate the entry module once, extract the
//! element tree, and hand it to `opencat_core::parse::jsx`.
//!
//! Why a throwaway realm: composition parsing is synchronous and pure, so the
//! tree must exist before `prepare` (fonts, target registry, …). The same
//! generated per-frame driver is embedded in the returned composition as the
//! script source — the pipeline then re-evaluates the module graph every frame,
//! which keeps the `frame = f(t)` contract identical to the XML `<script>` path.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::js_context::V8JsContext;
use opencat_core::parse::ParsedComposition;
use opencat_core::parse::jsx::ENTRY_SPEC;
use opencat_core::parse::jsx_transform;
use opencat_core::script::js_context::JsContext;
use opencat_core::script::realm::JS_PLATFORM_SHIM;
use opencat_core::script::runtime::{
    ANIMATION_RUNTIME, ASSET_API_RUNTIME, CANVAS_API_RUNTIME, CONTEXT2D_RUNTIME, DOM_API_RUNTIME,
    JSX_RUNTIME, NODE_STYLE_RUNTIME, TREE_API_RUNTIME,
};

/// Parse a JSX entry source into a [`ParsedComposition`].
///
/// 样式表只有一条来源：打包器嵌进 entry 段的 `export const css`
/// （`opencat-plugin.mjs` 与 `scripts/build-example-bundle.mjs` 都写它）。没有
/// css 时 class 不产生任何声明。
pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let (extract, driver) = extract_with_base_dir(input, base_dir)?;
    opencat_core::parse::jsx::from_extract(&extract, &driver, base_dir)
}

/// 单文件多模块段的段首标记：`/*@@oc-module:<spec>@@*/`。
///
/// 打包器把 gsap/three 的 CJS 产物与 entry 的 ESM 产物拼进同一个
/// `dist/<stem>.bundle.jsx`，每段一个标记。分段的理由不是文件大小，而是
/// `__ocResetFrame` 的跨帧保活（`jsx_runtime.js`）：它只保留 `gsap` / `three`
/// / `canvaskit-wasm` 三个**模块表条目**，three 的 1.6MB 类定义每帧重估会
/// OOM。段切开后键名与保活列表不变，运行时零改动。
const SEGMENT_PREFIX: &str = "/*@@oc-module:";
const SEGMENT_SUFFIX: &str = "@@*/";

/// 按段首标记把 bundle 切成 `(spec, source)`。没有标记的裸 `.jsx`（以及标记
/// 之前的正文）整体归到 [`ENTRY_SPEC`]，两种输入走同一条切段路径。
fn split_bundle_segments(input: &str) -> Vec<(String, &str)> {
    let mut segments: Vec<(String, &str)> = Vec::new();
    let mut current: Option<String> = None;
    let mut start = 0usize;
    let mut offset = 0usize;
    for line in input.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(SEGMENT_PREFIX)
            && let Some(spec) = rest.strip_suffix(SEGMENT_SUFFIX)
        {
            match current.take() {
                Some(spec) => segments.push((spec, &input[start..offset])),
                // 第一个标记之前的正文（裸 `.jsx` 不会有）算 entry。
                None if !input[start..offset].trim().is_empty() => {
                    segments.push((ENTRY_SPEC.to_string(), &input[start..offset]));
                }
                None => {}
            }
            current = Some(spec.trim().to_string());
            offset += line.len();
            start = offset;
            continue;
        }
        offset += line.len();
    }
    match current {
        Some(spec) => segments.push((spec, &input[start..offset])),
        None => segments.push((ENTRY_SPEC.to_string(), input)),
    }
    segments
}

/// Evaluate the entry module once and extract the tree: returns the extract
/// payload (`{meta, fonts, lines}`) plus the per-frame driver source. This is
/// the host-side half of the JSX frontend contract — hosts without a JS realm
/// (wasm) receive the pair over the bridge and run `from_extract` themselves.
pub fn extract_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<(Value, String)> {
    let mut ctx = V8JsContext::new()?;
    // Realm bootstrap + the runtime slice the extract pass needs. The load
    // order mirrors `ScriptRealm::ensure_runtime` (style/canvas/three before
    // the JSX runtime) so the real-package builtins can forward to
    // `ctx.threejs` / `globalThis.CanvasKit` — author modules may `require`
    // them at top level during extract. `gsap` / `three` are project-supplied:
    // their segments are materialized into `__ocModuleSources` by
    // [`split_bundle_segments`].
    ctx.eval(CANVAS_API_RUNTIME)?;
    // rAF queue: the gsap bundle wakes its ticker once at eval time, which
    // schedules a tick — without this the load throws. Same shim (and same
    // queue) the pipeline realm installs before its own bundles.
    ctx.eval(JS_PLATFORM_SHIM)?;
    // Module top-level code may call `document.getElementById` / read
    // `window.innerWidth` before the first frame exists. `ctx.global` starts
    // zeroed, so geometry reads return 0 — correct for extract; the values
    // arrive per-frame once the driver runs.
    ctx.eval(DOM_API_RUNTIME)?;
    // `node.getContext('2d')` at module top level (paint setup outside a hook).
    ctx.eval(CONTEXT2D_RUNTIME)?;
    ctx.eval(NODE_STYLE_RUNTIME)?;
    // Script-side DOM overlay handles. Must follow NODE_STYLE_RUNTIME: a
    // declared handle delegates its style surface to `ctx.__domTarget`.
    ctx.eval(TREE_API_RUNTIME)?;
    ctx.eval(ANIMATION_RUNTIME)?;
    // Last: the JSX runtime itself — its package builtins forward to the
    // namespaces the runtimes above install. `gsap` is not installed here:
    // it is project-supplied and materialized below from the bundle's own segment.
    ctx.eval(JSX_RUNTIME)?;
    // Web loading shims (fetch/Blob/URL/createImageBitmap over the engine's
    // asset natives) — self-guarded, a no-op without them.
    ctx.eval(ASSET_API_RUNTIME)?;

    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let mut queue: VecDeque<(String, String)> = VecDeque::new();

    // 单文件多段：bundle 自带 gsap/three 段时先把它们灌进模块表，BFS 遇到
    // 这些 specifier 就命中缓存直接跳过。裸 `.jsx` 只有一个段，行为不变。
    for (spec, source) in split_bundle_segments(input) {
        let (code, requires) = transform_source(source)?;
        for next in &requires {
            queue.push_back((spec.clone(), next.clone()));
        }
        modules.insert(spec, code);
    }

    while let Some((from, spec)) = queue.pop_front() {
        let key = match normalize_relative(&spec) {
            Some(key) => key,
            None => {
                // `gsap` / `three` 由打包器切段带进来（见 split_bundle_segments）；
                // 段缺失说明项目没装该包，模块图里也没有它，落到 JS 运行时的
                // resolveSpec 报缺失包名。其余非相对 specifier（`opencat/runtime`
                // 等内建）同理。
                continue;
            }
        };
        if modules.contains_key(&key) {
            continue;
        }
        let path = module_path(base_dir, &from, &key);
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("jsx: failed to read module `{spec}` ({})", path.display()))?;
        let (code, requires) = transform_source(&source)?;
        modules.insert(key.clone(), code);
        for next in requires {
            queue.push_back((key.clone(), next));
        }
    }

    let driver = build_driver(&modules);
    ctx.eval(&format!(
        "globalThis.__ocModuleSources = {}; globalThis.__ocMode = 'extract';",
        serde_json::to_string(&modules)?
    ))?;
    ctx.eval(&driver)?;
    let extract = eval_json_string(&ctx, &format!("globalThis.__ocExtract({})", json!(ENTRY_SPEC)))?;

    Ok((extract, driver))
}

/// Evaluate `code`, expect a JSON-encodable value, and parse it back.
fn eval_json_string(ctx: &V8JsContext, code: &str) -> Result<Value> {
    let wrapped = format!("JSON.stringify({code})");
    let raw = ctx.eval_json(&wrapped)?;
    let text = raw
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("jsx: expected a JSON string from `{code}`"))?;
    Ok(serde_json::from_str(text)?)
}

/// ESM/JSX → 引擎模块脚本（oxc 前端，`parse::jsx_transform`）。旧的手写 JS
/// 扫描器（`__ocTransformJsx`）已退役。
fn transform_source(source: &str) -> Result<(String, Vec<String>)> {
    let out = jsx_transform::transform(source)?;
    Ok((out.code, out.requires))
}

/// Normalize a relative specifier to a module key with forward slashes.
fn normalize_relative(spec: &str) -> Option<String> {
    if !spec.starts_with('.') {
        return None;
    }
    let normalized = spec.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for seg in normalized.split('/') {
        match seg {
            "" | "." => continue,
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

fn module_path(base_dir: Option<&Path>, from_key: &str, key: &str) -> PathBuf {
    let dir = match from_key.rfind('/') {
        Some(idx) => &from_key[..idx],
        None => "",
    };
    let base = base_dir.unwrap_or_else(|| Path::new("."));
    if dir.is_empty() {
        base.join(key)
    } else {
        base.join(dir).join(key)
    }
}

/// The per-frame driver: fresh module state, render the entry component (this
/// is where `use*` calls register their per-frame drivers). Re-rendering every
/// frame is what re-initializes globals — the `frame = f(t)` contract, same
/// as the XML `<script>` block. The hooks themselves run after the frame's
/// microtask drain (run_frame), so promise-resolved surfaces — the official
/// `CanvasKitInit(...).then(...)` shape — are in place before any draw.
fn build_driver(modules: &BTreeMap<String, String>) -> String {
    format!(
        "(function () {{\n\
             globalThis.__ocModuleSources = {sources};\n\
             globalThis.__ocResetFrame();\n\
             var __ocEntry = globalThis.__ocRequire({entry});\n\
             globalThis.__ocMeta = (__ocEntry && __ocEntry.meta) || {{}};\n\
             // Entry convention: `export function main()` first, `export default`\n\
             // as the single-file spelling (see __ocExtract / resolveEntryRoot).\n\
             var __ocRoot = typeof __ocEntry.main === 'function' ? __ocEntry.main(__ocEntry.defaultProps || {{}}) : __ocEntry.default;\n\
             if (typeof __ocRoot === 'function') __ocRoot = __ocRoot(__ocEntry.defaultProps || {{}});\n\
             // Walk the tree so `use*` calls inside components register their\n\
             // per-frame drivers. The walk is UNCONDITIONAL: `main()` may return\n\
             // either a component or an already-rendered element, and an element\n\
             // still holds components further down that own hooks.\n\
             globalThis.__ocWalkTree(__ocRoot, null);\n\
         }})();",
        sources = serde_json::to_string(modules).unwrap_or_else(|_| "{}".to_string()),
        entry = serde_json::to_string(ENTRY_SPEC).unwrap_or_else(|_| "\"<entry>\"".to_string()),
    )
}
