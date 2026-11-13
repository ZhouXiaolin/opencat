//! `@omp` 提取期文本变换：标注识别 → ir 对象 + PRNG 采集骨架 → 守卫包裹。
//!
//! 标注是零语义注释（asm.js / esbuild `@__PURE__` 同族）——Chrome 直跑
//! 语料行为不变；守卫只存在于 opencat 变换后的 driver 里。任何解析失败
//! 都原样返回（不变换 = 原循环照跑，「pragma 未被编译器处理」降级语义）。
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

// ── 词法扫描（与引擎无关的源码级工具） ──────────────────────────────

/// JS 词法扫描的态：`Code`（携带未配对 `{` 深度）与模板串。`${` 把代码态
/// 压栈、裸 `}` 弹回模板——fillStyle 模板串里的 `${...}` 嵌套安全。
#[derive(Clone, Copy, PartialEq)]
enum ScanMode {
    Template,
    Code(usize),
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// 从 `open`（`{` 的字节索引）扫到配对 `}`。行/块注释与 `'` `"` 字符串内
/// 的花括号忽略；模板串内 `${` 进入代码子态。失败返回 None。
fn matching_brace(src: &str, open: usize) -> Option<usize> {
    let b = src.as_bytes();
    // 栈底恒置一帧 Code(0)：空栈态的 `{` 也计入深度，首个弹底 `}` 即配对。
    let mut stack: Vec<ScanMode> = vec![ScanMode::Code(0)];
    let mut i = open + 1;
    while i < b.len() {
        let in_code = matches!(stack.last(), Some(ScanMode::Code(_)));
        match b[i] {
            // 注释只在代码态有效（模板串里 `/` 是字面量）。
            b'/' if in_code && i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if in_code && i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 1;
            }
            b'\'' | b'"' if in_code => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'`' => match stack.last() {
                Some(ScanMode::Template) => {
                    stack.pop();
                }
                _ => stack.push(ScanMode::Template),
            },
            b'$' if stack.last() == Some(&ScanMode::Template)
                && i + 1 < b.len()
                && b[i + 1] == b'{' =>
            {
                stack.push(ScanMode::Code(0));
                i += 1;
            }
            b'\\' if stack.last() == Some(&ScanMode::Template) => {
                i += 1;
            }
            b'{' if in_code => {
                if let Some(ScanMode::Code(d)) = stack.last_mut() {
                    *d += 1;
                }
            }
            b'}' if in_code => match stack.last_mut() {
                Some(ScanMode::Code(0)) => {
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i);
                    }
                }
                Some(ScanMode::Code(d)) => {
                    *d -= 1;
                }
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
    None
}

/// for 头部 `(...)` 的配对（引号/注释感知）。
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

struct Annotation<'a> {
    /// `args(...)` 内的原始文本（逗号分隔的名字面）。
    args: &'a str,
    /// 标注行在源里的范围 [start, line_end)（不含换行）。
    line_start: usize,
    line_end: usize,
}

/// 找下一处 `@omp parallel for` 行注释。行首必须（可选空白后）以 `//`
/// 开头——字符串/模板里的假命中不成立。`native:<name>` 前缀可选（兼容
/// 旧写法，忽略）。
fn find_annotation(src: &str, from: usize) -> Option<Annotation<'_>> {
    let hay = &src[from..];
    let rel = hay.find("@omp parallel for")?;
    let abs = from + rel;
    // 行首回溯：`@` 前只允许空白与 `//`。
    let line_start = src[..abs].rfind('\n').map_or(0, |p| p + 1);
    let prefix = src[line_start..abs].trim_start();
    let ok = match prefix.strip_prefix("//") {
        Some(rest) => rest.trim().is_empty(),
        None => false,
    };
    if !ok {
        return None;
    }
    let line_end = src[abs..].find('\n').map_or(src.len(), |p| abs + p);
    let mut it = &src[abs + "@omp parallel for".len()..line_end];
    // 可选 native:<name>（旧语法，忽略其名——通用编译器按 args 编译）。
    if let Some(rest) = it.trim_start().strip_prefix("native:") {
        let name_len = rest
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map_or(rest.len(), |(p, _)| p);
        it = &rest[name_len..];
    }
    let args_pos = it.find("args(")?;
    let args_open_abs = line_end - it.len() + args_pos + "args(".len();
    let args_close = src[args_open_abs..].find(')')? + args_open_abs;
    Some(Annotation {
        args: &src[args_open_abs..args_close],
        line_start,
        line_end,
    })
}

