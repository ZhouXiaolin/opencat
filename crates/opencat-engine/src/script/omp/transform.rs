//! `@omp` 提取期变换（oxc 前端）：标注识别 → ir 对象 + PRNG 采集骨架 → 守卫包裹。
//!
//! 标注是零语义注释（asm.js / esbuild `@__PURE__` 同族）——Chrome 直跑
//! 语料行为不变；守卫只存在于 opencat 变换后的 driver 里。任何解析失败
//! 都原样返回（不变换 = 原循环照跑，「pragma 未被编译器处理」降级语义）。
//!
//! 前端 = oxc parser + semantic（取代手写白名单解析器）：标注注释按
//! span 定位，自由标识符按 ScopeTree 的 symbol 声明位置精确判定（循环
//! 内声明 = 局部，声明在循环外 = 自由 capture），页面函数按引用解析的
//! symbol 抽取（span 级切源码，shadow 语义精确，支持面 = 完整 JS）。
//!
//! 发射形态：
//! ```js
//! if (!(typeof __ocOmpTake === 'function'
//!       && globalThis.__ocOmpPrep(<irobj>)
//!       && __ocOmpTake(<irobj>, <args...>, <skeleton>))) {
//!   <原循环逐字保留>
//! }
//! ```
//! 骨架（采集 IIFE）只作为 take 的实参求值——`&&` 短路保证 prep 失败时
//! 骨架不跑（PRNG 不被无谓消耗，fallback 语义纯净）。骨架 = 两遍头部
//! 重放：count 遍数内层迭代数，fill 遍按原序 `out[w++] = random()` 批量
//! 预采——页面自己的 PRNG 原样跑，逐位一致天然成立。

use std::borrow::Cow;

use serde_json::Value as JsonValue;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    AssignmentOperator, AssignmentTarget, Expression, ForStatement, FunctionType, Program,
    SimpleAssignmentTarget, Statement,
};
use oxc_ast::{AstKind, CommentKind};
use oxc_parser::Parser;
use oxc_semantic::{IsGlobalReference, Scoping, Semantic, SemanticBuilder};
use oxc_span::{GetSpan, SourceType, Span};

/// span → 源文本切片（Span 的端点是 u32）。
fn span_text<'s>(source: &'s str, s: Span) -> &'s str {
    &source[s.start as usize..s.end as usize]
}

