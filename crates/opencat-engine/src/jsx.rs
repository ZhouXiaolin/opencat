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
use opencat_core::script::js_context::JsContext;
use opencat_core::script::runtime::{CANVAS_API_RUNTIME, JSX_RUNTIME, JSX_TRANSFORM};

/// Parse a JSX entry source into a [`ParsedComposition`].
pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let (mut extract, driver) = extract_with_base_dir(input, base_dir)?;
    // 约定：<jsx-dir>/tailwind.css（scripts/build-css.sh 产物）存在时，
    // class 解析走标准 CSS 引擎；不存在时 class 不产生任何声明。
    if let Some(dir) = base_dir {
        let css_path = dir.join("tailwind.css");
        if css_path.is_file() {
            let css_text = std::fs::read_to_string(&css_path)
                .with_context(|| format!("jsx: 读取 {}", css_path.display()))?;
            extract["css"] = json!(css_text);
        }
    }
    opencat_core::parse::jsx::from_extract(&extract, &driver, base_dir)
}

/// Evaluate the entry module once and extract the tree: returns the extract
/// payload (`{meta, fonts, lines}`) plus the per-frame driver source. This is
/// the host-side half of the JSX frontend contract — hosts without a JS realm
/// (wasm) receive the pair over the bridge and run `from_extract` themselves.
pub fn extract_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<(Value, String)> {
    let mut ctx = V8JsContext::new()?;
    ctx.eval(JSX_RUNTIME)?;
    ctx.eval(JSX_TRANSFORM)?;
    // Module top-level code may read `ctx.CanvasKit` (e.g. `useCanvasKit()`);
    // the shim is pure JS until a draw call happens, so it is safe here.
    ctx.eval(CANVAS_API_RUNTIME)?;

    let mut modules: BTreeMap<String, String> = BTreeMap::new();
    let (entry_code, entry_requires) = transform_source(&ctx, input)?;
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
        let (code, requires) = transform_source(&ctx, &source)?;
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

fn transform_source(ctx: &V8JsContext, source: &str) -> Result<(String, Vec<String>)> {
    let call = format!(
        "globalThis.__ocTransformJsx({})",
        serde_json::to_string(source)?
    );
    let out = eval_json_string(&ctx, &call)?;
    let code = out
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("jsx transform: missing `code`"))?
        .to_string();
    let requires = out
        .get("requires")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok((code, requires))
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