/// 定位标注行之后紧贴的 for 语句（含匹配的花括号），返回
/// [for_start, for_end]（闭区间，含 `}`）与 for 行缩进。标注与 for 之间
/// 只容忍空行——注释行/其他语句都判失败（不变换，注释不丢）。
fn find_annotated_for(src: &str, after: usize) -> Option<(usize, usize, usize)> {
    let mut search = after;
    let (for_start, for_end) = loop {
        let line_end = src[search..].find('\n').map_or(src.len(), |p| search + p);
        let line = &src[search..line_end];
        let trimmed = line.trim_start();
        if trimmed.starts_with("for") {
            let for_start = search + (line.len() - trimmed.len());
            let paren_open = src[for_start..].find('(')? + for_start;
            let paren_close = matching_paren(src, paren_open)?;
            let mut brace = paren_close + 1;
            while brace < src.len() && src[brace..].starts_with(char::is_whitespace) {
                brace += 1;
            }
            if !src[brace..].starts_with('{') {
                return None;
            }
            let brace_close = matching_brace(src, brace)?;
            break (for_start, brace_close);
        }
        // 空行跳过；任何其他内容（含注释行）判失败。
        if !trimmed.is_empty() || line_end >= src.len() {
            return None;
        }
        search = line_end + 1;
    };
    let for_line_start = src[..for_start].rfind('\n').map_or(0, |p| p + 1);
    Some((for_start, for_end, for_line_start))
}

