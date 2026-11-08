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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::resource::fetch::{build_http_client, build_preload_runtime, download_bytes};
use crate::script::V8JsContext;
use opencat_core::parse::dom::DomElement;
use opencat_core::parse::html::{HtmlScript, parse_html};
use opencat_core::script::dispatch::{binding_shim_js, dispatch_binding};
use opencat_core::script::js_context::JsContext;
use opencat_core::script::realm::JS_PLATFORM_SHIM;
use opencat_core::script::runtime::{
    ANIMATION_RUNTIME, ASSET_API_RUNTIME, CANVAS_API_RUNTIME, CONTEXT2D_RUNTIME, DOM_API_RUNTIME,
    GL_OPCODES_RUNTIME, HTML_API_RUNTIME, JSX_RUNTIME, NODE_STYLE_RUNTIME, TREE_API_RUNTIME,
    WEBGL_API_RUNTIME, WEBGPU_API_RUNTIME,
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
    let (_, bytes) = fetch_remote_bytes(base_dir, url)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// [`fetch_remote_text`] 的字节版：返回（缓存文件相对 base_dir 的路径，字节）。
pub(crate) fn fetch_remote_bytes(base_dir: &Path, url: &str) -> Result<(PathBuf, Vec<u8>)> {
    let rel = Path::new(".oc-cache").join("net").join(cache_file_name(url));
    let path = base_dir.join(&rel);
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok((rel, bytes));
    }
    let dir = base_dir.join(".oc-cache").join("net");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("html: failed to create cache dir {}", dir.display()))?;
    let client = build_http_client("html: failed to build http client")?;
    let rt = build_preload_runtime("html remote resources")?;
    // 代理/TLS 偶发断连：重试 3 次，只有连续失败才上报。
    let mut last: Option<anyhow::Error> = None;
    for _ in 0..3 {
        match rt.block_on(download_bytes(&client, url)) {
            Ok(bytes) => {
                std::fs::write(&path, &bytes)
                    .with_context(|| format!("html: failed to write cache {}", path.display()))?;
                return Ok((rel, bytes));
            }
            Err(err) => {
                last = Some(err);
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        }
    }
    Err(last.expect("loop ran at least once"))
}

/// 语句结束位置：`;` 只在括号与引号之外才是终结符。
fn find_statement_end(s: &str) -> usize {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ';' if depth == 0 => return i,
                _ => {}
            },
        }
    }
    s.len()
}

/// 展开 CSS 文本里的 `@import`：拼接后 `@import` 不在表首、解析器按规范
/// 丢弃，这里按文本取出 url 抓取（远程走缓存，本地按 base_dir），返回被
/// 引入的样式文本（只服务 `@font-face` 等与规则序无关的声明）。
fn expand_css_imports(base_dir: &Path, css: &str) -> Result<String> {
    let mut out = String::new();
    let mut rest = css;
    while let Some(at) = rest.find("@import") {
        rest = &rest[at + "@import".len()..];
        // `;` 只有括号与引号之外才终结语句：Google Fonts 的查询串里带
        // 分号（权重后缀），按裸 `;` 切会把后面的族名整段丢掉。
        let end = find_statement_end(rest);
        let stmt = rest[..end].trim();
        let target = stmt
            .strip_prefix("url(")
            .and_then(|s| s.split(')').next())
            .unwrap_or(stmt)
            .trim()
            .trim_matches(|c| c == '"' || c == '\'');
        if !target.is_empty() {
            // 浏览器语义：@import 取不到就跳过该样式表，不中断文档。
            match resolve_external(base_dir, target) {
                Ok(text) => {
                    out.push('\n');
                    out.push_str(&text);
                }
                Err(err) => eprintln!("html: @import `{target}` 失败，跳过：{err:#}"),
            }
        }
        rest = &rest[end..];
    }
    Ok(out)
}

