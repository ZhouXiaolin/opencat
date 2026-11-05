//! HTML 前端宿主侧（engine）：docs/html-align/DESIGN.md。
//!
//! 与 `jsx.rs` 同构的提取宿主：html5ever 种子树灌进一次性 V8 realm，页面
//! 脚本（本地相对路径 / 远程抓取+磁盘缓存）在种子上构建最终树，
//! `__ocHtmlExtract()` 把最终树序列化回 DomElement JSON，交给
//! `parse::jsx::from_extract`（`html` 标记已在那条链路打通）。
//!
//! 每帧 driver 自包含 [`HTML_API_RUNTIME`]：重物化种子树 → 重放页面脚本
//! （external 守卫只估一次，gsap 等库跨帧存活）→ wrap 过的 gsap 产物
//! pause + 绝对 seek —— write-out 即 seek，`frame = f(t)`。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::resource::fetch::{build_http_client, build_preload_runtime, download_bytes};
use crate::script::V8JsContext;
use opencat_core::parse::html::{HtmlScript, parse_html};
use opencat_core::script::dispatch::{binding_shim_js, dispatch_binding};
use opencat_core::script::js_context::JsContext;
use opencat_core::script::realm::JS_PLATFORM_SHIM;
use opencat_core::script::runtime::{
    ANIMATION_RUNTIME, CANVAS_API_RUNTIME, CONTEXT2D_RUNTIME, DOM_API_RUNTIME,
    HTML_API_RUNTIME, NODE_STYLE_RUNTIME, TREE_API_RUNTIME,
};
use opencat_core::parse::ParsedComposition;

/// Parse an HTML composition source into a [`ParsedComposition`].
pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    let (extract, driver) = extract_html_with_base_dir(input, base_dir)?;
    opencat_core::parse::jsx::from_extract(&extract, &driver, base_dir)
}

/// 从文件读取 HTML 源并解析；`base_dir` 取文件所在目录。
pub fn parse_file(path: impl AsRef<Path>) -> Result<ParsedComposition> {
    let path = path.as_ref();
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    parse_with_base_dir(&input, Some(base_dir))
}

/// script/link 的 src/href 解析：http(s) 走抓取+磁盘缓存，其余按 base_dir
/// 相对路径直读。
fn resolve_external(base_dir: &Path, url: &str) -> Result<String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        fetch_remote_text(base_dir, url)
    } else {
        let path = base_dir.join(url);
        std::fs::read_to_string(&path)
            .with_context(|| format!("html: failed to read `{url}` ({})", path.display()))
    }
}

/// URL → 缓存文件名：非字母数字折叠成 `_` 并截断。缓存只是确定性保障
/// （同 URL 稳定映射），不追求密码学键。
fn cache_file_name(url: &str) -> String {
    let mut name: String = url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    name.truncate(120);
    name
}