// ── 子句解析（纯文本面，与 AST 无关） ────────────────────────────────

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// for 头部 `(...)` 的配对（引号/注释感知）——子句名单解析用。
fn matching_paren(src: &str, open: usize) -> Option<usize> {
    let b = src.as_bytes();
    let mut depth = 0usize;
    let mut i = open + 1;
    while i < b.len() {
        match b[i] {
            b'/' if i + 1 < b.len() && (b[i + 1] == b'/' || b[i + 1] == b'*') => {
                if b[i + 1] == b'/' {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                } else {
                    i += 2;
                    while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'\'' | b'"' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'(' => depth += 1,
            b')' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 名单子句：`(a, b, c)` 内逗号分隔纯标识符。空名单合法（OpenMP 允许）。
fn parse_clause_list(inner: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for item in inner.split(',') {
        let name = item.trim();
        if name.is_empty() {
            if inner.trim().is_empty() {
                continue;
            }
            return None;
        }
        if !name.bytes().all(is_ident_byte) || name.as_bytes()[0].is_ascii_digit() {
            return None;
        }
        if out.iter().any(|n| n == name) {
            return None; // 同一子句内重复名单项
        }
        out.push(name.to_string());
    }
    Some(out)
}

/// OpenMP 数据共享子句面。未列出的外层变量默认 shared——`default_none`
/// 打开时改为「必须显式列出」（default(none)）。运行形态子句
/// （schedule/collapse/num_threads）原文记入 ir 供 runtime 映射；本实现
/// 不跟踪的子句在解析期已整条拒绝。
#[derive(Default)]
pub(crate) struct Clauses {
    shared: Vec<String>,
    firstprivate: Vec<String>,
    private: Vec<String>,
    /// `reduction(op: list)`（保持出现序；op ∈ `+` `*`）。
    reductions: Vec<(String, Vec<String>)>,
    default_none: bool,
    /// `schedule(kind[, chunk])` 原文（规范化后，如 "dynamic,1"）。
    schedule: Option<String>,
    collapse: Option<u64>,
    num_threads: Option<u64>,
    /// simd 构造（`parallel for simd` / `for simd`）：`for simd`（无
    /// parallel）按 TR15 是串行单元+向量提示，实现上映射同一后端——行独
    /// 立循环的行并行与串行字节确定等价（docs/js-openmp.md §2 收紧说明）。
    simd: bool,
}

/// 解析标注行的子句序列（子句间空白或可选逗号分隔）。遇到本实现不支持的
/// 正式子句（lastprivate/linear/ordered/if/nowait/…）或任何非法形态返回
/// None——整条标注不处理（「pragma 未被编译器处理」语义）。
fn parse_clauses(text: &str) -> Option<Clauses> {
    let b = text.as_bytes();
    let mut i = 0usize;
    let mut out = Clauses::default();
    let mut seen_default = false;
    // name → 数据共享类别（1=shared 2=firstprivate 3=private），防同名字
    // 跨类别重复（OpenMP：一个名单项不得出现在两个子句里）。
    let mut kinds: Vec<(String, u8)> = Vec::new();
    let mut push_names = |kinds: &mut Vec<(String, u8)>, kind: u8, slot: &mut Vec<String>, names: Vec<String>| -> Option<()> {
        for n in names {
            if let Some((_, k)) = kinds.iter().find(|(x, _)| *x == n) {
                if *k != kind {
                    return None; // 跨类别重复
                }
                return Some(()); // 同类别重复子句合法（规范允许复现子句）
            }
            kinds.push((n.clone(), kind));
            slot.push(n);
        }
        Some(())
    };
    while i < b.len() {
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b',' {
            i += 1; // 子句间可选逗号
            continue;
        }
        if i >= b.len() {
            break;
        }
        if !is_ident_byte(b[i]) || b[i].is_ascii_digit() {
            return None;
        }
        let name_start = i;
        while i < b.len() && is_ident_byte(b[i]) {
            i += 1;
        }
        let name = &text[name_start..i];
        // 可选 (…) 参数（子句名与括号间允许空白）。
        let mut j = i;
        while j < b.len() && (b[j] as char).is_whitespace() {
            j += 1;
        }
        let mut inner: Option<&str> = None;
        if j < b.len() && b[j] == b'(' {
            let close = matching_paren(text, j)?;
            inner = Some(&text[j + 1..close]);
            i = close + 1;
        }
        match name {
            "shared" | "firstprivate" | "private" => {
                let names = parse_clause_list(inner?)?;
                let (kind, slot) = match name {
                    "shared" => (1u8, 0usize),
                    "firstprivate" => (2u8, 1usize),
                    _ => (3u8, 2usize),
                };
                let slot = match slot {
                    0 => &mut out.shared,
                    1 => &mut out.firstprivate,
                    _ => &mut out.private,
                };
                push_names(&mut kinds, kind, slot, names)?;
            }
            "default" => {
                if seen_default {
                    return None;
                }
                seen_default = true;
                match inner?.trim() {
                    "shared" => {}
                    "none" => out.default_none = true,
                    _ => return None, // default(private/firstprivate)：5.1+，不支持
                }
            }
            // 运行形态子句：校验语法，原文记入 ir（runtime 映射 schedule；
            // collapse/num_threads 是请求非承诺，结果与调度无关）。
            "schedule" => {
                let s = inner?;
                if s.contains(':') {
                    return None; // monotonic:/nonmonotonic:/simd: 修饰符不支持
                }
                let mut parts = s.split(',');
                let kind = parts.next().unwrap_or("").trim();
                if !matches!(kind, "static" | "dynamic" | "guided" | "auto" | "runtime") {
                    return None;
                }
                let mut norm = kind.to_string();
                if let Some(chunk) = parts.next() {
                    let chunk = chunk.trim();
                    match chunk.parse::<u64>() {
                        Ok(v) if v > 0 => norm = format!("{kind},{v}"),
                        _ => return None,
                    }
                }
                if parts.next().is_some() || out.schedule.replace(norm).is_some() {
                    return None;
                }
            }
            "reduction" => {
                let s = inner?;
                let Some((op_raw, list_raw)) = s.split_once(':') else {
                    return None; // TR15 语法 reduction(op: list)，无冒号 = 非法
                };
                let op = op_raw.trim().to_string();
                if !matches!(op.as_str(), "+" | "*") {
                    return None; // v1：+ 与 *（docs/js-openmp.md §3.4）
                }
                let names = parse_clause_list(list_raw.trim())?;
                if names.is_empty() {
                    return None;
                }
                // reduction 变量与数据共享类别互斥（区域内是私有副本）。
                for n in &names {
                    if kinds.iter().any(|(x, _)| x == n) {
                        return None;
                    }
                    kinds.push((n.clone(), 4));
                }
                if out.reductions.iter().any(|(o, _)| *o == op) {
                    // 同 op 子句合并（规范允许复现子句：reduction(+:a) reduction(+:b)）
                    let entry = out.reductions.iter_mut().find(|(o, _)| *o == op).unwrap();
                    entry.1.extend(names);
                } else {
                    out.reductions.push((op, names));
                }
            }
            "collapse" | "num_threads" => {
                let v = inner?.trim();
                let Ok(n) = v.parse::<u64>() else { return None };
                if n == 0 {
                    return None;
                }
                if name == "collapse" {
                    if out.collapse.replace(n).is_some() {
                        return None;
                    }
                } else if out.num_threads.replace(n).is_some() {
                    return None;
                }
            }
            // 已认识的正式子句但本实现不支持 → 整条标注不处理。
            "lastprivate" | "linear" | "ordered" | "if" | "allocate" | "order"
            | "proc_bind" | "copyin" | "nowait" => return None,
            _ => return None,
        }
    }
    Some(out)
}

// ── oxc 分析层 ──────────────────────────────────────────────────────

/// JS 全局内建（unresolved 引用不算 capture——它们不来自页面作用域，
/// JSON 解码函数值会毒化条目）。
const JS_GLOBAL_BUILTINS: &[&str] = &[
    // 值属性
    "undefined", "NaN", "Infinity", "globalThis",
    // 基本对象
    "Object", "Function", "Boolean", "Symbol", "Error", "EvalError", "RangeError",
    "ReferenceError", "SyntaxError", "TypeError", "URIError", "AggregateError",
    // 数字与日期
    "Number", "BigInt", "Math", "Date",
    // 文本
    "String", "RegExp",
    // 索引集合
    "Array", "Int8Array", "Uint8Array", "Uint8ClampedArray", "Int16Array", "Uint16Array",
    "Int32Array", "Uint32Array", "Float32Array", "Float64Array", "BigInt64Array",
    "BigUint64Array",
    // 键集合
    "Map", "Set", "WeakMap", "WeakSet", "WeakRef", "FinalizationRegistry",
    // 控制抽象
    "Promise", "Iterator", "AsyncIterator", "Generator", "AsyncGenerator", "Reflect",
    "Proxy",
    // 反射与结构化数据
    "JSON", "Atomics",
    // 浏览器/驱动环境常见全局
    "document", "window", "navigator", "location", "performance", "console",
    "requestAnimationFrame", "cancelAnimationFrame", "setTimeout", "setInterval",
    "clearTimeout", "clearInterval", "requestIdleCallback", "cancelIdleCallback", "alert",
    "fetch", "URL", "URLSearchParams", "TextEncoder", "TextDecoder", "AbortController",
    "Event", "CustomEvent", "Image", "ImageData", "Path2D", "DOMParser", "XMLSerializer",
    "screen", "history", "localStorage", "sessionStorage",
    // 函数属性
    "parseInt", "parseFloat", "isNaN", "isFinite", "decodeURI", "decodeURIComponent",
    "encodeURI", "encodeURIComponent", "eval", "queueMicrotask", "structuredClone",
    "crypto", "btoa", "atob", "require", "module", "exports",
];

/// 一处标注的分析产物（全 owned，AST 借用不越出分析作用域）。
pub(crate) struct LoopSite {
    pub clauses: Clauses,
    /// 外层 for 语句完整源码（含 body）。
    pub loop_src: String,
    /// 外层 for 的 [start, end]（AST span，end 含 `}`）。
    pub loop_start: usize,
    pub loop_end: usize,
    /// 各层 (header 文本 = 括号内, body 文本 = 花括号内)。
    pub levels: Vec<(String, String)>,
    /// 精确自由标识符（loop span + 页面函数闭包并集，保序去重）。
    pub free: Vec<String>,
    /// 抽取的页面函数（name, 源码）。
    pub fn_srcs: Vec<(String, String)>,
    /// prng 候选（零参裸调用且无同源声明，保序）。
    pub prng_candidates: Vec<String>,
    /// innermost body 的零参调用出现序（重复名字每次出现各计一次）。
    pub prng_inner_occ: Vec<String>,
    /// loop span 内 ctx 用法名（成员调用接收者 / fillStyle 赋值目标）。
    pub ctx_names: Vec<String>,
    /// 标注行尾（含 @omp 注释整行的 `\n` 前），失败保留用。
    pub annotation_line_end: usize,
    /// for 所在行行首（缩进起点）。
    pub indent_start: usize,
    /// 外层 header 文本（`let y = 0; y < h; y += 1` 形态）。
    pub outer_header: String,
    /// 外层循环变量名。
    pub loop_var: String,
    /// 外层体（Set/Acc 文本级改写后，bundle 行函数体）。
    pub row_body: String,
    /// 数组写回目标名（`c[i] = / c[i] +=` 的 c），首次出现序。
    pub array_names: Vec<String>,
}

/// oxc 一次 parse + semantic，产出全部标注位。解析失败 = None（调用方
/// identity 返回——「pragma 未被编译器处理」）。
fn analyze(source: &str) -> Option<Vec<LoopSite>> {
    let allocator = Allocator::new();
    let parsed = Parser::new(&allocator, source, SourceType::jsx()).parse();
    if parsed.diagnostics.has_errors() {
        return None;
    }
    let program = parsed.program;
    // 0.153 的 new() 只建 scoping 不建节点库（AstNodes 恒空），for 定位与
    // AstKind 遍历都需要节点库，必须显式开启。
    let semantic = SemanticBuilder::new().with_build_nodes(true).build(&program).semantic;

    // 标注注释：Line 注释、文本含 construct、独立一行（`//` 与 `@omp`
    // 之间、行首到 `//` 之间都只允许空白）。comments 按源码序。
    let mut sites = Vec::new();
    for comment in &program.comments {
        if comment.kind != CommentKind::Line {
            continue;
        }
        let cspan = comment.span;
        let text = span_text(source, cspan);
        // construct 面（TR15 §6/§11）：`@omp parallel for [simd]` 与
        // `@omp for simd`。裸 `@omp for` 不是独立合法形态，不处理。
        let Some((rel, clauses_text, simd_flag)) = find_construct(text) else {
            continue;
        };
        if text[..rel].trim() != "//" {
            continue;
        }
        let line_start = source[..cspan.start as usize].rfind('\n').map_or(0, |p| p + 1);
        if !source[line_start..cspan.start as usize].trim().is_empty() {
            continue;
        }
        let Some(mut clauses) = parse_clauses(clauses_text) else {
            continue;
        };
        clauses.simd = simd_flag;
        let Some(fs) = find_next_for(&semantic, cspan.end as usize, source) else {
            continue;
        };
        let annotation_line_end = source[cspan.end as usize..]
            .find('\n')
            .map_or(source.len(), |p| cspan.end as usize + p);
        let Some(site) =
            build_site(source, &program, &semantic, fs, clauses, annotation_line_end)
        else {
            continue;
        };
        sites.push(site);
    }
    Some(sites)
}

/// construct 识别：返回 (construct 起点字节, 子句文本, 是否 simd)。
/// `@omp parallel for [simd]` / `@omp for simd`；两个字面量互不前缀包含，
/// 无重叠歧义。simd 是 construct 尾词（后随空白/逗号/行尾），词间空白
/// 允许。
fn find_construct(text: &str) -> Option<(usize, &str, bool)> {
    let rel_p = text.find("@omp parallel for");
    let rel_s = text.find("@omp for simd");
    match (rel_p, rel_s) {
        (None, None) => None,
        (None, Some(s)) => Some((s, &text[s + "@omp for simd".len()..], true)),
        (Some(p), s) => {
            if let Some(s) = s {
                if s < p {
                    return Some((s, &text[s + "@omp for simd".len()..], true));
                }
            }
            let rest = &text[p + "@omp parallel for".len()..];
            // 可选 simd 尾词（词边界：后随空白/逗号/行尾）。
            let ws = rest.len() - rest.trim_start().len();
            if let Some(tail) = rest[ws..].strip_prefix("simd") {
                if tail.is_empty() || tail.starts_with(|c: char| c.is_whitespace() || c == ',') {
                    return Some((p, &rest[ws + 4..], true));
                }
            }
            Some((p, rest, false))
        }
    }
}

/// comment 之后的第一个 ForStatement，二者之间源文本全空白。
fn find_next_for<'a>(
    semantic: &Semantic<'a>,
    after: usize,
    source: &str,
) -> Option<&'a ForStatement<'a>> {
    let mut best: Option<&'a ForStatement<'a>> = None;
    for node in semantic.nodes().iter() {
        let AstKind::ForStatement(fs) = node.kind() else {
            continue;
        };
        if (fs.span.start as usize) < after {
            continue;
        }
        let closer = match best {
            Some(b) => fs.span.start < b.span().start,
            None => true,
        };
        if closer {
            best = Some(fs);
        }
    }
    let fs = best?;
    let between = &source[after..fs.span.start as usize];
    if !between.trim().is_empty() {
        return None;
    }
    Some(fs)
}

/// 内层可重放的 canonical for 巢：各层 (header, body) 文本。
/// init/test/update 缺失或 body 非 block 或 for..in/of → 拒（非 canonical）。
fn collect_levels(
    source: &str,
    fs: &ForStatement<'_>,
    out: &mut Vec<(String, String)>,
) -> bool {
    let (Some(init), Some(test), Some(update)) = (&fs.init, &fs.test, &fs.update) else {
        return false;
    };
    let Statement::BlockStatement(block) = &fs.body else {
        return false;
    };
    let header = source[init.span().start as usize..update.span().end as usize].to_string();
    let body = source
        [block.span.start as usize + 1..block.span.end as usize - 1]
        .to_string();
    out.push((header, body));
    // 下一层：体语句里的第一个 for（其间语句 = intervening code，canonical
    // form 允许）；for..in/of 出现在同一层 → 拒。
    let mut next: Option<&ForStatement<'_>> = None;
    for stmt in &block.body {
        match stmt {
            Statement::ForStatement(inner) => {
                if next.is_none() {
                    next = Some(inner);
                }
            }
            Statement::ForInStatement(_) | Statement::ForOfStatement(_) => return false,
            _ => {}
        }
    }
    match next {
        Some(inner) => collect_levels(source, inner, out),
        None => true,
    }
}

fn span_within(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn push_unique(out: &mut Vec<String>, name: &str) {
    if !out.iter().any(|n| n == name) {
        out.push(name.to_string());
    }
}

/// 范围内引用且声明在范围外的标识符（精确自由集）。unresolved 引用
/// （跨 script 全局）除 JS 内建外同样算自由——capture 后由 take 的
/// args 逐帧传入。
fn free_idents_within(
    semantic: &Semantic<'_>,
    scoping: &Scoping,
    span: Span,
) -> Vec<String> {
    let mut out = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::IdentifierReference(ident) = node.kind() else {
            continue;
        };
        if !span_within(span, ident.span) {
            continue;
        }
        if ident.is_global_reference(scoping) {
            if !JS_GLOBAL_BUILTINS.contains(&ident.name.as_str()) {
                push_unique(&mut out, &ident.name);
            }
            continue;
        }
        let resolved = ident
            .reference_id
            .get()
            .and_then(|rid| scoping.get_reference(rid).symbol_id());
        let Some(sym) = resolved else { continue };
        let decl_span = scoping.symbol_span(sym);
        if !span_within(span, decl_span) {
            push_unique(&mut out, &ident.name);
        }
    }
    out
}

/// 范围内 callee 为裸标识符的调用名（保序去重）。
fn bare_call_names(semantic: &Semantic<'_>, span: Span) -> Vec<String> {
    let mut out = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !span_within(span, call.span) {
            continue;
        }
        if let Expression::Identifier(ident) = &call.callee {
            push_unique(&mut out, &ident.name);
        }
    }
    out
}

/// 范围内零参调用出现序（重复名字每次出现各计一次——骨架按序预采）。
fn zero_arg_calls(semantic: &Semantic<'_>, span: Span) -> Vec<String> {
    let mut out = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !span_within(span, call.span) || !call.arguments.is_empty() {
            continue;
        }
        if let Expression::Identifier(ident) = &call.callee {
            out.push(ident.name.to_string());
        }
    }
    out
}

/// 范围内 ctx 用法名：成员调用接收者或 fillStyle 赋值目标。成员**读**
/// 不算（`s.phase` 是普通对象成员读，误分类会因解码缺 `__ocNodeId` 毒化）。
/// 排除两类：内建引用（`Math.sqrt()` 的 Math 在 worker 原生可解析）、
/// span 内声明的局部量（`alpha.toFixed(3)` 的 alpha 是行内数字）。
fn ctx_user_names(semantic: &Semantic<'_>, scoping: &Scoping, span: Span) -> Vec<String> {
    let mut out = Vec::new();
    let mut is_ctx_receiver = |id: &oxc_ast::ast::IdentifierReference<'_>| -> bool {
        if id.is_global_reference(scoping) {
            return !JS_GLOBAL_BUILTINS.contains(&id.name.as_str());
        }
        let local = id
            .reference_id
            .get()
            .and_then(|rid| scoping.get_reference(rid).symbol_id())
            .map(|sym| span_within(span, scoping.symbol_span(sym)))
            .unwrap_or(false);
        !local
    };
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::CallExpression(call) => {
                if !span_within(span, call.span) {
                    continue;
                }
                if let Expression::StaticMemberExpression(sme) = &call.callee {
                    if let Expression::Identifier(id) = &sme.object {
                        if is_ctx_receiver(id) {
                            push_unique(&mut out, &id.name);
                        }
                    }
                }
            }
            AstKind::AssignmentExpression(ae) => {
                if !span_within(span, ae.span) || ae.operator != AssignmentOperator::Assign {
                    continue;
                }
                if let AssignmentTarget::StaticMemberExpression(sme) = &ae.left {
                    if sme.property.name.as_str() == "fillStyle" {
                        if let Expression::Identifier(id) = &sme.object {
                            if is_ctx_receiver(id) {
                                push_unique(&mut out, &id.name);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// loop span 内对 `name` 的引用所解析到的声明 span（无解析引用 = None）。
fn resolved_decl_spans(
    semantic: &Semantic<'_>,
    scoping: &Scoping,
    span: Span,
    name: &str,
) -> Vec<Span> {
    let mut out = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::IdentifierReference(ident) = node.kind() else {
            continue;
        };
        if ident.name.as_str() != name || !span_within(span, ident.span) {
            continue;
        }
        if ident.is_global_reference(scoping) {
            continue;
        }
        let Some(rid) = ident.reference_id.get() else { continue };
        let Some(sym) = scoping.get_reference(rid).symbol_id() else { continue };
        out.push(scoping.symbol_span(sym));
    }
    out
}

/// 组装一处标注的分析产物。失败 = None（该处不变换）。
fn build_site(
    source: &str,
    program: &Program<'_>,
    semantic: &Semantic<'_>,
    fs: &ForStatement<'_>,
    clauses: Clauses,
    annotation_line_end: usize,
) -> Option<LoopSite> {
    let scoping = semantic.scoping();
    let loop_span = fs.span;
    let mut levels = Vec::new();
    if !collect_levels(source, fs, &mut levels) {
        return None;
    }

    // 内层 body span（零参骨架采集只认内层——外层调用会破坏每行采集数
    // 均一）。逐层 span 重算：由 header/body 文本在外层源中定位太绕，直接
    // 顺着 AST 再走一遍拿最后一层的 block span。
    let inner_span = innermost_body_span(fs)?;

    let mut free = free_idents_within(semantic, scoping, loop_span);
    let mut fn_srcs: Vec<(String, String)> = Vec::new();
    let mut prng_candidates: Vec<String> = Vec::new();

    // 页面函数抽取队列：(名字, 引用所在 span)。初始 = loop 内裸调用；
    // 闭包发现的调用带所属函数 span——传递函数（如 radiance→intersect）
    // 在 loop 里未必被直接引用，解析必须回到引用处。
    let mut queue: Vec<(String, Span)> = bare_call_names(semantic, loop_span)
        .into_iter()
        .map(|n| (n, loop_span))
        .collect();
    let zero_arg_in_loop = zero_arg_calls(semantic, loop_span);
    while let Some((name, ref_span)) = queue.first().cloned() {
        queue.remove(0);
        if fn_srcs.iter().any(|(n, _)| *n == name) || prng_candidates.contains(&name) {
            continue;
        }
        // loop 内零参裸调用 = 页面 PRNG：预采协议（骨架在主 realm 按消费
        // 序采样，页面函数原样跑），永不抽取——worker 里跑自带状态会破坏
        // 行序确定性。
        if ref_span == loop_span && zero_arg_in_loop.iter().any(|n| n == &name) {
            push_unique(&mut prng_candidates, &name);
            continue;
        }
        // 按引用解析的 symbol 找声明（shadow 精确）：必须是 loop 外的
        // FunctionDeclaration（箭头函数赋值等形态 v1 不抽取——行为对齐
        // 旧实现，capture plain 由上层降级）。symbol_span 是绑定标识符的
        // span，抽取单元（源码/闭包遍历）按所属 FunctionDeclaration 整 span。
        let decl = resolved_decl_spans(semantic, scoping, ref_span, &name)
            .into_iter()
            .find(|d| !span_within(loop_span, *d))
            .and_then(|d| function_decl_span(semantic, d));
        let Some(decl) = decl else {
            continue;
        };
        let src = span_text(source, decl).to_string();
        // 传递闭包：函数体内的自由标识符并入 free；体内裸调用名进队。
        let fn_span = Span::new(decl.start, decl.end);
        for f in free_idents_within(semantic, scoping, fn_span) {
            push_unique(&mut free, &f);
        }
        for callee in bare_call_names(semantic, fn_span) {
            if !queue.iter().any(|(n, _)| *n == callee) {
                queue.push((callee, fn_span));
            }
        }
        push_unique_name(&mut fn_srcs, name, src);
        if fn_srcs.len() >= 64 {
            break;
        }
    }
    // 抽成函数的名字不再是 capture（函数源码随 bundle 走）。
    let fn_names: Vec<&str> = fn_srcs.iter().map(|(n, _)| n.as_str()).collect();
    free.retain(|n| !fn_names.contains(&n.as_str()));

    let prng_inner_occ: Vec<String> = zero_arg_calls(semantic, inner_span)
        .into_iter()
        .filter(|n| prng_candidates.contains(n))
        .collect();
    let ctx_names = ctx_user_names(semantic, scoping, loop_span);

    // 数组写回面：`name[expr] = v` / `name[expr] += v` → bundle 行函数内
    // 改写为 Set/Acc 记录调用（行缓冲 + host 按行序应用；数组 copy-in 零
    // 字节）。限制（如实拒绝整条标注）：`++`/`--` 同族、其余复合写
    // （`-=` 等）、对写回数组的任何下标**读**（token 不进 worker，读了
    // 拿 undefined，宁可诚实降级）。
    let mut array_writes: Vec<(Span, Span, Span, String, bool)> = Vec::new();
    let mut write_obj_spans: Vec<Span> = Vec::new();
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::AssignmentExpression(ae) => {
                if !span_within(loop_span, ae.span) {
                    continue;
                }
                let AssignmentTarget::ComputedMemberExpression(cme) = &ae.left else {
                    continue;
                };
                let Expression::Identifier(id) = &cme.object else {
                    continue;
                };
                let is_acc = ae.operator == AssignmentOperator::Addition;
                if !is_acc && ae.operator != AssignmentOperator::Assign {
                    return None;
                }
                write_obj_spans.push(id.span);
                array_writes.push((
                    ae.span,
                    cme.expression.span(),
                    match &ae.right {
                        Expression::SequenceExpression(seq) => seq.span(),
                        expr => expr.span(),
                    },
                    id.name.to_string(),
                    is_acc,
                ));
            }
            AstKind::UpdateExpression(ue) => {
                if !span_within(loop_span, ue.span) {
                    continue;
                }
                if let SimpleAssignmentTarget::ComputedMemberExpression(cme) = &ue.argument {
                    if matches!(&cme.object, Expression::Identifier(_)) {
                        return None;
                    }
                }
            }
            _ => {}
        }
    }
    let mut array_names: Vec<String> = Vec::new();
    for (_, _, _, name, _) in &array_writes {
        push_unique(&mut array_names, name);
    }
    for name in &array_names {
        for node in semantic.nodes().iter() {
            let AstKind::IdentifierReference(id) = node.kind() else {
                continue;
            };
            if id.name.as_str() != name
                || !span_within(loop_span, id.span)
                || write_obj_spans.contains(&id.span)
            {
                continue;
            }
            return None; // 下标读
        }
    }
    // Set/Acc 文本级改写（外层体拷贝上按相对 span 降序替换——头无写回，
    // 体块起点不受平移影响）。
    let Statement::BlockStatement(outer_block) = &fs.body else {
        return None;
    };
    let body_start = outer_block.span.start as usize + 1;
    let body_end = outer_block.span.end as usize - 1;
    let mut row_body = source[body_start..body_end].to_string();
    let mut edits: Vec<(usize, usize, String)> = array_writes
        .iter()
        .map(|(span, expr, val, name, is_acc)| {
            let fname = if *is_acc { "__ocOmpAcc" } else { "__ocOmpSet" };
            (
                span.start as usize - body_start,
                span.end as usize - body_start,
                format!(
                    "{fname}({name}, {}, {})",
                    span_text(source, *expr),
                    span_text(source, *val)
                ),
            )
        })
        .collect();
    edits.sort_by(|a, b| b.0.cmp(&a.0));
    for (s, e, txt) in edits {
        row_body.replace_range(s..e, &txt);
    }
    // 外层 header 与循环变量名（canonical 校验保证 `let <name> = …` 形态）。
    let outer_header = levels[0].0.clone();
    let loop_var = outer_header
        .trim()
        .strip_prefix("let ")
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or_default()
        .to_string();

    Some(LoopSite {
        clauses,
        loop_src: span_text(source, loop_span).to_string(),
        loop_start: loop_span.start as usize,
        loop_end: loop_span.end as usize,
        levels,
        free,
        fn_srcs,
        prng_candidates,
        prng_inner_occ,
        ctx_names,
        annotation_line_end,
        indent_start: source[..loop_span.start as usize]
            .rfind('\n')
            .map_or(0, |p| p + 1),
        outer_header,
        loop_var,
        row_body,
        array_names,
    })
}

fn push_unique_name(out: &mut Vec<(String, String)>, name: String, src: String) {
    if !out.iter().any(|(n, _)| n == &name) {
        out.push((name, src));
    }
}

/// 绑定标识符 span 所属的 FunctionDeclaration：只有当 decl 恰是某函数
/// 声明的**名字标识符**时才成立（symbol_span 对 FunctionDeclaration 就是
/// 名字 span；const 绑定落在某外层函数体内不算——不能把外层函数整个
/// 抽走）。返回整函数 span。
fn function_decl_span(semantic: &Semantic<'_>, decl: Span) -> Option<Span> {
    for node in semantic.nodes().iter() {
        let AstKind::Function(f) = node.kind() else {
            continue;
        };
        if f.r#type != FunctionType::FunctionDeclaration {
            continue;
        }
        if let Some(id) = &f.id {
            if id.span == decl {
                return Some(node.span());
            }
        }
    }
    None
}

/// 顺着 AST 拿最内层 body 的 span（canonical 巢已验证过）。
fn innermost_body_span(fs: &ForStatement<'_>) -> Option<Span> {
    let mut cur = fs;
    loop {
        let Statement::BlockStatement(block) = &cur.body else {
            return None;
        };
        let mut next: Option<&ForStatement<'_>> = None;
        for stmt in &block.body {
            if let Statement::ForStatement(inner) = stmt {
                next = Some(inner);
                break;
            }
        }
        match next {
            Some(inner) => cur = inner,
            None => return Some(block.span),
        }
    }
}

fn build_skeleton(levels: &[(String, String)], prng_calls: &[String]) -> String {
    let mut count = String::from("var __ocOmpN = 0; ");
    let mut fill = String::from("var __ocOmpW = 0; ");
    for (header, _) in levels {
        count.push_str(&format!("for ({header}) {{ "));
        fill.push_str(&format!("for ({header}) {{ "));
    }
    count.push_str("__ocOmpN += 1; ");
    for c in prng_calls {
        fill.push_str(&format!("__ocOmpOut[__ocOmpW++] = {c}(); "));
    }
    let close = "} ".repeat(levels.len());
    count.push_str(&close);
    fill.push_str(&close);
    format!(
        "(function () {{ {count} var __ocOmpOut = new Float64Array(__ocOmpN); {fill} return __ocOmpOut; }})()"
    )
}

// ── 变换主流程 ──────────────────────────────────────────────────────

/// 语料文本的 @omp 变换。无标注时零拷贝返回；任何解析失败只跳过该处
/// 标注（原样保留，循环照跑）。
pub fn transform(source: &str) -> Cow<'_, str> {
    if !source.contains("@omp") {
        return Cow::Borrowed(source);
    }
    let Some(sites) = analyze(source) else {
        return Cow::Borrowed(source);
    };
    let mut out = String::with_capacity(source.len() + 1024);
    let mut cursor = 0usize;
    let mut changed = false;
    for site in &sites {
        // 标注行（@omp 注释所在行）的范围：span 前回溯行首、后到行尾。
        // LoopSite 未带注释 span——从 loop_start 回溯不可靠；analyze 里
        // 已把标注行信息吸收进 build_site 失败路径，这里用 site 的守卫
        // 产物定位：annotation 行尾 = loop 前最后一个非空白位置之前的行。
        // 为精确起见 LoopSite 记录 annotation_line_end（见 analyze）。
        let Some((wrapper, else_arm)) = build_wrapper(site) else {
            // 变换失败：标注行原样保留，继续下一处。
            let keep_end = site.annotation_line_end;
            if keep_end > cursor {
                out.push_str(&source[cursor..keep_end]);
                cursor = keep_end;
            }
            continue;
        };
        let indent = source[site.indent_start..site.loop_start].to_string();
        out.push_str(&source[cursor..site.indent_start]);
        out.push_str(&indent);
        out.push_str(&wrapper);
        out.push('\n');
        out.push_str(&source[site.indent_start..=site.loop_end]);
        out.push('\n');
        out.push_str(&indent);
        out.push_str("}\n");
        if let Some(arm) = else_arm {
            out.push_str(&indent);
            out.push_str(&arm);
            out.push('\n');
        }
        cursor = site.loop_end + 1;
        changed = true;
    }
    if !changed {
        return Cow::Borrowed(source);
    }
    out.push_str(&source[cursor..]);
    Cow::Owned(out)
}

/// 标注 → (ir 对象, 骨架表达式)。失败 = None（不变换）。
///
/// capture 名单按 OpenMP 数据共享规则合成：显式 `shared`/`firstprivate`
/// 名单 + 隐式自由标识符（default(shared)，OpenMP 缺省）− `private` 名单。
/// `default(none)` 时隐式名单关闭、每个自由标识符必须显式列出。分类：
/// 循环体成员调用/fillStyle 赋值 → ctx；零参裸调用 → prng；其余 plain。
fn build_ir(site: &LoopSite) -> Option<(JsonValue, String)> {
    let clauses = &site.clauses;
    let mut arg_names: Vec<String> = Vec::new();
    let mut push_cap = |out: &mut Vec<String>, n: &str| {
        if !out.iter().any(|x| x == n) {
            out.push(n.to_string());
        }
    };
    for n in &clauses.shared {
        push_cap(&mut arg_names, n);
    }
    for n in &clauses.firstprivate {
        push_cap(&mut arg_names, n);
    }
    // reduction 变量是行私有副本（TR15 §8.10），不算 capture——隐式分析
    // 与 default(none) 校验都把它从自由标识符里减掉。
    let red_names: Vec<String> = clauses
        .reductions
        .iter()
        .flat_map(|(_, ns)| ns.iter().cloned())
        .collect();
    let is_red = |f: &str| red_names.iter().any(|x| x == f);
    if clauses.default_none {
        // default(none)：每个自由标识符必须落在显式名单（private 也算列出）。
        for f in &site.free {
            if !clauses.shared.contains(f)
                && !clauses.firstprivate.contains(f)
                && !clauses.private.contains(f)
                && !is_red(f)
            {
                return None;
            }
        }
    } else {
        for f in &site.free {
            if !clauses.private.contains(f) && !is_red(f) {
                push_cap(&mut arg_names, f);
            }
        }
    }
    // prng 候选（零参裸调用、无同名声明）进 capture 名单——骨架按序预采。
    for n in &site.prng_candidates {
        push_cap(&mut arg_names, n);
    }

    // capture 分类：数组写回 → array；成员调用/fillStyle 赋值 → ctx；
    // 零参调用 → prng；plain。
    let mut captures = serde_json::Map::new();
    let mut prng_names: Vec<String> = Vec::new();
    for n in &arg_names {
        if site.array_names.iter().any(|a| a == n) {
            captures.insert(n.clone(), JsonValue::String("array".into()));
        } else if site.ctx_names.iter().any(|c| c == n) {
            captures.insert(n.clone(), JsonValue::String("ctx".into()));
        } else if site.prng_candidates.iter().any(|p| p == n) {
            captures.insert(n.clone(), JsonValue::String("prng".into()));
            prng_names.push(n.clone());
        }
    }

    let fns = serde_json::Map::from_iter(
        site.fn_srcs.iter().map(|(n, s)| (n.clone(), JsonValue::String(s.clone()))),
    );

    // 采集骨架（内层零参出现序；无 prng → null）。
    let skeleton = if prng_names.is_empty() {
        "null".to_string()
    } else {
        if site.prng_inner_occ.is_empty() {
            return None;
        }
        build_skeleton(&site.levels, &site.prng_inner_occ)
    };

    let mut fields_todo_reduction: Option<JsonValue> = None;
    // reduction 子句进 ir（[[op, vars...], ...]，展开序 = 折叠槽位序）。
    // isolate 后端通道（行私有副本 + 固定行序左折叠）；S5 起 ctx 循环也
    // 上 isolate，red × ctx 共存合法（垫片 ovals 与 red 槽同行产出）。
    if !clauses.reductions.is_empty() {
        fields_todo_reduction = Some(JsonValue::Array(
            clauses
                .reductions
                .iter()
                .map(|(op, names)| {
                    JsonValue::Array(vec![
                        JsonValue::String(op.clone()),
                        JsonValue::Array(
                            names.iter().map(|n| JsonValue::String(n.clone())).collect(),
                        ),
                    ])
                })
                .collect(),
        ));
    }
    let mut fields = vec![
        ("loop".to_string(), JsonValue::String(site.loop_src.clone())),
        ("fns".to_string(), JsonValue::Object(fns)),
        ("args".to_string(), JsonValue::Array(
            arg_names.iter().map(|n| JsonValue::String(n.clone())).collect(),
        )),
        ("captures".to_string(), JsonValue::Object(captures)),
        // isolate bundle 面（外层 header/行体/循环变量名）：行体已含
        // Set/Acc 改写。
        ("header".to_string(), JsonValue::String(site.outer_header.clone())),
        ("var".to_string(), JsonValue::String(site.loop_var.clone())),
        ("body".to_string(), JsonValue::String(site.row_body.clone())),
    ];
    if let Some(red) = fields_todo_reduction {
        fields.push(("reduction".to_string(), red));
    }
    if !clauses.private.is_empty() {
        fields.push(("private".to_string(), JsonValue::Array(
            clauses.private.iter().map(|n| JsonValue::String(n.clone())).collect(),
        )));
    }
    // 运行形态子句进 ir：schedule 由 runtime 映射到行分块；collapse 必须
    // ≤ 巢深（深了规范就要求合并更多层，本实现只并行最外层 → 拒绝）；
    // num_threads 是线程数请求（OpenMP 语义：实现可少给不可多给）。
    if let Some(s) = &clauses.schedule {
        fields.push(("schedule".to_string(), JsonValue::String(s.clone())));
    }
    if let Some(n) = clauses.collapse {
        if n as usize > site.levels.len() {
            return None;
        }
        fields.push(("collapse".to_string(), JsonValue::Number(n.into())));
    }
    if let Some(n) = clauses.num_threads {
        fields.push(("num_threads".to_string(), JsonValue::Number(n.into())));
    }
    if clauses.simd {
        fields.push(("simd".to_string(), JsonValue::Bool(true)));
    }
    Some((JsonValue::Object(serde_json::Map::from_iter(fields)), skeleton))
}

/// 为一处标注构建守卫头（不含循环体与收尾 `}`）。
fn build_wrapper(site: &LoopSite) -> Option<(String, Option<String>)> {
    let (irobj, skeleton) = build_ir(site)?;
    let irjson = irobj.to_string();
    // capture 名单即 take 实参（求值序 = ir args 序）；无 capture 时不留空位。
    let args_part = if irobj.get("args").and_then(|v| v.as_array()).map_or(true, |a| a.is_empty()) {
        String::new()
    } else {
        let names: Vec<&str> = irobj["args"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        format!("{}, ", names.join(", "))
    };
    // 带 reduction 时 take 返回折叠终值数组（false = 未接管）：头部前置
    // `var __ocOmpRed`（var 提升，严格模式安全），take 实参原样；else 臂
    // 把槽位终值按 ir.reduction 展开序赋回页面变量（行序左折叠的收口面）。
    let red_vars: Vec<String> = irobj
        .get("reduction")
        .and_then(|v| v.as_array())
        .map(|rs| {
            rs.iter()
                .filter_map(|r| r.get(1).and_then(|l| l.as_array()))
                .flat_map(|l| l.iter().filter_map(|n| n.as_str().map(String::from)))
                .collect()
        })
        .unwrap_or_default();
    let (var_pre, take_expr) = if red_vars.is_empty() {
        (
            String::new(),
            format!("__ocOmpTake({irjson}, {args_part}{skeleton})"),
        )
    } else {
        (
            "var __ocOmpRed = false; ".to_string(),
            format!("(__ocOmpRed = __ocOmpTake({irjson}, {args_part}{skeleton})) !== false"),
        )
    };
    let head = format!(
        "{var_pre}if (!(typeof __ocOmpTake === 'function' && typeof globalThis.__ocOmpPrep === 'function' && globalThis.__ocOmpPrep({irjson}) && {take_expr})) {{"
    );
    let else_arm = if red_vars.is_empty() {
        None
    } else {
        let assigns = red_vars
            .iter()
            .enumerate()
            .map(|(k, v)| format!("{v} = __ocOmpRed[{k}];"))
            .collect::<Vec<_>>()
            .join(" ");
        Some(format!("else {{ {assigns} }}"))
    };
    Some((head, else_arm))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// simd 构造两面：`parallel for simd` 尾词与独立 `for simd`，ir.simd
    /// 落字段；plain 标注不带该字段。
    #[test]
    fn transform_simd_constructs() {
        let src_a = "function draw() {\n  // @omp parallel for simd\n  for (let y = 0; y < 4; y += 1) {\n    acc[y] = y * 2;\n  }\n}\n";
        let Some(sites) = analyze(src_a) else { panic!("analyze a") };
        assert_eq!(sites.len(), 1);
        let (ir, _) = build_ir(&sites[0]).expect("build_ir a");
        assert_eq!(ir.get("simd").and_then(|v| v.as_bool()), Some(true), "{ir}");

        let src_b = "function draw() {\n  // @omp for simd\n  for (let y = 0; y < 4; y += 1) {\n    acc[y] = y * 2;\n  }\n}\n";
        let Some(sites_b) = analyze(src_b) else { panic!("analyze b") };
        assert_eq!(sites_b.len(), 1);
        let (ir_b, _) = build_ir(&sites_b[0]).expect("build_ir b");
        assert_eq!(ir_b.get("simd").and_then(|v| v.as_bool()), Some(true), "{ir_b}");

        // 尾词词边界：`simdfoo` 不是 simd 构造。
        let src_c = "function draw() {\n  // @omp parallel for simdfoo\n  for (let y = 0; y < 4; y += 1) {\n    acc[y] = y * 2;\n  }\n}\n";
        let sites_c = analyze(src_c).unwrap_or_default();
        assert!(sites_c.is_empty() || build_ir(&sites_c[0]).unwrap().0.get("simd").is_none());
    }

    #[test]
    fn transform_wraps_and_emits_ir() {
        // bare 标注（OpenMP 缺省 default(shared)）：外层引用隐式 capture。
        let src = "function draw() {\n  // @omp parallel for\n  for (let y = 0; y < h; y += 1) {\n    for (let x = 0; x < w; x += 1) {\n      const noise = random();\n      ctx.fillStyle = `rgba(${noise * 200 | 0}, 1, 1, 1)`;\n      ctx.beginPath();\n      ctx.arc(x, y, 1, 0, Math.PI * 2);\n      ctx.fill();\n    }\n  }\n}\nfunction mix(a, b) { return a; }\n";
        let out = transform(src);
        let s = &*out;
        assert!(s.contains("__ocOmpPrep("));
        assert!(s.contains("__ocOmpTake("));
        assert!(s.contains("Float64Array(__ocOmpN)"));
        assert!(s.contains("for (let y = 0; y < h; y += 1) {"));
        // 隐式名单：头/体自由标识符（h/w/ctx 为跨域全局引用）+ prng 候选。
        // captures 是 serde_json Map（BTreeMap，键字母序）；args 数组保持
        // 源码序（random 先于 ctx）。
        assert!(s.contains("\"args\":[\"h\",\"w\",\"random\",\"ctx\"]"), "{s}");
        assert!(s.contains("\"captures\":{\"ctx\":\"ctx\",\"random\":\"prng\"}"), "{s}");
        // 原循环逐字保留。
        assert!(s.contains("ctx.arc(x, y, 1, 0, Math.PI * 2);"));
    }

    /// smallpt 同款标注形态：`schedule(dynamic, 1) private(r)`，隐式 shared。
    /// ir 带 private 名单；端到端编译通过（private 名字绑定主帧局部槽）。
    #[test]
    fn transform_std_clauses_private_and_schedule() {
        let src = "let r = 0;\nfunction draw() {\n  // @omp parallel for schedule(dynamic, 1) private(r)\n  for (let y = 0; y < 4; y += 1) {\n    for (let x = 0; x < 3; x += 1) {\n      r = x + y;\n      ctx.fillStyle = `rgba(${r | 0}, 1, 1, 1)`;\n      ctx.beginPath();\n      ctx.arc(x, y, 1, 0, Math.PI * 2);\n      ctx.fill();\n    }\n  }\n}\n";
        let out = transform(src);
        let s = &*out;
        assert!(s.contains("__ocOmpPrep("));
        // private 名单进 ir；schedule 原文进 ir；r 不混进 args。
        assert!(s.contains("\"private\":[\"r\"]"));
        assert!(s.contains("\"schedule\":\"dynamic,1\""));
        assert!(s.contains("\"args\":[\"ctx\"]"));
        // 端到端编译：private 绑定 + 体赋值 + facet 五连全链。
        let Some(sites) = analyze(src) else { panic!("analyze") };
        assert_eq!(sites.len(), 1);
        let site = &sites[0];
        let Some((ir, _skeleton)) = build_ir(site) else { panic!("build_ir") };
        let arg_names: Vec<String> = ir["args"].as_array().unwrap()
            .iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let arg_kinds: Vec<crate::script::omp::compile::CaptureKind> = arg_names.iter()
            .map(|n| match ir["captures"].get(n).and_then(|v| v.as_str()) {
                Some("prng") => crate::script::omp::compile::CaptureKind::Prng,
                Some("ctx") => crate::script::omp::compile::CaptureKind::Ctx,
                _ => crate::script::omp::compile::CaptureKind::Plain,
            }).collect();
        let fn_list: Vec<(String, String)> = ir["fns"].as_object().unwrap()
            .iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        let loop_src = ir["loop"].as_str().unwrap().to_string();
        let prog = crate::script::omp::compile::compile(
            &loop_src, &fn_list, &arg_names, &arg_kinds, &["r".to_string()],
        )
        .expect("private loop must compile");
        assert_eq!(prog.arg_names, vec!["ctx".to_string()]);
        assert!(prog.n_slots >= 1);
        assert_eq!(prog.n_globals, 2); // ctx + y
    }

    /// 正式子句不支持面：lastprivate / default 未知值 → 整条标注不处理
    /// （identity，「pragma 未被编译器处理」）。
    #[test]
    fn transform_unsupported_clauses_are_identity() {
        for annot in [
            "// @omp parallel for lastprivate(x)",
            "// @omp parallel for default(firstprivate)",
            "// @omp parallel for schedule(mondynamic)",
            "// @omp parallel for ordered",
            "// @omp parallel for if(1)",
        ] {
            let src = format!("{annot}\nlet notAFor = 1;\n");
            assert_eq!(&*transform(&src), &src, "annot: {annot}");
        }
    }

    /// default(none)：自由标识符漏列 → 不变换；列全 → 变换成功。
    #[test]
    fn transform_default_none_requires_explicit_list() {
        let mk = |shared: &str| {
            format!(
                "function draw() {{\n  // @omp parallel for default(none) {shared}\n  for (let y = 0; y < h; y += 1) {{\n    ctx.fillStyle = `rgba(1, 1, 1, 1)`;\n    ctx.fillRect(0, y, w, 1);\n  }}\n}}\n"
            )
        };
        let missing = mk("shared(ctx)");
        assert_eq!(&*transform(&missing), &missing, "h 未列出 → identity");
        let ok_src = mk("shared(ctx, w, h)");
        let ok = transform(&ok_src);
        assert!(ok.contains("__ocOmpPrep("));
        assert!(ok.contains("\"args\":[\"ctx\",\"w\",\"h\"]"));
    }

    #[test]
    fn transform_no_annotation_is_identity() {
        let src = "for (let i = 0; i < 3; i++) { }\n";
        assert!(matches!(transform(src), Cow::Borrowed(_)));
    }

    #[test]
    fn transform_bad_annotation_is_identity() {
        let src = "// @omp parallel for shared(1+2)\nlet notAFor = 1;\n";
        assert_eq!(&*transform(src), src);
    }

    /// 非源文本上下文的假标注（字符串里的 `// @omp` 不是注释）不触发变换。
    #[test]
    fn transform_ignores_annotation_inside_string() {
        let src = "const s = \"// @omp parallel for\\n\";\nfor (let i = 0; i < 3; i += 1) { }\n";
        assert!(matches!(transform(src), Cow::Borrowed(_)));
    }

    #[test]
    fn skeleton_counts_and_fills() {
        let levels = vec![
            ("let y = 0; y < 2; y += 1".to_string(), String::new()),
            ("let x = 0; x < 3; x += 1".to_string(), String::new()),
        ];
        let sk = build_skeleton(&levels, &["random".to_string()]);
        assert!(sk.contains("for (let y = 0; y < 2; y += 1) {"));
        assert!(sk.contains("__ocOmpN += 1;"));
        assert!(sk.contains("__ocOmpOut[__ocOmpW++] = random();"));
        assert!(sk.matches("} ").count() >= 2);
    }

    /// 语料实分析：inspector halftone 循环 → AST 定位 → 分类 → 编译全链。
    /// 语料缺失时静默跳过（外部工程路径）。输入 = `<script>` 内文本
    /// （与 html.rs 的 transform 调用点同口径）。
    #[test]
    fn corpus_halftone_compiles() {
        let Ok(html) = std::fs::read_to_string("F:/hyperframes-launches/inspector-launch/index.html")
        else {
            return;
        };
        let Some(code) = last_inline_script(&html) else {
            panic!("inline script not found");
        };
        let Some(sites) = analyze(&code) else { panic!("analyze failed") };
        assert!(!sites.is_empty(), "annotation not found");
        let site = &sites[0];
        let Some((ir, skeleton)) = build_ir(site) else {
            panic!("build_ir failed");
        };

        // 分类面：random=prng、ctx=ctx、普通对象（s/mass）不入 captures。
        let caps = ir["captures"].as_object().unwrap();
        assert_eq!(caps.get("random").and_then(|v| v.as_str()), Some("prng"));
        assert_eq!(caps.get("ctx").and_then(|v| v.as_str()), Some("ctx"));
        assert!(caps.get("s").is_none(), "plain object must not be ctx");
        assert!(caps.get("mass").is_none());

        // 传递闭包：8 个页面函数全抽到。
        let fns = ir["fns"].as_object().unwrap();
        for f in [
            "clamp", "mix", "smoothstep", "hash2", "valueNoise", "fbm", "gradientMass",
            "shaderField",
        ] {
            assert!(fns.contains_key(f), "fn `{f}` must be extracted");
        }
        // rand 不该被抽（random 是采集点，不在抽取队列）。
        assert!(!fns.contains_key("rand"));

        // 骨架带两遍重放。
        assert!(skeleton.contains("Float64Array(__ocOmpN)"));
        assert!(skeleton.contains("__ocOmpOut[__ocOmpW++] = random();"));

        // 编译全链。
        let arg_names: Vec<String> = ir["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let arg_kinds: Vec<crate::script::omp::compile::CaptureKind> = arg_names
            .iter()
            .map(|n| match caps.get(n).and_then(|v| v.as_str()) {
                Some("prng") => crate::script::omp::compile::CaptureKind::Prng,
                Some("ctx") => crate::script::omp::compile::CaptureKind::Ctx,
                _ => crate::script::omp::compile::CaptureKind::Plain,
            })
            .collect();
        let fn_list: Vec<(String, String)> = fns
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect();
        let loop_src = ir["loop"].as_str().unwrap().to_string();
        let prog = crate::script::omp::compile::compile(
            &loop_src,
            &fn_list,
            &arg_names,
            &arg_kinds,
            &[],
        )
        .expect("corpus loop must compile");
        assert_eq!(prog.loop_var, "y");
        assert_eq!(prog.arg_names.len(), 14);
        assert!(prog.header_start > 0);
    }

    /// smallpt 语料实分析：递归 radiance 抽取 + 隐式 shared 精确名单。
    /// isolate 后端的验收锚点（opcode 编译面拒绝递归不再妨碍 capture 面）。
    #[test]
    fn corpus_smallpt_analysis() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/smallpt/index.html");
        let Ok(html) = std::fs::read_to_string(path) else { return };
        let Some(code) = last_inline_script(&html) else { panic!("script") };
        let Some(sites) = analyze(&code) else { panic!("analyze failed") };
        assert!(!sites.is_empty());
        let site = &sites[0];
        let Some((ir, skeleton)) = build_ir(site) else { panic!("build_ir") };

        // 递归 radiance 照抽（span 级切源码，无递归拒绝——拒绝面只属
        // opcode 编译器）。
        let fns = ir["fns"].as_object().unwrap();
        for f in ["erand48", "sphereIntersect", "intersect", "radiance", "vec3", "vadd"] {
            assert!(fns.contains_key(f), "fn `{f}` must be extracted");
        }
        // erand48 带参调用 = 不是 prng；行内自持种子归行函数体。
        assert!(caps_str(&ir).get("erand48").is_none());
        // private(r) 进 ir；骨架为 null（无零参采集）。
        assert_eq!(ir["private"], serde_json::json!(["r"]));
        assert_eq!(skeleton, "null");
        // capture 名单含 spheres/cam/cx/cy/w/h/c（数组写回通道）/samps。
        let args = ir["args"].as_array().unwrap();
        for n in ["spheres", "cam", "cx", "cy", "w", "h", "c", "samps"] {
            assert!(args.contains(&serde_json::json!(n)), "args must contain {n}: {args:?}");
        }
    }

    fn caps_str(ir: &JsonValue) -> &serde_json::Map<String, JsonValue> {
        ir["captures"].as_object().unwrap()
    }

    /// html 里最后一个无 src 的 `<script>` 块文本（与 html.rs 的 inline
    /// script 口径一致；测试辅助）。
    fn last_inline_script(html: &str) -> Option<String> {
        let mut last = None;
        let mut cursor = 0usize;
        while let Some(open_rel) = html[cursor..].find("<script") {
            let open = cursor + open_rel;
            let Some(tag_end_rel) = html[open..].find('>') else { break };
            let tag_end = open + tag_end_rel;
            let tag = &html[open..=tag_end];
            if tag.contains("src=") {
                cursor = tag_end + 1;
                continue;
            }
            let Some(close_rel) = html[tag_end..].find("</script>") else { break };
            let close = tag_end + close_rel;
            last = Some(html[tag_end + 1..close].to_string());
            cursor = close + 1;
        }
        last
    }
}