/// `@font-face` → 文档字体清单项（族名，相对 base_dir 的字体文件路径，字节）。
/// 远程字体落 `.oc-cache/net`（与脚本/样式同一缓存）；woff2 解码为 TTF
/// 后落 `.oc-cache/fonts`（fontdb 只认 TTF/OTF），woff 1.0 跳过。同一
/// 文件多次声明只收一次。
/// 返回值附带文档声明的全部 @font-face 族名（含未收集的 woff2 面），
/// 供默认字体注入判断「文档已带此族」。
fn css_font_faces(
    base_dir: &Path,
    css: &str,
) -> Result<(Vec<(String, PathBuf, Vec<u8>)>, Vec<String>)> {
    let sheet = opencat_core::css::CssStylesheet::parse(css)?;
    let declared: Vec<String> = sheet
        .font_faces
        .iter()
        .map(|f| f.family.clone())
        .collect();
    let mut out: Vec<(String, PathBuf, Vec<u8>)> = Vec::new();
    for face in &sheet.font_faces {
        // woff2 放行（解码见下）；woff 1.0（zlib per-table）语料未命中，仍跳过。
        let Some(url) = face.urls.iter().find(|u| {
            let lower = u.split(['?', '#']).next().unwrap_or(u).to_ascii_lowercase();
            !lower.ends_with(".woff")
        }) else {
            eprintln!("html: @font-face `{}` 只有 woff 源，跳过", face.family);
            continue;
        };
        let (rel, bytes) = if url.starts_with("http://") || url.starts_with("https://") {
            // 浏览器语义：字体下载失败只影响该 face，文档继续。
            match fetch_remote_bytes(base_dir, url) {
                Ok(pair) => pair,
                Err(err) => {
                    eprintln!("html: @font-face `{}` 下载失败，跳过：{err:#}", face.family);
                    continue;
                }
            }
        } else {
            let rel = PathBuf::from(url);
            let bytes = std::fs::read(base_dir.join(&rel))
                .with_context(|| format!("html: failed to read font `{url}`"))?;
            (rel, bytes)
        };
        // woff2 → TTF：渲染期清单只传路径，解码产物落 `.oc-cache/fonts/`
        // （与 append_default_html_fonts 同模式），rel 改指缓存——prepare 按
        // 路径重读时拿到的是 TTF。提取期 db 直接吃解码后的字节。
        // 解码失败回退「跳过该 face」语义（浏览器容错：坏字体面不炸文档；
        // 硬失败会让此前靠兜底字体出片的工程整体变挂——hero 回归的教训）。
        let decoded = opencat_core::fonts::decode_woff2_if_needed(bytes);
        let is_woff2 = url
            .split(['?', '#'])
            .next()
            .unwrap_or(url)
            .to_ascii_lowercase()
            .ends_with(".woff2");
        let (rel, bytes) = if is_woff2 && decoded.starts_with(b"wOF2") {
            eprintln!("html: @font-face `{}` woff2 解码失败，跳过", face.family);
            continue;
        } else if is_woff2 {
            let dir = base_dir.join(".oc-cache").join("fonts");
            let name = format!(
                "woff2-{}.ttf",
                cache_file_name(url.split(['?', '#']).next().unwrap_or(url))
            );
            let path = dir.join(&name);
            if !path.exists() {
                if let Err(err) =
                    std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &decoded))
                {
                    eprintln!("html: woff2 缓存写盘失败：{err:#}");
                }
            }
            (PathBuf::from(".oc-cache/fonts").join(name), decoded)
        } else {
            // 非 woff2（或 woff2 已解）：decode_woff2_if_needed 对非 `wOF2`
            // magic 原样返回，decoded 即原始字节。
            (rel, decoded)
        };
        if out.iter().all(|(_, p, _)| *p != rel) {
            out.push((face.family.clone(), rel, bytes));
        }
    }
    Ok((out, declared))
}