/// 远程资源抓取 + 磁盘缓存（`base_dir/.oc-cache/net/`）：命中读盘，未命中
/// 下载写盘——同输入逐字节可复现（DESIGN.md 的网络确定性）。
fn fetch_remote_text(base_dir: &Path, url: &str) -> Result<String> {
    let dir = base_dir.join(".oc-cache").join("net");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("html: failed to create cache dir {}", dir.display()))?;
    let path = dir.join(cache_file_name(url));
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    let client = build_http_client("html: failed to build http client")?;
    let rt = build_preload_runtime("html remote resources")?;
    let bytes = rt.block_on(download_bytes(&client, url))?;
    std::fs::write(&path, &bytes)
        .with_context(|| format!("html: failed to write cache {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// 评估页面脚本并提取最终树：返回 extract payload（`{meta, css, tree, html}`）
/// 加每帧 driver 源。
pub fn extract_html_with_base_dir(
    input: &str,
    base_dir: Option<&Path>,
) -> Result<(Value, String)> {
    let doc = parse_html(input, 30)?;
    let base = base_dir.unwrap_or_else(|| Path::new("."));

    // 嵌套 composition（data-composition-src）：语料深度全部为 1（k3），批次 C。
    if !doc.composition_hosts.is_empty() {
        anyhow::bail!(
            "html: nested composition (data-composition-src) not supported yet ({} host(s))",
            doc.composition_hosts.len()
        );
    }

    // 侧通道样式：`<link rel=stylesheet>` 文本并进 `<style>` 级联尾部（文档序）。
    let mut css_text = doc.css_text.clone();
    for link in &doc.links {
        if link.rel.eq_ignore_ascii_case("stylesheet") {
            css_text.push('\n');
            css_text.push_str(&resolve_external(base, &link.href)?);
        }
    }

    // meta 直接取 parse_html 的根选择结果（data-composition-id 根的
    // data-width/height/fps/duration）。不能从 protos 随便取一个——HashMap
    // 迭代序不确定，会随机落到子场景的 3s 缺省时长上。
    let meta = doc.meta.clone();

    let tree = doc
        .root
        .clone()
        .ok_or_else(|| anyhow::anyhow!("html: no composition root (data-composition-id)"))?;
    let seed = json!({ "meta": meta, "tree": tree, "css": css_text, "html": true });

    let driver = build_html_driver(&doc.scripts, base, &seed)?;

    // 提取 realm：`ScriptRealm::ensure_runtime` 的最小镜像（ctx 全局 +
    // dispatcher + binding shim——V8JsContext 自带 MutationStore，store 绑定
    // 在提取期就是真的，ctx.surface/measureText 可用），然后与 ensure_runtime
    // 同序的平台面。driver 头部的 HTML_API_RUNTIME 覆盖 document 换皮。
    //
    // hyperframes 页面脚本在提取期就调 measureText（k3 pillDims），而 scoped
    // fontdb 要到 prepare 才装——这里按 engine fallback 目录（含系统字体）
    // 现建一个 db 作用住整个提取。
    let fallback_dirs: Vec<String> = crate::fonts::engine_font_fallback_dirs()
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let fallback_refs: Vec<&str> = fallback_dirs.iter().map(|s| s.as_str()).collect();
    let db = std::sync::Arc::new(opencat_core::text::extend_font_db_with_dirs(
        &opencat_core::text::empty_font_db(),
        &fallback_refs,
    ));
    let (extract, driver) = opencat_core::text::scope_font_db(&db, || -> Result<(Value, String)> {
        let mut ctx = V8JsContext::new()?;
        ctx.eval(
            "globalThis.ctx = globalThis.ctx || {\
             global:{time:0, duration:0, frame:0}, current:{time:0, duration:0, frame:0}, \
             __currentCanvasTarget:'',\
             __targetRegistry:{visual:Object.create(null),canvas:Object.create(null),nonVisual:Object.create(null)}\
         };\
         globalThis.__opencatDrivers = globalThis.__opencatDrivers || Object.create(null);",
        )?;
        ctx.install_dispatcher(dispatch_binding)?;
        ctx.eval(&binding_shim_js())?;
        ctx.eval(CANVAS_API_RUNTIME)?;
        ctx.eval(JS_PLATFORM_SHIM)?;
        ctx.eval(DOM_API_RUNTIME)?;
        ctx.eval(CONTEXT2D_RUNTIME)?;
        ctx.eval(NODE_STYLE_RUNTIME)?;
        ctx.eval(TREE_API_RUNTIME)?;
        ctx.eval(ANIMATION_RUNTIME)?;
        ctx.eval(&driver)?;
        let extract = eval_json_string(&ctx, "globalThis.__ocHtmlExtract()")?;
        Ok((extract, driver))
    })?;
    Ok((extract, driver))
}

/// 评估 `code`，期望 JSON 字符串并解析回 `Value`。
fn eval_json_string(ctx: &V8JsContext, code: &str) -> Result<Value> {
    let wrapped = format!("JSON.stringify({code})");
    let raw = ctx.eval_json(&wrapped)?;
    let text = raw
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("html: expected a JSON string from `{code}`"))?;
    Ok(serde_json::from_str(text)?)
}

/// 每帧 driver：头部内嵌 [`HTML_API_RUNTIME`]（IIFE 幂等，每帧重 eval 即
/// 树状态重置）→ 重物化种子 → external 守卫重放 → inline 每帧重放 → 尾部
/// 绝对 seek。gsap 等库定义在 globalThis 上跨帧存活（`__ocResetFrame` 不清
/// globals），守卫键 `__ocExt<n>` 每帧只估一次。
fn build_html_driver(scripts: &[HtmlScript], base_dir: &Path, seed: &Value) -> Result<String> {
    let mut body = String::new();
    let mut ext_idx = 0usize;
    for script in scripts {
        match (&script.src, &script.inline) {
            (Some(src), _) => {
                let code = resolve_external(base_dir, src)?;
                // external 只在首帧估一次（每帧重估会逐帧泄漏 gsap 的全局
                // 插件实例，整片渲染直接 V8 OOM）。但库在初始化时造的探测
                // 元素（gsap 的字体/样式嗅探）会占用合成 id 序，导致后续帧
                // 的 id 整体漂移。用 __ocExtPhase 把它们隔离到独立命名空间，
                // 主命名空间的 id 才两阶段一致。
                body.push_str(&format!(
                    "if (!globalThis.__ocExt{}) {{ globalThis.__ocExt{} = true; \
globalThis.__ocExtPhase = true;\n{}\nglobalThis.__ocExtPhase = false; }}\n",
                    ext_idx, ext_idx, code
                ));
                body.push_str("globalThis.__ocHtmlAfterScripts();\n");
                ext_idx += 1;
            }
            (None, Some(code)) => {
                body.push_str(code);
                body.push_str("\nglobalThis.__ocHtmlAfterScripts();\n");
            }
            (None, None) => {}
        }
    }
    // HTML_API_RUNTIME 原文直接拼（不进 format! 模板——JS 花括号会被当
    // 占位符解析）；seed JSON 走独立插值。
    let mut driver = String::from(HTML_API_RUNTIME);
    driver.push_str("\n;(function () {\n");
    driver.push_str("    // jsx 通道的帧重置（hooks/模块表）；html 通道无 hooks，守卫跳过。\n");
    driver.push_str("    if (typeof globalThis.__ocResetFrame === 'function') globalThis.__ocResetFrame();\n");
    driver.push_str("    globalThis.__ocHtmlResetFrame(");
    driver.push_str(&serde_json::to_string(seed)?);
    driver.push_str(");\n");
    driver.push_str(&body);
    driver.push_str("    globalThis.__ocHtmlSeek(typeof globalThis.ctx !== 'undefined' && globalThis.ctx && globalThis.ctx.current ? globalThis.ctx.current.time : 0);\n");
    driver.push_str("})();");
    Ok(driver)
}
