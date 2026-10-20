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
use opencat_core::script::runtime::{
    ANIMATION_RUNTIME, CANVAS_API_RUNTIME, CONTEXT2D_RUNTIME, DOM_API_RUNTIME, JSX_RUNTIME,
    NODE_STYLE_RUNTIME, THREE_API_RUNTIME,
};

/// Parse a JSX entry source into a [`ParsedComposition`].
pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let (extract, driver) = extract_with_base_dir(input, base_dir)?;
    // 样式表唯一来源：打包器嵌进 bundle 的 `export const css`。没有则 class
    // 不产生任何声明。
    opencat_core::parse::jsx::from_extract(&extract, &driver, base_dir)
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
    // `ctx.gsap` / `ctx.threejs` / `ctx.canvas` — author modules may
    // `require` them at top level during extract.
    // Module top-level code may read `ctx.CanvasKit` (e.g. `useCanvasKit()`);
    // the shim is pure JS until a draw call happens, so it is safe here.
    ctx.eval(CANVAS_API_RUNTIME)?;
    // Module top-level code may call `document.getElementById` / read
    // `window.innerWidth` before the first frame exists. `ctx.global` starts
    // zeroed, so geometry reads return 0 — correct for extract; the values
    // arrive per-frame once the driver runs.
    ctx.eval(DOM_API_RUNTIME)?;
    // `node.getContext('2d')` at module top level (paint setup outside a hook).
    ctx.eval(CONTEXT2D_RUNTIME)?;
    ctx.eval(NODE_STYLE_RUNTIME)?;
    ctx.eval(THREE_API_RUNTIME)?;
    ctx.eval(ANIMATION_RUNTIME)?;
    // Last: the JSX runtime itself — its package builtins forward to the
    // namespaces the runtimes above install.
    ctx.eval(JSX_RUNTIME)?;

    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let (entry_code, entry_requires) = transform_source(input)?;
    modules.insert(ENTRY_SPEC.to_string(), entry_code);

    let mut queue: VecDeque<(String, String)> = VecDeque::new();
    for spec in entry_requires {
        queue.push_back((ENTRY_SPEC.to_string(), spec));
    }

    while let Some((from, spec)) = queue.pop_front() {
        let key = match normalize_relative(&spec) {
            Some(key) => key,
            None => continue, // builtin (`opencat/runtime`) or non-relative spec
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
/// is where `use*` calls register their per-frame drivers), then replay the
/// hooks. Re-rendering every frame is what re-initializes globals — the
/// `frame = f(t)` contract, same as the XML `<script>` block.
fn build_driver(modules: &BTreeMap<String, String>) -> String {
    format!(
        "(function () {{\n\
             globalThis.__ocModuleSources = {sources};\n\
             globalThis.__ocResetFrame();\n\
             var __ocEntry = globalThis.__ocRequire({entry});\n\
             globalThis.__ocMeta = (__ocEntry && __ocEntry.meta) || {{}};\n\
             var __ocRoot = __ocEntry.default;\n\
             if (typeof __ocRoot === 'function') {{\n\
                 __ocRoot = __ocRoot(__ocEntry.defaultProps || {{}});\n\
                 // Walk the tree so `use*` calls inside components register\n\
                 // their per-frame drivers.\n\
                 globalThis.__ocWalkTree(__ocRoot, null);\n\
             }}\n\
             if (globalThis.__ocMode !== 'extract') globalThis.__ocRunFrameHooks();\n\
         }})();",
        sources = serde_json::to_string(modules).unwrap_or_else(|_| "{}".to_string()),
        entry = serde_json::to_string(ENTRY_SPEC).unwrap_or_else(|_| "\"<entry>\"".to_string()),
    )
}