/// 页面声明了某字体族却没带 @font-face 时，补引擎内置品牌字体面
/// （hyperframes 参考环境的运行时默认注入：参考标题字重 ≥700 实测落
/// 700 面——Inter 只有 400/500/600/700，没有 800/900）。字节落
/// `.oc-cache/fonts/`：清单要的是路径，磁盘缓存保证渲染确定性。
/// 文档已声明同族 @font-face 则整族跳过（文档面优先，浏览器语义）。
fn append_default_html_fonts(
    base_dir: &Path,
    css: &str,
    mut faces: Vec<(String, PathBuf, Vec<u8>)>,
    declared: &[String],
) -> Vec<(String, PathBuf, Vec<u8>)> {
    for face in crate::fonts::html_default_font_faces() {
        let family = face.family;
        if declared.iter().any(|d| d.eq_ignore_ascii_case(family)) {
            continue;
        }
        // 粗筛：页面没引用这个族就不注入（省 ~1.6MB 加载与缓存落盘）。
        if !css.contains(family) {
            continue;
        }
        let dir = base_dir.join(".oc-cache").join("fonts");
        let path = dir.join(face.file);
        if !path.exists() {
            if let Err(err) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, face.bytes)) {
                eprintln!("html: 默认字体 `{family}` 缓存失败，跳过：{err:#}");
                continue;
            }
        }
        let rel = PathBuf::from(".oc-cache/fonts").join(face.file);
        if faces.iter().all(|(_, p, _)| *p != rel) {
            faces.push((family.to_string(), rel, face.bytes.to_vec()));
        }
    }
    faces
}

/// 评估页面脚本并提取最终树：返回 extract payload（`{meta, css, tree, html}`）
/// 加每帧 driver 源。
pub fn extract_html_with_base_dir(
    input: &str,
    base_dir: Option<&Path>,
) -> Result<(Value, String)> {
    let doc = parse_html(input, 30)?;
    let base = base_dir.unwrap_or_else(|| Path::new("."));

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

    let mut tree = doc
        .root
        .clone()
        .ok_or_else(|| anyhow::anyhow!("html: no composition root (data-composition-id)"))?;

    // 嵌套 composition（`data-composition-src`）：子工程内联进宿主元素，它的
    // `<style>` 并进同一级联，脚本带时间基准并进同一序列。子工程在浏览器里
    // 由 hyperframes 的 runtime 单独装载，脚本只看到自己的子树；我们的等价
    // 做法是让根选择器（`[data-composition-id="…"]`）把样式与查询都收窄到
    // 子根上（子根被挂进宿主，`page` 层不变）。
    let mut scripts = doc.scripts.clone();
    // 子文档 src 脚本的撞车 id 改写文本（归一路径 → 改写后全文），driver
    // 读取时按同键替换——保持「同 src 只估一次」的去重语义。
    let mut src_rewrites = std::collections::HashMap::new();
    if !doc.composition_hosts.is_empty() {
        splice_nested_hosts(&mut tree, &mut css_text, &mut scripts, base, 0.0, &mut src_rewrites)?;
    }

    // `@import`（子工程 `<style>` 里的 Google Fonts 等）展开 + `@font-face`
    // → 文档字体清单：提取期 measureText 与渲染期排版用同一批字体。
    let imported = expand_css_imports(base, &css_text)?;
    css_text.push_str(&imported);
    let (mut css_fonts, declared_families) = css_font_faces(base, &css_text)?;
    let css_fonts = append_default_html_fonts(base, &css_text, css_fonts, &declared_families);

    let seed = json!({ "meta": meta, "tree": tree, "css": css_text, "html": true });

    let driver = build_html_driver(&scripts, base, &seed, doc.import_map.as_deref(), &src_rewrites)?;
    if let Some(path) = std::env::var_os("OC_DUMP_DRIVER") {
        let _ = std::fs::write(&path, &driver);
    }

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
    let mut db = opencat_core::text::extend_font_db_with_dirs(
        &opencat_core::text::empty_font_db(),
        &fallback_refs,
    );
    for (_, _, bytes) in &css_fonts {
        db.load_font_data(bytes.clone());
    }
    let db = std::sync::Arc::new(db);
    let (mut extract, driver) = opencat_core::text::scope_font_db(&db, || -> Result<(Value, String)> {
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
        // WebGL2/WebGPU 录制面：three 的 WebGLRenderer 走 ctx.getContext
        // （GL_OPCODES 先装，WEBGL_API 引用 GLOP.*）。顺序与 realm 一致。
        ctx.eval(GL_OPCODES_RUNTIME)?;
        ctx.eval(WEBGL_API_RUNTIME)?;
        ctx.eval(WEBGPU_API_RUNTIME)?;
        ctx.eval(NODE_STYLE_RUNTIME)?;
        ctx.eval(TREE_API_RUNTIME)?;
        ctx.eval(ANIMATION_RUNTIME)?;
        // 网络加载垫（fetch/Blob/URL/createImageBitmap）：html 页面的 three
        // GLTFLoader 等走这条通道读 `<base_dir>` 下的资产。
        ctx.eval(ASSET_API_RUNTIME)?;
        // 模块装载（__ocRequire/__ocModuleSources）：`<script type="module">`
        // 的降级产物在提取期就要可执行。
        ctx.eval(JSX_RUNTIME)?;
        crate::script::bridge::install_asset_bridge(&ctx, base);
        ctx.eval(&driver)?;
        // 资产读取是微任务（fetch 返回 Promise）：脚本同步体跑完后再排一次
        // 微任务队列，onLoad 回调才会在 extract 之前落地。
        ctx.run_microtasks();
        let extract = eval_json_string(&ctx, "globalThis.__ocHtmlExtract()")?;
        if let Some(path) = std::env::var_os("OC_DUMP_EXTRACT") {
            let _ = std::fs::write(&path, serde_json::to_string_pretty(&extract)?);
        }
        Ok((extract, driver))
    })?;
    if !css_fonts.is_empty() {
        let faces: Vec<Value> = css_fonts
            .iter()
            .enumerate()
            .map(|(i, (family, rel, _))| {
                json!({
                    "id": format!("css-font-{i}"),
                    "family": family,
                    "path": rel.to_string_lossy().replace('\\', "/"),
                })
            })
            .collect();
        extract["fonts"] = json!({ "css": true, "faces": faces });
    }
    Ok((extract, driver))
}