/// 逐段扫过字符串/模板/注释，遇到词边界裸标识符后跟 `(` 就回调
/// （成员调用 `.name(` 与关键字/Math 由调用方过滤）。dyn 限定的回调让
/// `${}` 子代码的递归扫描不膨胀单态化。
fn scan_ident_parens(text: &str, cb: &mut dyn FnMut(usize, &str)) {
    let b = text.as_bytes();
    let mut i = 0usize;
    let mut in_template = 0usize; // 模板嵌套深度（${} 计数）
    while i < b.len() {
        // 注释。
        if in_template == 0 && b[i] == b'/' && i + 1 < b.len() {
            if b[i + 1] == b'/' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if b[i + 1] == b'*' {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
        }
        // 字符串。
        if in_template == 0 && (b[i] == b'\'' || b[i] == b'"') {
            let q = b[i];
            i += 1;
            while i < b.len() && b[i] != q {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        // 模板串。
        if b[i] == b'`' {
            in_template += 1;
            i += 1;
            continue;
        }
        if in_template > 0 {
            if b[i] == b'$' && i + 1 < b.len() && b[i + 1] == b'{' {
                // `${` 后进入子代码态：手工跳配对（简化：借助 matching）。
                let close = matching_brace(text, i + 1);
                match close {
                    Some(c) => {
                        // 子代码里也可能有裸调用（采集点常在模板里）——递归扫。
                        scan_ident_parens(&text[i + 2..c], cb);
                        i = c + 1;
                        continue;
                    }
                    None => return,
                }
            }
            if b[i] == b'`' {
                in_template -= 1;
            }
            i += 1;
            continue;
        }
        // 裸标识符。
        if is_ident_byte(b[i]) && !b[i].is_ascii_digit() {
            let start = i;
            let prev_ok = i == 0 || !(is_ident_byte(b[i - 1]) || b[i - 1] == b'.');
            while i < b.len() && is_ident_byte(b[i]) {
                i += 1;
            }
            if prev_ok {
                let name = &text[start..i];
                let mut j = i;
                while j < b.len() && (b[j] as char).is_whitespace() {
                    j += 1;
                }
                if j < b.len() && b[j] == b'(' {
                    cb(start, name);
                }
            }
            continue;
        }
        i += 1;
    }
}

const BARE_CALL_SKIP: &[&str] = &[
    "if", "for", "while", "do", "switch", "catch", "return", "function", "typeof", "new",
    "Math", "Boolean", "Number", "String", "Array", "Object", "JSON", "console", "Promise",
    "parseInt", "parseFloat", "isNaN", "super", "import", "delete", "Symbol", "BigInt",
];

/// 文本里的裸调用名（去重保序，滤关键字/全局）。
fn bare_calls(text: &str, skip: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    scan_ident_parens(text, &mut |_, name| {
        if BARE_CALL_SKIP.contains(&name) || skip.iter().any(|s| s == name) {
            return;
        }
        if !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    });
    out
}

/// 在脚本源里找 `function <name>(` 声明（词边界），返回整段源码范围
/// [start, end]（含配对 `}`）。
fn find_fn_source(source: &str, name: &str) -> Option<(usize, usize)> {
    let pat = format!("function {name}(");
    let mut from = 0usize;
    loop {
        let rel = source[from..].find(&pat)?;
        let abs = from + rel;
        // 词边界：`function` 前不能是标识符字符（排除 `myfunction foo(`）。
        let boundary_ok = abs == 0 || !is_ident_byte(source.as_bytes()[abs - 1]);
        if boundary_ok {
            let paren_open = abs + pat.len() - 1;
            let paren_close = matching_paren(source, paren_open)?;
            let mut brace = paren_close + 1;
            while brace < source.len() && source[brace..].starts_with(char::is_whitespace) {
                brace += 1;
            }
            if !source[brace..].starts_with('{') {
                // `function f() ;`? 不构成函数声明——继续找下一处。
                from = abs + pat.len();
                continue;
            }
            let brace_close = matching_brace(source, brace)?;
            return Some((abs, brace_close));
        }
        from = abs + pat.len();
    }
}

/// 标注 args 名单：逗号分隔、每项必须是纯标识符。
fn parse_arg_names(raw: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for item in raw.split(',') {
        let name = item.trim();
        if name.is_empty() {
            return None;
        }
        if !name.bytes().all(is_ident_byte) || name.as_bytes()[0].is_ascii_digit() {
            return None; // 只收纯标识符（通用契约：args 即值名单）
        }
        if out.iter().any(|n| n == name) {
            return None;
        }
        out.push(name.to_string());
    }
    Some(out)
}

// ── 循环巢结构 ──────────────────────────────────────────────────────

struct NestLevel<'a> {
    /// `for (...)` 括号内的头文本（逐字）。
    header: &'a str,
    /// 花括号内的体文本。
    body: &'a str,
}

/// 解析 for 语句巢：外层→内层的 (头, 体) 链。内层 for 必须在体的花括号
/// 深度 0 处出现；体里 `)` 后到 `{` 之间只许空白。
fn parse_for_chain<'a>(mut stmt: &'a str) -> Option<Vec<NestLevel<'a>>> {
    let mut levels = Vec::new();
    loop {
        let ws: &'a str = stmt.trim_start();
        if !ws.starts_with("for") {
            return None;
        }
        let cur: &'a str = &ws[3..];
        let paren_open = cur.find('(')?;
        let paren_close = matching_paren(cur, paren_open)?;
        let header: &'a str = &cur[paren_open + 1..paren_close];
        let mut brace = paren_close + 1;
        let between = &cur[brace..];
        let trimmed = between.trim_start();
        if !trimmed.starts_with('{') {
            return None;
        }
        brace += between.len() - trimmed.len();
        let brace_close = matching_brace(cur, brace)?;
        let body: &'a str = &cur[brace + 1..brace_close];
        levels.push(NestLevel { header, body });
        // 体里深度 0 的下一个 for。
        match find_depth0_for(body)? {
            Some(p) => stmt = &body[p..],
            None => break,
        }
    }
    Some(levels)
}