/// 嵌套 composition 展开：每个 `data-composition-src` 宿主的子工程根成为该
/// 宿主的唯一子节点；子工程的 `<style>` 并进同一级联，脚本带时间基准并进
/// 同一序列。递归处理子工程里的再嵌套，偏移在子工程自己的时间轴上累加。
fn splice_nested_hosts(
    root: &mut DomElement,
    css: &mut String,
    scripts: &mut Vec<HtmlScript>,
    base_dir: &Path,
    offset: f64,
    src_rewrites: &mut std::collections::HashMap<String, String>,
) -> Result<()> {
    let mut hosts: Vec<(String, String, f64, Option<f64>)> = Vec::new();
    collect_hosts(root, &mut hosts);
    // 宿主树已占用的 id 集（作者 id + 合成 id）：子文档的作者 id 与之撞车
    // 时整文档一致改写（树内 + CSS id 选择器），子文档脚本照旧按作者 id 查
    // 自己的子树——查询面（querySelector）按 composition 根收窄，改写后
    // 树内 id 与选择器同步，语义不变。
    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    collect_ids(root, &mut taken);
    for (host_id, src, start, host_duration) in hosts {
        let text = resolve_external(base_dir, &src)?;
        // 合成 id 带宿主 id 前缀：子文档的 `hf-N` 与宿主的 `hf-N` 会撞在
        // 同一棵树里。作者 id 的撞车改写在 core（parse_html_prefixed 以
        // `taken` 为已占集）。
        let sub = opencat_core::parse::html::parse_html_prefixed_excluded(
            &text,
            30,
            &format!("{host_id}-"),
            &taken,
        )
        .with_context(|| format!("html: failed to parse nested composition `{src}`"))?;
        let mut sub_root = sub.root.clone().ok_or_else(|| {
            anyhow::anyhow!("html: nested composition `{src}` has no composition root")
        })?;
        let sub_offset = offset + start;
        // 子文档资产相对子文档目录（hyperframes composition 协议，参考端同
        // 语义）：`./`/`../` 形态的媒体路径按子文档所在目录归一为相对主
        // base_dir——prepare 按 path 直读，不归一就是 `missing host input`
        // （variables/frame-md 语料）。
        let sub_dir = match src.rfind('/') {
            Some(i) => &src[..i],
            None => "",
        };
        rewrite_asset_paths(&mut sub_root, sub_dir);
        // 子文档的可见窗（data-start）是子文档局部时间，引擎按全局秒判窗：
        // 先平移再递归（再嵌套的子根在递归里按自己的偏移平移，不重复叠加）。
        shift_clip_starts(&mut sub_root, sub_offset);
        // 宿主槽位时长是可见权威：子文档根的 data-duration 是内容时长
        // （如 flex-shader 的 0.94s 内容对 1.20s 槽位——尾部 `tl.to({},{})`
        // 补齐持帧）。按子时长切窗会把整组合提前隐藏，参考端持帧到槽尾。
        if let Some(host_duration) = host_duration
            && let DomElement::Div {
                data_duration: Some(dur),
                ..
            } = &mut sub_root
            && *dur < host_duration
        {
            *dur = host_duration;
        }
        splice_nested_hosts(&mut sub_root, css, scripts, base_dir, sub_offset, src_rewrites)?;
        for link in &sub.links {
            if link.rel.eq_ignore_ascii_case("stylesheet") {
                let href = rewrite_rel_path(&link.href, sub_dir);
                css.push('\n');
                css.push_str(&resolve_external(base_dir, &href)?);
            }
        }
        // 子文档 CSS 的相对 url(...)（@font-face 的 `../fonts/…` 等）同样
        // 相对子文档目录——按 sub_dir 归一，css_font_faces 才能按主 base_dir
        // 读到（woff2 放行后 pr-to-video 语料暴露）。link href 同语义。
        css.push('\n');
        css.push_str(&rewrite_css_asset_urls(&sub.css_text, sub_dir));
        for script in &sub.scripts {
            let mut script = script.clone();
            script.start += sub_offset;
            // 撞车改写的作者 id 同步进脚本文本：查询面是全局单树，改名后
            // `#old` 按作者 id 查询落空（spacex `#tk` 语料——三份子文档同名
            // id）。inline 直接重写；src 文本在 build_html_driver 读取时按
            // `src_rewrites` 表替换（同键保同库去重语义）。module 文本走
            // 模块图装载，不在此列。
            if !sub.id_renames.is_empty() {
                if let Some(code) = script.inline.as_mut() {
                    rewrite_id_selectors(code, &sub.id_renames);
                }
                if let (Some(src), false) = (&script.src, script.module) {
                    let norm = rewrite_rel_path(src, sub_dir);
                    if let Ok(code) = resolve_external(base_dir, &norm) {
                        let mut rewritten = code;
                        rewrite_id_selectors(&mut rewritten, &sub.id_renames);
                        src_rewrites.insert(norm.clone(), rewritten);
                        script.src = Some(norm);
                    }
                }
            }
            scripts.push(script);
        }
        // 改写后的子文档 id 并回已占集：同一宿主树的多个 composition 之间
        // 也不撞。
        collect_ids(&sub_root, &mut taken);
        if let Some(host) = find_element_mut(root, &host_id)
            && let Some(children) = host.children_mut()
        {
            children.clear();
            children.push(sub_root);
        }
    }
    Ok(())
}

/// `./`/`../` 开头的相对路径按子文档目录归一为主 base_dir 相对路径；
/// 其余（裸相对/远程/数据 URI）原样返回。
fn rewrite_rel_path(path: &str, sub_dir: &str) -> String {
    if path.starts_with("./") || path.starts_with("../") {
        let joined = if sub_dir.is_empty() {
            path.to_string()
        } else {
            format!("{sub_dir}/{path}")
        };
        module_key(&joined)
    } else {
        path.to_string()
    }
}

/// 子文档 CSS 文本里 `url(...)` 的相对引用归一（@font-face src 等）。
fn rewrite_css_asset_urls(css: &str, sub_dir: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(at) = rest.find("url(") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 4..];
        let Some(close) = tail.find(')') else {
            out.push_str("url(");
            rest = tail;
            break;
        };
        let inner = tail[..close].trim();
        let unquoted = inner.trim_matches(|c| c == '\'' || c == '"');
        out.push_str("url(");
        if unquoted.starts_with("./") || unquoted.starts_with("../") {
            out.push_str(&rewrite_rel_path(unquoted, sub_dir));
        } else {
            out.push_str(inner);
        }
        out.push(')');
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// 脚本文本里的 `#old` id 选择器同步改写（querySelector/gsap 字符串等）。
/// 词边界 = 后随字符不是 ident 字符（字母/数字/`-`/`_`），与 CSS 侧
/// `rewrite_css_ids` 同规则。裸字符串 id（`getElementById('old')`）不在此
/// 处理面——现网语料的子文档查询全走选择器形态。
fn rewrite_id_selectors(code: &mut String, renames: &std::collections::HashMap<String, String>) {
    for (old, new) in renames {
        let needle = format!("#{old}");
        let mut out = String::with_capacity(code.len());
        let mut rest = code.as_str();
        while let Some(at) = rest.find(&needle) {
            out.push_str(&rest[..at]);
            let after = &rest[at + needle.len()..];
            let boundary = after
                .chars()
                .next()
                .is_none_or(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_'));
            if boundary {
                out.push('#');
                out.push_str(new);
            } else {
                out.push_str(&needle);
            }
            rest = after;
        }
        out.push_str(rest);
        *code = out;
    }
}

/// 子树资产路径按子文档目录归一：`./`/`../` 开头的媒体路径（video/image/
/// lottie/audio 的 path）相对 `sub_dir` 归一为主 base_dir 相对路径。裸相对
/// （`assets/…`）与远程 URL 不动——前者按主 base 语义已是现网行为，后者
/// 无目录语义。
fn rewrite_asset_paths(el: &mut DomElement, sub_dir: &str) {
    let rewrite = |p: &mut Option<String>| {
        if let Some(path) = p {
            *path = rewrite_rel_path(path, sub_dir);
        }
    };
    match el {
        DomElement::Video { path, .. } => rewrite(path),
        DomElement::Image { path, .. } => rewrite(path),
        DomElement::Lottie { path, .. } => rewrite(path),
        DomElement::Audio { path, .. } => rewrite(path),
        _ => {}
    }
    if let Some(children) = el.children_mut() {
        for child in children {
            rewrite_asset_paths(child, sub_dir);
        }
    }
}

/// 子树可见窗起点平移 `offset` 秒（嵌套 composition 局部时间 → 全局）。
fn shift_clip_starts(el: &mut DomElement, offset: f64) {
    if let DomElement::Div { data_start: Some(start), .. }
    | DomElement::Video { data_start: Some(start), .. } = el
    {
        *start += offset;
    }
    if let Some(children) = el.children_mut() {
        for child in children {
            shift_clip_starts(child, offset);
        }
    }
}

/// 收集子树全部元素 id（含根）。
fn collect_ids(el: &DomElement, out: &mut std::collections::HashSet<String>) {
    let id = el.id();
    if !id.is_empty() {
        out.insert(id.to_string());
    }
    for child in el.children() {
        collect_ids(child, out);
    }
}

/// 收集子树里的嵌套 composition 宿主：`(元素 id, 子工程路径, data-start,
/// data-duration)`。
fn collect_hosts(el: &DomElement, out: &mut Vec<(String, String, f64, Option<f64>)>) {
    let attrs = el.html_attrs();
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    if let Some(src) = get("data-composition-src").filter(|src| !src.is_empty()) {
        let start = get("data-start")
            .and_then(|value| value.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let duration = get("data-duration").and_then(|value| value.trim().parse::<f64>().ok());
        out.push((el.id().to_string(), src.to_string(), start, duration));
    }
    for child in el.children() {
        collect_hosts(child, out);
    }
}

fn find_element_mut<'a>(el: &'a mut DomElement, id: &str) -> Option<&'a mut DomElement> {
    if el.id() == id {
        return Some(el);
    }
    let children = el.children_mut()?;
    for child in children.iter_mut() {
        if let Some(hit) = find_element_mut(child, id) {
            return Some(hit);
        }
    }
    None
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
///
/// `<script type="module">` 走模块图装载（[`load_html_modules`]）：oxc 降级
/// 后的源码经 `__ocModuleSources` 注入，原位置以 `__ocRequire(key)` 占位。
/// 注意 html driver **不调** jsx 的 `__ocResetFrame`——模块 exports 缓存在
/// JSX_RUNTIME 闭包里跨帧保活（与 jsx 通道的 three 豁免同语义），重跑
/// three 的定义体每帧 1.4MB 会拖垮整片渲染。
fn build_html_driver(
    scripts: &[HtmlScript],
    base_dir: &Path,
    seed: &Value,
    import_map: Option<&str>,
    src_rewrites: &std::collections::HashMap<String, String>,
) -> Result<String> {
    let mut body = String::new();
    let mut ext_idx = 0usize;
    // 每个脚本执行前的 timeline 计数断点，配它所属 composition 的时间基准：
    // 尾部按段 seek，让嵌套 composition 用 `主时间 − data-start` 驱动自己的
    // timeline（浏览器里它们是各自独立的文档时间轴）。
    let mut marks: Vec<(f64, String)> = Vec::new();
    let mut seen_src: Vec<&str> = Vec::new();
    // module 入口按文档序收集；require 占位语句与装载产物一一对应。
    let module_entries: Vec<(usize, &HtmlScript)> = scripts
        .iter()
        .enumerate()
        .filter(|(_, s)| s.module)
        .collect();
    let (module_sources, module_requires) =
        load_html_modules(&module_entries, base_dir, import_map)?;
    for (index, script) in scripts.iter().enumerate() {
        let mark = format!("__ocB{index}");
        body.push_str(&format!(
            "var {mark} = globalThis.__ocTimelineCount();\n"
        ));
        marks.push((script.start, mark));
        if script.module {
            let slot = module_entries
                .iter()
                .position(|(i, _)| *i == index)
                .and_then(|p| module_requires.get(p));
            if let Some(call) = slot {
                body.push_str(call);
                body.push('\n');
            }
            body.push_str("globalThis.__ocHtmlAfterScripts();\n");
            continue;
        }
        match (&script.src, &script.inline) {
            (Some(src), _) => {
                // 同一库脚本（宿主与子工程都引 gsap CDN）只估一次。
                if seen_src.contains(&src.as_str()) {
                    continue;
                }
                seen_src.push(src);
                // 撞车 id 改写过的子文档 src 脚本用改写文本（键=归一路径），
                // 其余照读原文。
                let code = match src_rewrites.get(src) {
                    Some(rewritten) => rewritten.clone(),
                    None => resolve_external(base_dir, src)?,
                };
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
    driver.push_str("    // jsx 通道的帧重置只在 jsx realm 有意义：html 的模块缓存\n");
    driver.push_str("    // 必须跨帧保活（见上），故不调 __ocResetFrame。\n");
    driver.push_str("    globalThis.__ocHtmlResetFrame(");
    driver.push_str(&serde_json::to_string(seed)?);
    driver.push_str(");\n");
    if !module_sources.is_empty() {
        driver.push_str("    globalThis.__ocModuleSources = ");
        driver.push_str(&serde_json::to_string(&module_sources)?);
        driver.push_str(";\n");
    }
    driver.push_str(&body);
    // 每段用「主时间 − 该 composition 的 data-start」驱动，与浏览器里子文档
    // 自己的时间轴一致；list[0] 是宿主（偏移 0），–ocSeekT 取它保 putImageData
    // 的键仍是主时间。
    let time_expr = "(globalThis.ctx && globalThis.ctx.current ? globalThis.ctx.current.time : 0)";
    driver.push_str("    globalThis.__ocHtmlSeekGroups([");
    for (index, (offset, mark)) in marks.iter().enumerate() {
        let next = marks
            .get(index + 1)
            .map(|(_, next_mark)| next_mark.clone())
            .unwrap_or_else(|| "globalThis.__ocTimelineCount()".to_string());
        driver.push_str(&format!("[{time_expr} - ({offset:?}), {mark}, {next}],"));
    }
    driver.push_str("]);\n");
    // hyperframes 播放器协议：页面把整帧重绘挂在 `hf-seek` 上（liquid 的
    // window.addEventListener('hf-seek')）。引擎按段 seek 完毕后派发一次，
    // detail.time = 当前主时间。将来归 hyperframes 适配层。
    driver.push_str(
        "    try { globalThis.dispatchEvent(new CustomEvent('hf-seek', { detail: { time: globalThis.__ocSeekT || 0 } })); } catch (e) {}\n",
    );
    driver.push_str("})();");
    Ok(driver)
}

/// 模块图装载：`<script type="module">` 的入口逐个 BFS——每模块过 oxc
/// 降级（import → `__ocRequire`、export → `module.exports`，jsx 通道同一
/// 前端），`import.meta` 替换为按模块 key 构造的对象字面量。窄语料：liquid
/// 只用 `import.meta.url` 取模块相对资产；classic 语境里 `import.meta` 是
/// early SyntaxError，必须替换（将来要全量 import.meta 语义再上 oxc VisitMut）。
/// specifier 解析：importmap 命中 → 相对 base_dir 目标；`./`/`../` 相对 →
/// 自模块 key 所在目录；其余（内建/远程）留给运行时 resolveSpec 或报错。
fn load_html_modules(
    entries: &[(usize, &HtmlScript)],
    base_dir: &Path,
    import_map: Option<&str>,
) -> Result<(serde_json::Map<String, Value>, Vec<String>)> {
    let mut mapped: HashMap<String, String> = HashMap::new();
    if let Some(json) = import_map
        && let Ok(v) = serde_json::from_str::<Value>(json)
        && let Some(imports) = v.get("imports").and_then(|i| i.as_object())
    {
        for (k, val) in imports {
            if let Some(t) = val.as_str() {
                mapped.insert(k.clone(), t.to_string());
            }
        }
    }
    let mut sources: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut require_stmts: Vec<String> = Vec::new();
    let mut queue: std::collections::VecDeque<(String, String)> =
        std::collections::VecDeque::new();
    for (slot, (_, script)) in entries.iter().enumerate() {
        let (key, source) = match &script.src {
            Some(src) => (module_key(src), resolve_external(base_dir, src)?),
            None => (
                format!("__inline_module_{slot}.mjs"),
                script.inline.clone().unwrap_or_default(),
            ),
        };
        require_stmts.push(format!(
            "globalThis.__ocRequire({});",
            serde_json::to_string(&key)?
        ));
        queue.push_back((key, source));
    }
    while let Some((key, source)) = queue.pop_front() {
        if sources.contains_key(&key) {
            continue;
        }
        let transformed = opencat_core::parse::jsx_transform::transform(&source)?;
        let meta_url = serde_json::to_string(&key)?;
        let code = transformed
            .code
            .replace("import.meta", &format!("({{ url: {meta_url} }})"));
        // 产物的 require 保留原始 specifier（"three"、"./x.js"）；运行时
        // resolveSpec 的 sources 键是文件路径——逐条重写为解析后的键，两边
        // 才接得上（oxc codegen 双引号形态，替换前后串正好对应）。
        let mut code = code;
        for spec in &transformed.requires {
            let target = resolve_module_spec(spec, &key, &mapped);
            let req_key = target
                .as_ref()
                .map(|t| module_key(t))
                .unwrap_or_else(|| spec.clone());
            let from = format!("__ocRequire(\"{spec}\")");
            let to = format!("__ocRequire(\"{req_key}\")");
            code = code.replace(&from, &to);
            if let Some(t) = target {
                let next_key = module_key(&t);
                let text = resolve_external(base_dir, &t)
                    .with_context(|| format!("html: module `{spec}` (from `{key}`)"))?;
                queue.push_back((next_key, text));
            }
        }
        sources.insert(key.clone(), Value::String(code));
    }
    Ok((sources, require_stmts))
}

/// spec → 文件目标（相对 base_dir 的路径）：importmap 命中优先（three →
/// ./node_modules/...），否则相对 from 模块目录。远程与非相对内建返回
/// None（运行时 resolveSpec 兜底）。
fn resolve_module_spec(
    spec: &str,
    from_key: &str,
    mapped: &HashMap<String, String>,
) -> Option<String> {
    let spec = spec.trim();
    if !spec.starts_with('.') {
        return mapped.get(spec).cloned();
    }
    let dir = match from_key.rfind('/') {
        Some(p) => &from_key[..p],
        None => "",
    };
    Some(if dir.is_empty() {
        spec.to_string()
    } else {
        format!("{dir}/{spec}")
    })
}

/// 路径 → 模块 key：POSIX 分隔、`./`/`../` 归一。JSX_RUNTIME 的 resolveSpec
/// 以 `__ocModuleSources` 键直接命中，key 稳定性即缓存命中的前提。
fn module_key(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let posix = path.replace('\\', "/");
    for seg in posix.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}