/// 体文本里花括号深度 0 处的第一个 `for` 关键字（字符串/注释/模板感知）。
/// None = 扫描失败；Some(None) = 没有更深的 for（已是内层）。
fn find_depth0_for(body: &str) -> Option<Option<usize>> {
    let b = body.as_bytes();
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() {
            if b[i + 1] == b'/' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if b[i + 1] == b'*' {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
        }
        if b[i] == b'\'' || b[i] == b'"' {
            let q = b[i];
            i += 1;
            while i < b.len() && b[i] != q {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if b[i] == b'`' {
            // 模板串：跳到配对反引号（${} 内花括号深度不贡献 depth 计数——
            // 简化：模板里不该有裸 for）。
            i += 1;
            while i < b.len() && b[i] != b'`' {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            continue;
        }
        if b[i] == b'{' {
            depth += 1;
        } else if b[i] == b'}' {
            depth = depth.saturating_sub(1);
        } else if depth == 0
            && b[i] == b'f'
            && body[i..].starts_with("for")
            && (i == 0 || !is_ident_byte(b[i - 1]))
            && i + 3 < b.len()
            && !is_ident_byte(b[i + 3])
        {
            return Some(Some(i));
        }
        i += 1;
    }
    Some(None)
}

// ── 采集点识别与骨架 ────────────────────────────────────────────────

/// 体文本里的零参调用出现（保序、重复名字每次出现各计一次）。
fn prng_occurrences(body: &str, names: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    scan_ident_parens(body, &mut |start, name| {
        if !names.iter().any(|n| n == name) {
            return;
        }
        // 确认零参：`name(` 后跳空白必须是 `)`。
        let b = body.as_bytes();
        let mut j = start + name.len();
        while j < b.len() && (b[j] as char).is_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j] == b'(' {
            j += 1;
            while j < b.len() && (b[j] as char).is_whitespace() {
                j += 1;
            }
            if j < b.len() && b[j] == b')' {
                out.push(name.to_string());
            }
        }
    });
    out
}

/// ctx facet 使用检测：`name.<ident>(` 成员调用接收者，或
/// `name.fillStyle =` 赋值目标。成员**读**不算——`s.phase`/`mass.AX`
/// 这类普通对象成员读会被误分类成 ctx（解码缺 __ocNodeId 即毒化）。
fn ctx_usage(text: &str, name: &str) -> bool {
    let b = text.as_bytes();
    let nb = name.as_bytes();
    let mut i = 0usize;
    while i + nb.len() + 1 <= b.len() {
        if &b[i..i + nb.len()] == nb
            && (i == 0 || !is_ident_byte(b[i - 1]))
            && b[i + nb.len()] == b'.'
        {
            let mut j = i + nb.len() + 1;
            let ks = j;
            while j < b.len() && is_ident_byte(b[j]) {
                j += 1;
            }
            let key = &text[ks..j];
            let mut k = j;
            while k < b.len() && (b[k] as char).is_whitespace() {
                k += 1;
            }
            if k < b.len() && b[k] == b'(' {
                return true; // 成员调用
            }
            if key == "fillStyle"
                && k < b.len()
                && b[k] == b'='
                && b.get(k + 1) != Some(&b'=')
            {
                return true; // fillStyle 赋值
            }
        }
        i += 1;
    }
    false
}

/// 构建采集骨架 IIFE（两遍头部重放）。
fn build_skeleton(levels: &[NestLevel<'_>], prng_calls: &[String]) -> String {
    let mut count = String::from("var __ocOmpN = 0; ");
    let mut fill = String::from("var __ocOmpW = 0; ");
    for lvl in levels {
        count.push_str(&format!("for ({}) {{ ", lvl.header));
        fill.push_str(&format!("for ({}) {{ ", lvl.header));
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
    let mut out = String::with_capacity(source.len() + 1024);
    let mut cursor = 0usize;
    let mut changed = false;
    while let Some(a) = find_annotation(source, cursor) {
        let Some((for_start, for_end, for_line_start)) = find_annotated_for(source, a.line_end)
        else {
            out.push_str(&source[cursor..a.line_end]);
            cursor = a.line_end;
            continue;
        };
        let loop_src = &source[for_start..=for_end];
        match build_wrapper(source, loop_src, a.args) {
            Some(wrapper) => {
                let indent = source[for_line_start..for_start].to_string();
                out.push_str(&source[cursor..for_line_start]);
                out.push_str(&indent);
                out.push_str(&wrapper);
                out.push('\n');
                out.push_str(&source[for_line_start..=for_end]);
                out.push('\n');
                out.push_str(&indent);
                out.push_str("}\n");
                cursor = for_end + 1;
                changed = true;
            }
            None => {
                // 变换失败：标注行原样保留，继续扫后面的标注。
                out.push_str(&source[cursor..a.line_end]);
                cursor = a.line_end;
            }
        }
    }
    if !changed {
        return Cow::Borrowed(source);
    }
    out.push_str(&source[cursor..]);
    Cow::Owned(out)
}

/// 标注 → (ir 对象, 骨架表达式)。失败 = None（不变换）。
fn build_ir(source: &str, loop_src: &str, args_raw: &str) -> Option<(JsonValue, String)> {
    let arg_names = parse_arg_names(args_raw)?;
    let levels = parse_for_chain(loop_src)?;
    let innermost = levels.last()?.body;

    // capture 分类：成员调用/fillStyle 赋值 → ctx；零参调用 → prng；
    // 否则 plain。
    let mut captures = serde_json::Map::new();
    let mut prng_names: Vec<String> = Vec::new();
    let mut skip_for_fns: Vec<String> = Vec::new();
    for n in &arg_names {
        if ctx_usage(loop_src, n) {
            captures.insert(n.clone(), JsonValue::String("ctx".into()));
            skip_for_fns.push(n.clone());
        } else {
            let occ = prng_occurrences(loop_src, std::slice::from_ref(n));
            if !occ.is_empty() {
                captures.insert(n.clone(), JsonValue::String("prng".into()));
                prng_names.push(n.clone());
                skip_for_fns.push(n.clone());
            }
        }
    }
    // 无 ctx 落点：op 流没有去处，骨架还会消耗 PRNG → 不变换。
    if !captures.values().any(|v| v == "ctx") {
        return None;
    }

    // 页面函数传递闭包抽取（超集；编译期再校验面）。
    let mut fns = serde_json::Map::new();
    let mut queue = bare_calls(loop_src, &skip_for_fns);
    let mut done: Vec<String> = Vec::new();
    while let Some(name) = queue.first().cloned() {
        queue.remove(0);
        if done.contains(&name) {
            continue;
        }
        done.push(name.clone());
        let Some((fs, fe)) = find_fn_source(source, &name) else {
            continue; // 不是本脚本声明的函数——编译期若真被调用会拒
        };
        let src = &source[fs..=fe];
        fns.insert(name.clone(), JsonValue::String(src.to_string()));
        for callee in bare_calls(src, &skip_for_fns) {
            if !done.contains(&callee) && !queue.contains(&callee) {
                queue.push(callee);
            }
            if fns.len() >= 64 {
                break;
            }
        }
        if fns.len() >= 64 {
            break;
        }
    }

    // 采集骨架（多采集按体内出现序；无 prng → null）。
    let skeleton = if prng_names.is_empty() {
        "null".to_string()
    } else {
        let occ = prng_occurrences(innermost, &prng_names);
        if occ.is_empty() {
            return None;
        }
        build_skeleton(&levels, &occ)
    };

    let irobj = JsonValue::Object(serde_json::Map::from_iter([
        ("loop".to_string(), JsonValue::String(loop_src.to_string())),
        ("fns".to_string(), JsonValue::Object(fns)),
        ("args".to_string(), JsonValue::Array(
            arg_names.iter().map(|n| JsonValue::String(n.clone())).collect(),
        )),
        ("captures".to_string(), JsonValue::Object(captures)),
    ]));
    Some((irobj, skeleton))
}

/// 为一处标注构建守卫头（不含循环体与收尾 `}`）。
fn build_wrapper(source: &str, loop_src: &str, args_raw: &str) -> Option<String> {
    let (irobj, skeleton) = build_ir(source, loop_src, args_raw)?;
    let irjson = irobj.to_string();
    let args_expr = args_raw.trim();
    Some(format!(
        "if (!(typeof __ocOmpTake === 'function' && typeof globalThis.__ocOmpPrep === 'function' && globalThis.__ocOmpPrep({irjson}) && __ocOmpTake({irjson}, {args_expr}, {skeleton}))) {{"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_wraps_and_emits_ir() {
        let src = "function draw() {\n  // @omp parallel for args(w, h, random, ctx)\n  for (let y = 0; y < h; y += 1) {\n    for (let x = 0; x < w; x += 1) {\n      const noise = random();\n      ctx.fillStyle = `rgba(${noise * 200 | 0}, 1, 1, 1)`;\n      ctx.beginPath();\n      ctx.arc(x, y, 1, 0, Math.PI * 2);\n      ctx.fill();\n    }\n  }\n}\nfunction mix(a, b) { return a; }\n";
        let out = transform(src);
        let s = &*out;
        assert!(s.contains("__ocOmpPrep("));
        assert!(s.contains("__ocOmpTake("));
        assert!(s.contains("Float64Array(__ocOmpN)"));
        assert!(s.contains("for (let y = 0; y < h; y += 1) {"));
        // ir 里带循环源与函数源。
        assert!(s.contains("fns"));
        // 原循环逐字保留。
        assert!(s.contains("ctx.arc(x, y, 1, 0, Math.PI * 2);"));
    }

    #[test]
    fn transform_native_prefix_is_optional() {
        let src = "// @omp parallel for native:anything args(a, ctx)\nfor (let i = 0; i < 3; i += 1) {\n  ctx.fillRect(a, 0, 1, 1);\n}\n";
        let out = transform(src);
        assert!(out.contains("__ocOmpTake("));
    }

    #[test]
    fn transform_no_annotation_is_identity() {
        let src = "for (let i = 0; i < 3; i++) { }\n";
        assert!(matches!(transform(src), Cow::Borrowed(_)));
    }

    #[test]
    fn transform_bad_annotation_is_identity() {
        let src = "// @omp parallel for args(1+2)\nlet notAFor = 1;\n";
        assert_eq!(&*transform(src), src);
    }

    #[test]
    fn skeleton_counts_and_fills() {
        let levels = vec![
            NestLevel { header: "let y = 0; y < 2; y += 1", body: "" },
            NestLevel { header: "let x = 0; x < 3; x += 1", body: "" },
        ];
        let sk = build_skeleton(&levels, &["random".to_string()]);
        assert!(sk.contains("for (let y = 0; y < 2; y += 1) {"));
        assert!(sk.contains("__ocOmpN += 1;"));
        assert!(sk.contains("__ocOmpOut[__ocOmpW++] = random();"));
        assert_eq!(sk.matches("} ").count() >= 2, true);
    }

    /// 语料实编译：inspector halftone 循环 → 抽取 → 分类 → 编译全链。
    /// 语料缺失时静默跳过（外部工程路径）。
    #[test]
    fn corpus_halftone_compiles() {
        let Ok(html) = std::fs::read_to_string("F:/hyperframes-launches/inspector-launch/index.html")
        else {
            return;
        };
        let Some(a) = find_annotation(&html, 0) else {
            panic!("corpus annotation not found");
        };
        let (fs_, fe_, _) = find_annotated_for(&html, a.line_end).expect("for after annotation");
        let loop_src = &html[fs_..=fe_];
        let Some((ir, skeleton)) = build_ir(&html, loop_src, a.args) else {
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
        let prog = crate::script::omp::compile::compile(
            ir["loop"].as_str().unwrap(),
            &fn_list,
            &arg_names,
            &arg_kinds,
        )
        .expect("corpus loop must compile");
        assert_eq!(prog.loop_var, "y");
        assert_eq!(prog.arg_names.len(), 14);
        assert!(prog.header_start > 0);
    }
}
