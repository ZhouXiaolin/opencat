// HTML 前端文档面（docs/html-align/DESIGN.md）。
//
// hyperframes 页面 = 静态 HTML + 页面脚本动态建 DOM + GSAP 时间轴。本运行时
// 把 html5ever 解析出的种子树（`__ocHtmlSeed`）物化成一份 JS 元素树，页面
// 脚本在它上面继续构建最终结构；`__ocHtmlExtract()` 把最终树序列化回
// DomElement JSON。每帧 `__ocHtmlResetFrame` 从种子重物化（脚本重放幂等），
// `__ocHtmlSeek` 把本帧登记的 GSAP 产物 pause + absolute seek —— write-out
// 即 seek，frame = f(t)。
//
// 与 jsx 通道的关系：样式写走同一 store 通道（`__write_style_value`），gsap
// 面按元素对象目标工作（nodeType/tagName/style/classList）。文本内容静态
// （seed 期定稿）；textContent tween 挂账（DESIGN.md 批次 B）。
(function () {
    'use strict';
    // html 通道标记：webgl_api 的 tagAsWebGL2 以此限定作用域（jsx 通道的
    // three 走既有「WebGL1 判定 + 录制面提升」组合）。
    globalThis.__ocHtmlChannel = true;

    var nodes = Object.create(null); // id → element face
    var rootFace = null;
    // 本帧登记的 gsap 产物（timeline 或顶层补间）。必须挂在 globalThis 上：
    // 本文件每帧重 eval，闭包里的数组会随之重建，而 gsap 的包装器只装一次
    // （旧闭包仍持有它），新帧写进旧数组 → 产物永不回收，整片渲染顶到 V8
    // 堆上限（~110 帧即 OOM）。
    var timelines = globalThis.__ocHtmlTimelines || (globalThis.__ocHtmlTimelines = []);
    var synthSeq = 0;
    var seed = null;
    var hasStore = typeof __write_style_value === 'function';

    // ── style 通道 ────────────────────────────────────────────────
    // store 的键是 GSAP 词表，页面脚本写的是 CSS 词表：差异名在这一层
    // 归一（`fontSize` 与 gsap 的 `textSize` 是同一字段）。
    var CSS_KEY_ALIAS = { fontSize: 'textSize' };
    function styleKey(name) {
        name = String(name);
        if (name.indexOf('--') === 0) return name;
        if (name.indexOf('-') >= 0) {
            name = name.replace(/-([a-z])/g, function (_, c) { return c.toUpperCase(); });
        }
        return CSS_KEY_ALIAS[name] || name;
    }

    // 纯色判定：`border` 简写拆出来的颜色段要先验证是颜色本身。
    var FLAT_COLOR = /^#([0-9a-fA-F]{3,8})$|^rgba?\(|^hsla?\(|^[a-zA-Z]+$/;
    function writeStyle(id, key, value) {
        if (!hasStore || !id) return;
        var k = styleKey(key);
        var v = value;
        if (k === 'border') {
            // `border: 1px solid rgba(...)` 拆成 store 的宽度 + 颜色两字段
            var str = String(v);
            var w = /(\d+(?:\.\d+)?)px/.exec(str);
            var c = str.replace(/\d+(?:\.\d+)?px/g, '').replace(/solid|dashed|dotted|none/g, '').trim();
            if (w) __write_style_value(id, 'borderWidth', w[1]);
            if (c && FLAT_COLOR.test(c)) __write_style_value(id, 'borderColor', c);
            return;
        }
        __write_style_value(id, k, v);
    }

    function readStyle(id, key) {
        if (!hasStore || !id) return '';
        var v = __read_style_value(id, styleKey(key));
        return v === null || v === undefined ? '' : v;
    }

    function makeStyleProxy(el) {
        // 取 id 必须延迟：`this.style = makeStyleProxy(this)` 在 id 存取器
        // 定义之前执行，构造期读 `el.id` 恒为 undefined（写全被静默丢弃）。
        function id() { return el.__payload.id; }
        return new Proxy({}, {
            get(_t, prop) {
                if (typeof prop !== 'string') return undefined;
                if (prop === 'setProperty') {
                    return function (name, value) { writeStyle(id(), name, value); };
                }
                if (prop === 'getPropertyValue') {
                    return function (name) { return String(readStyle(id(), name)); };
                }
                if (prop === 'removeProperty') return function () { return ''; };
                if (prop === 'cssText') {
                    return function () {
                        var inline = el.__payload.style || '';
                        var dyn = Object.keys(el.__styleBag).map(function (k) {
                            return k + ': ' + el.__styleBag[k];
                        });
                        return dyn.length ? inline + '; ' + dyn.join('; ') : inline;
                    };
                }
                var v = readStyle(id(), prop);
                return v === '' ? (el.__styleBag[prop] !== undefined ? el.__styleBag[prop] : '') : v;
            },
            set(_t, prop, value) {
                if (typeof prop !== 'string') return false;
                if (prop === 'cssText') {
                    String(value).split(';').forEach(function (decl) {
                        var i = decl.indexOf(':');
                        if (i < 0) return;
                        var name = decl.slice(0, i).trim();
                        if (!name) return;
                        writeStyle(id(), name, decl.slice(i + 1).trim());
                    });
                    return true;
                }
                el.__styleBag[prop] = value;
                writeStyle(id(), prop, value);
                return true;
            },
            // gsap CSSPlugin 用 `'transform' in style` 探测属性支持
            // （_checkPropPrefix）：无 has trap 时探测落空 → gr=null →
            // `_parseTransform` 里 `e.replace` 炸。声明全支持（tree_api 的
            // detachedStyle 同款约定）。
            has() { return true; },
        });
    }

    // ── 选择器引擎 ────────────────────────────────────────────────
    // compound：tag / #id / .class / [attr=v]；组合器：后代 / > / + / ~；
    // 逗号分隔多选择器。伪类伪元素不支持（永不匹配）。
    function parseSelector(sel) {
        var s = String(sel == null ? '' : sel).trim();
        if (!s) return null;
        var seqs = s.split(',').map(function (part) { return part.trim(); }).filter(Boolean);
        var out = [];
        for (var g = 0; g < seqs.length; g++) {
            var seq = parseCompoundSeq(seqs[g]);
            if (!seq) return null;
            out.push(seq);
        }
        return out.length ? out : null;
    }

    // 一个 sequence：[{parts, comb}]，comb 是它与左侧相邻 compound 的组合器；
    // parts[0] 是 subject。右到左匹配从 sequence 末尾开始。
    function parseCompoundSeq(input) {
        var s = input;
        var i = 0;
        var seq = [];
        var pendingComb = null;
        function readCompound() {
            var parts = [];
            var m = /^[a-zA-Z][\w-]*/.exec(s.slice(i));
            if (m) { parts.push({ kind: 'tag', name: m[0].toLowerCase() }); i += m[0].length; }
            for (;;) {
                var ch = s[i];
                if (ch === '#') {
                    m = /^([\w-]+)/.exec(s.slice(i + 1));
                    if (!m) return null;
                    parts.push({ kind: 'id', name: m[1] }); i += 1 + m[0].length;
                } else if (ch === '.') {
                    m = /^([\w-]+)/.exec(s.slice(i + 1));
                    if (!m) return null;
                    parts.push({ kind: 'class', name: m[1] }); i += 1 + m[0].length;
                } else if (ch === '[') {
                    var close = s.indexOf(']', i);
                    if (close < 0) return null;
                    var am = /^([\w:.-]+)\s*(?:([~|^$*]?=)\s*([\s\S]*?))?$/
                        .exec(s.slice(i + 1, close).trim());
                    if (!am) return null;
                    var raw = am[3] === undefined ? '' : am[3];
                    var ci = false;
                    var fm = /\s+([iIsS])$/.exec(raw);
                    if (fm) { ci = fm[1].toLowerCase() === 'i'; raw = raw.slice(0, fm.index); }
                    var attrVal = raw.trim().replace(/^["']|["']$/g, '');
                    parts.push(am[2]
                        ? { kind: 'attr', name: am[1].toLowerCase(), op: am[2], value: attrVal, ci: ci }
                        : { kind: 'attr', name: am[1].toLowerCase(), op: null, value: null });
                    i = close + 1;
                } else if (ch === ':') {
                    return null;
                } else break;
            }
            return parts.length ? parts : null;
        }
        for (;;) {
            while (i < s.length && /\s/.test(s[i])) i++;
            if (i >= s.length) break;
            var parts = readCompound();
            if (!parts) return null;
            var comb = pendingComb || ' ';
            pendingComb = null;
            seq.push({ parts: parts, comb: comb });
            var j = i;
            while (j < s.length && /\s/.test(s[j])) j++;
            if (j >= s.length) break;
            if (s[j] === '>') { pendingComb = '>'; i = j + 1; }
            else if (s[j] === '+') { pendingComb = '+'; i = j + 1; }
            else if (s[j] === '~') { pendingComb = '~'; i = j + 1; }
            // j > i 说明两个 compound 之间隔着空白 = 后代组合器（s[j] 已经是
            // 下一个 compound 的首字符，不能再拿它判空白，否则 `.pin .stem`
            // 这类后代选择器整条被丢弃）。
            else if (j > i) { pendingComb = ' '; i = j; }
            else return null;
        }
        return seq.length ? seq : null;
    }

    function elTag(el) {
        var t = String(el.__payload.type || 'div').toLowerCase();
        // 引擎节点名 → HTML 语义（CSS tag 选择器输入）
        if (t === 'text') return 'span';
        if (t === 'image') return 'img';
        return t;
    }

    function elClasses(el) {
        var cls = el.__payload.className || '';
        return cls ? cls.split(/\s+/) : [];
    }

    // 属性选择器求值（与 crates/opencat-core/src/css/cascade.rs 的
    // attr_value_matches 同语义：`=`/`~=`/`|=`/`^=`/`$=`/`*=` + i 标志）。
    function attrMatches(actual, op, value, ci) {
        var a = ci ? String(actual).toLowerCase() : String(actual);
        var b = ci ? String(value).toLowerCase() : String(value);
        if (op === '=') return a === b;
        if (op === '~=') return a.split(/\s+/).indexOf(b) >= 0;
        if (op === '|=') return a === b || a.indexOf(b + '-') === 0;
        if (op === '^=') return a.indexOf(b) === 0;
        if (op === '$=') return b.length <= a.length && a.slice(a.length - b.length) === b;
        if (op === '*=') return a.indexOf(b) >= 0;
        return false;
    }

    function compoundMatches(el, parts) {
        for (var i = 0; i < parts.length; i++) {
            var p = parts[i];
            if (p.kind === 'tag') {
                if (elTag(el) !== p.name) {
                    // html/body 别名只给根
                    if (!((p.name === 'html' || p.name === 'body') && el === rootFace)) return false;
                }
            } else if (p.kind === 'id') {
                if (el.id !== p.name) return false;
            } else if (p.kind === 'class') {
                if (elClasses(el).indexOf(p.name) < 0) return false;
            } else if (p.kind === 'attr') {
                var v = el.getAttribute(p.name);
                if (p.op === null || p.op === undefined) { if (v === null) return false; }
                else if (v === null || !attrMatches(v, p.op, p.value, p.ci)) return false;
            }
        }
        return true;
    }

    function seqMatches(el, seq) {
        // seq 右到左：seq[len-1] 是 subject。
        var idx = seq.length - 1;
        if (!compoundMatches(el, seq[idx].parts)) return false;
        var cur = el;
        while (idx > 0) {
            idx--;
            var comb = seq[idx + 1].comb; // subject 与左侧的连接
            if (comb === '>') {
                cur = cur.parentNode;
                if (!cur || cur.nodeType !== 1 || !compoundMatches(cur, seq[idx].parts)) return false;
            } else if (comb === ' ') {
                var found = false;
                cur = cur.parentNode;
                while (cur && cur.nodeType === 1) {
                    if (compoundMatches(cur, seq[idx].parts)) { found = true; break; }
                    cur = cur.parentNode;
                }
                if (!found) return false;
            } else if (comb === '+' || comb === '~') {
                var sibs = elementSiblings(cur);
                var at = sibs.indexOf(cur);
                var hit = false;
                for (var k = at - 1; k >= 0; k--) {
                    if (compoundMatches(sibs[k], seq[idx].parts)) { hit = true; break; }
                    if (comb === '+') break; // 相邻只看紧邻前兄
                }
                if (!hit) return false;
                cur = sibs[k];
            } else return false;
        }
        return true;
    }

    function matchesSelector(el, parsed) {
        for (var i = 0; i < parsed.length; i++) {
            if (seqMatches(el, parsed[i])) return true;
        }
        return false;
    }

    function walkAll(el, out) {
        var kids = el.__kids || [];
        for (var i = 0; i < kids.length; i++) {
            if (kids[i].nodeType !== 1) continue; // 文本节点不进选择器树
            out.push(kids[i]);
            walkAll(kids[i], out);
        }
        // svg 子树的查询面（__svgKids）：种子/fragment 里 svg 是整体 inline
        // XML（serializeNode 遇 element 子节点会把 svg 降级 div），所以子面
        // 不进 __kids、只进这里——选择器遍历看得见，提取序列化看不见。
        var sk = el.__svgKids;
        if (sk) {
            for (i = 0; i < sk.length; i++) {
                if (sk[i].nodeType !== 1) continue;
                out.push(sk[i]);
                walkAll(sk[i], out);
            }
        }
        return out;
    }

    // 选择器兄弟语义只看元素（+ / ~ 跳过文本节点，DOM 标准行为）。
    function elementSiblings(el) {
        var parent = el.parentNode;
        if (!parent) return [];
        return (parent.__kids || []).filter(function (k) { return k.nodeType === 1; });
    }

    function queryAll(roots, sel) {
        var parsed = parseSelector(sel);
        if (!parsed) return [];
        var all = [];
        for (var i = 0; i < roots.length; i++) walkAll(roots[i], all);
        return all.filter(function (el) { return matchesSelector(el, parsed); });
    }

    // ── innerHTML fragment 解析 ───────────────────────────────────
    // 页面脚本注入的标记是模板字符串拼的（k3 语料）：div/span/br/img/
    // svg 子树/文本/实体。 tokenizer 级解析，不追求完整 HTML5 语法。
    var ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0' };

    function decodeEntities(text) {
        return String(text).replace(/&(#x?[0-9a-fA-F]+|[a-zA-Z]+);/g, function (_, body) {
            if (body[0] === '#') {
                var code = body[1] === 'x' || body[1] === 'X'
                    ? parseInt(body.slice(2), 16) : parseInt(body.slice(1), 10);
                return isNaN(code) ? _ : String.fromCharCode(code);
            }
            return ENTITIES[body] !== undefined ? ENTITIES[body] : _;
        });
    }

    var VOID_TAGS = { br: 1, img: 1, hr: 1, input: 1, meta: 1, link: 1 };

    // fragment/createElement 的 tag → 引擎 DomElement 归一（serde 枚举只认
    // 固定变体，unknown 直接炸——Rust 解析端的 Div 兜底到不了这条路）。
    // 语义装饰标签（i/b/u/…）降 div：Div 承载文本时 Rust 端合成 P，不丢内容。
    var FRAG_TAG_MAP = {
        img: 'image', image: 'image', video: 'video', audio: 'audio',
        canvas: 'canvas', svg: 'svg',
        p: 'p', span: 'span', text: 'span',
        h1: 'p', h2: 'p', h3: 'p', h4: 'p', h5: 'p', h6: 'p',
        i: 'div', b: 'div', strong: 'div', em: 'div', u: 'div', small: 'div',
        label: 'div', section: 'div', header: 'div', footer: 'div', nav: 'div',
        main: 'div', article: 'div', aside: 'div', hr: 'div', br: 'div',
        ul: 'div', ol: 'div', li: 'div', dl: 'div', dt: 'div', dd: 'div',
        a: 'div', button: 'div', input: 'div', form: 'div', table: 'div',
    };
    function normalizeEngineTag(tag) {
        return FRAG_TAG_MAP[tag] || 'div';
    }

    function xmlEscape(s) {
        return String(s)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    }

    // svg 子树整体序列化成近似 XML（DomElement::Svg.inline 通道，与 Rust
    // 解析端 serialize_subtree 同语义）；子树不再展开成 JS 元素。
    function xmlSerialize(node) {
        var s = '<' + node.tag;
        for (var k in node.attrs) s += ' ' + k + '="' + xmlEscape(node.attrs[k] || '') + '"';
        if (!node.kids.length) return s + '/>';
        s += '>';
        node.kids.forEach(function (kid) {
            if (kid.text !== undefined) s += xmlEscape(kid.text);
            else s += xmlSerialize(kid);
        });
        return s + '</' + node.tag + '>';
    }

    // ── svg 子树的查询面 ─────────────────────────────────────────
    // intro 语料在页面脚本里查 svg 内部元素（`path[d]` 读 d/fill、渐变 stop
    // 读 offset/stop-color）再光栅进 GL 纹理。seed 里 svg 是整体 inline XML
    // 字符串（Rust serialize_subtree 产物：双引号属性、&amp;/&lt;/&gt;/&quot;
    // 实体、`/>` 自闭合），这里 tokenizer 级解析回轻量 face 挂 __svgKids——
    // walkAll 让选择器看得见，serializeNode 看不见（不进提取树）。
    function parseInlineXml(text) {
        var root = { tag: '#root', attrs: {}, kids: [] };
        var stack = [root];
        var s = String(text == null ? '' : text);
        var i = 0;
        while (i < s.length) {
            var lt = s.indexOf('<', i);
            if (lt < 0) lt = s.length;
            if (lt > i) {
                var txt = decodeEntities(s.slice(i, lt));
                if (txt.trim()) {
                    var top = stack[stack.length - 1];
                    top.kids.push({ tag: '#text', attrs: {}, kids: [], text: txt });
                }
            }
            if (lt >= s.length) break;
            i = lt;
            if (s.slice(i, i + 2) === '</') {
                var close = s.indexOf('>', i);
                if (close < 0) break;
                if (stack.length > 1) stack.pop();
                i = close + 1;
            } else {
                var close2 = s.indexOf('>', i);
                if (close2 < 0) break;
                var selfClose = s[close2 - 1] === '/';
                var inner = s.slice(i + 1, selfClose ? close2 - 1 : close2);
                var m = /^([^\s/>]+)[\s\S]*$/.exec(inner);
                if (!m) { i = close2 + 1; continue; }
                var attrs = {};
                var are = /([^\s=/>]+)\s*=\s*"([^"]*)"/g, am;
                while ((am = are.exec(inner))) attrs[am[1]] = decodeEntities(am[2]);
                var bareRest = inner.replace(/([^\s=/>]+)\s*=\s*"[^"]*"/g, '');
                var bare = bareRest.split(/\s+/);
                for (var b = 0; b < bare.length; b++) {
                    if (bare[b] && !(bare[b] in attrs)) attrs[bare[b]] = '';
                }
                var node = { tag: m[1], attrs: attrs, kids: [] };
                stack[stack.length - 1].kids.push(node);
                if (!selfClose) stack.push(node);
                i = close2 + 1;
            }
        }
        return root;
    }

    // 面的 payload.type 直接用真实小写标签名（elTag 的匹配输入）；引擎标签
    // 归一会把未知标签映成 div，tag 选择器（`path`）就断了。这些面不进提取。
    function buildSvgQueryFace(node, parent) {
        var tag = String(node.tag).toLowerCase();
        var payload = {
            type: tag,
            uaTag: tag,
            id: node.attrs.id || nextSynthId(tag),
            className: node.attrs['class'] || '',
            style: node.attrs['style'] || ''
        };
        var face = createElementFace(payload);
        // 带 id 的 svg 内部元素（如 `<defs id="fxdefs">`）按浏览器语义可被
        // getElementById 命中——nodes 表是 getElementById 的唯一查找面。每帧
        // __ocHtmlResetFrame 重建 nodes 后重新走物化，无泄漏。
        if (node.attrs.id) nodes[payload.id] = face;
        for (var k in node.attrs) {
            if (k === 'class' || k === 'style' || k === 'id') continue;
            face.__attrs[k] = node.attrs[k];
        }
        face.parentNode = parent;
        face.__svgKids = [];
        for (var i = 0; i < node.kids.length; i++) {
            if (node.kids[i].tag === '#text') continue;
            face.__svgKids.push(buildSvgQueryFace(node.kids[i], face));
        }
        return face;
    }

    function attachSvgQueryFaces(el, inline) {
        if (!inline || typeof inline !== 'string' || el.__svgKids) return;
        var tree = parseInlineXml(inline);
        el.__svgKids = [];
        for (var i = 0; i < tree.kids.length; i++) {
            if (tree.kids[i].tag === '#text') continue;
            el.__svgKids.push(buildSvgQueryFace(tree.kids[i], el));
        }
    }

    function parseFragment(html) {
        var out = []; // [{tag, attrs, text}]
        var s = String(html == null ? '' : html);
        var i = 0;
        var stack = []; // 未闭合标签栈；text 收进当前栈顶（或根）
        function emit(node) {
            if (stack.length) stack[stack.length - 1].kids.push(node);
            else out.push(node);
        }
        while (i < s.length) {
            var lt = s.indexOf('<', i);
            if (lt < 0) {
                var rest = s.slice(i);
                if (rest) emit({ text: decodeEntities(rest) });
                break;
            }
            if (lt > i) emit({ text: decodeEntities(s.slice(i, lt)) });
            if (s.slice(lt, lt + 4) === '<!--') {
                var end = s.indexOf('-->', lt);
                i = end < 0 ? s.length : end + 3;
                continue;
            }
            var close = /^<\/\s*([a-zA-Z][\w-]*)\s*>/.exec(s.slice(lt));
            if (close) {
                // 弹到同名标签为止（不校验嵌套，语料形态简单）
                var name = close[1].toLowerCase();
                for (var d = stack.length - 1; d >= 0; d--) {
                    if (stack[d].tag === name) { stack.length = d; break; }
                }
                i = lt + close[0].length;
                continue;
            }
            var open = /^<\s*([a-zA-Z][\w-]*)((?:\s+[\w-]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*)\s*(\/?)\s*>/.exec(s.slice(lt));
            if (!open) {
                // 不是标签（比如 `< ` 字面量）→ 文本
                emit({ text: '<' });
                i = lt + 1;
                continue;
            }
            var node = { tag: open[1].toLowerCase(), attrs: {}, kids: [] };
            var attrRe = /([\w-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
            var am;
            while ((am = attrRe.exec(open[2]))) {
                var v = am[2] !== undefined ? am[2] : am[3] !== undefined ? am[3] : am[4] !== undefined ? am[4] : '';
                node.attrs[am[1].toLowerCase()] = decodeEntities(v);
            }
            emit(node);
            if (!open[3] && !VOID_TAGS[node.tag]) stack.push(node);
            i = lt + open[0].length;
        }
        return out;
    }

    // ── 序列化（extract）─────────────────────────────────────────
    // JS 树 → DomElement JSON。裸文本不能挂 Div（DomElement::Div 无 text
    // 字段）：统一合成 P 子节点，与 html.rs 解析期 assemble_children 同语义。
    function serializeNode(el) {
        var p = el.__payload;
        var out = { type: p.type, id: p.id };
        if (p.className) out.className = p.className;
        var inline = p.style || '';
        if (inline) out.style = inline;
        if (p.uaTag) out.uaTag = p.uaTag;
        // 属性表回传（选择器匹配与页内 getAttribute 的输入）。class/style/id
        // 等已单列字段，这里只带其余原文属性与脚本写过的。
        var attrPairs = [];
        for (var an in el.__attrs) attrPairs.push([an, el.__attrs[an]]);
        if (attrPairs.length) out.attrs = attrPairs;
        // svg 整树字符串通道：静态树（Rust 解析端产）与 fragment 路径同字段
        if (p.inline) out.inline = p.inline;
        else if (p.__svgInline) out.inline = p.__svgInline;
        if (p.width !== undefined && p.width !== null) out.width = p.width;
        if (p.height !== undefined && p.height !== null) out.height = p.height;
        if (p.preserveAspectRatio) out.preserveAspectRatio = p.preserveAspectRatio;
        if (p.data_start !== undefined && p.data_start !== null) out['data-start'] = p.data_start;
        if (p.data_duration !== undefined && p.data_duration !== null) out['data-duration'] = p.data_duration;
        // src 按 http(s) 前缀分流 path/url（与 parse/html.rs 的 img/video
        // 解析同语义，serde 端 DomElement 只认这两个字段）；静态 seed 的
        // path/url 原样带回。
        if (p.src) {
            if (/^https?:\/\//i.test(p.src)) out.url = p.src;
            else out.path = p.src;
        } else {
            if (p.path) out.path = p.path;
            if (p.url) out.url = p.url;
        }
        if (p.duration !== undefined && p.duration !== null) out.duration = p.duration;

        var elementKids = el.__kids.filter(function (k) { return k.nodeType === 1; });
        var textContent = p.text || '';

        // 动态文本子节点（createTextNode → appendChild）并进 textContent 池
        var textKids = '';
        el.__kids.forEach(function (k) {
            if (k.nodeType === 3) textKids += k.data;
        });

        // 前后空白裁剪与 html.rs 一致；中间空白保留（CSS 折叠在渲染端）。
        // 注意 JS 的 `\s` 含 U+00A0（&nbsp;）——它是不可折叠空格，必须保留，
        // 否则 k3 逐字 span 用 innerHTML="&nbsp;" 占位的空格会消失。
        var allText = (textContent + textKids)
            .replace(/^[ \t\n\r\f]*\n[ \t\n\r\f]*/, '')
            .replace(/[ \t\n\r\f]*$/, '');

        if (elementKids.length === 0) {
            if (p.type === 'div') {
                if (allText || el.__textDynamic) {
                    out.children = [{ type: 'p', id: p.id + ':t', text: allText }];
                }
            } else {
                // p/span/text 承载文本
                if (allText || p.type === 'p' || p.type === 'span' || p.type === 'text') {
                    out.text = allText;
                }
            }
            return out;
        }

        // 有 element 子节点：
        if (p.type === 'div') {
            var kids = [];
            if (allText) kids.push({ type: 'p', id: p.id + ':t', text: allText });
            elementKids.forEach(function (k) { kids.push(serializeNode(k)); });
            out.children = kids;
            return out;
        }
        // p/span 承载文本却有了元素子节点 → 降级 div + 文本 P 首子
        out.type = 'div';
        var downgraded = [{ type: 'p', id: p.id + ':t', text: allText }];
        elementKids.forEach(function (k) { downgraded.push(serializeNode(k)); });
        out.children = downgraded;
        return out;
    }

    // ── 元素 face ─────────────────────────────────────────────────
    function HtmlText(data) {
        this.nodeType = 3;
        this.nodeName = '#text';
        this.data = String(data);
        this.parentNode = null;
    }

    function FragmentFace() {
        this.nodeType = 11;
        this.nodeName = '#document-fragment';
        this.__kids = [];
        this.parentNode = null;
    }
    FragmentFace.prototype.appendChild = function (child) {
        if (!child) return child;
        if (child.parentNode) child.parentNode.removeChild(child);
        child.parentNode = this;
        this.__kids.push(child);
        return child;
    };

    // 外部库初始化期间（__ocExtPhase）造的探测元素另立命名空间：它们只在
    // 首帧存在，混进主序号会让后续帧的合成 id 整体漂移，mutation 就落空。
    var extSeq = 0;
    function nextSynthId(hint) {
        var base = String(hint || 'el').replace(/[^\w-]/g, '') || 'el';
        if (globalThis.__ocExtPhase) {
            return 'ext:' + base + ':' + (extSeq++);
        }
        var id;
        do { id = base + ':' + (synthSeq++); } while (nodes[id]);
        return id;
    }

    function createElementFace(payload) {
        return new HtmlElement(payload);
    }

    // 媒体元素（video/audio）的最小面：取帧时刻由引擎按 data-start 驱动
    // （frame = f(t)），页面的 play/pause/currentTime 不改变取帧。play 返回
    // 已兑现的 Promise——与浏览器自动播放成功同形（页面普遍 `.catch`）。
    function attachMediaFace(el) {
        el.paused = true;
        el.currentTime = 0;
        el.play = function () { this.paused = false; return Promise.resolve(); };
        el.pause = function () { this.paused = true; };
        el.load = function () {};
    }

    function HtmlElement(payload) {
        this.nodeType = 1;
        this.__payload = payload;
        this.__styleBag = Object.create(null);
        this.__kids = []; // element/text faces；种子 children 在物化期填
        this.parentNode = null;
        this.__attrs = Object.create(null); // data-* 等动态属性袋
        this.__evs = Object.create(null); // addEventListener 事件表（img 加载等）
        // 种子 payload 的原文属性（Rust 解析端产）：属性选择器与
        // getAttribute 的匹配输入。
        if (payload && payload.attrs) {
            for (var ai = 0; ai < payload.attrs.length; ai++) {
                var an = String(payload.attrs[ai][0]).toLowerCase();
                // 有专字段的属性不复刻（class/style/id/src）。data-start/
                // data-duration 有专字段但也留一份原文进 __attrs：dataset
                // 与属性选择器都从这读。
                if (an === 'class' || an === 'style' || an === 'id' || an === 'src') continue;
                this.__attrs[an] = String(payload.attrs[ai][1]);
            }
        }
        this.style = makeStyleProxy(this);
        this.classList = attachClassList(this);
        if (payload && (payload.type === 'video' || payload.type === 'audio')) attachMediaFace(this);

        var self = this;
        Object.defineProperty(this, 'id', {
            get: function () { return self.__payload.id; },
            set: function (v) {
                var old = self.__payload.id;
                if (old && nodes[old] === self) delete nodes[old];
                self.__payload.id = String(v);
                nodes[self.__payload.id] = self;
            },
            enumerable: true,
        });
        Object.defineProperty(this, 'className', {
            get: function () { return self.__payload.className || ''; },
            set: function (v) { self.__payload.className = String(v || ''); },
            enumerable: true,
        });
        Object.defineProperty(this, 'tagName', { get: function () { return elTag(self).toUpperCase(); } });
        this.nodeName = this.tagName;
        Object.defineProperty(this, 'innerHTML', {
            get: function () { return serializeHtml(self); },
            set: function (v) {
                self.__kids = [];
                var frag = parseFragment(v);
                frag.forEach(function (n) { self.__kids.push(materializeFragmentNode(n, self)); });
            },
            enumerable: true,
        });
        Object.defineProperty(this, 'textContent', {
            get: function () {
                var t = self.__payload.text || '';
                self.__kids.forEach(function (k) {
                    if (k.nodeType === 3) t += k.data;
                    else if (k.nodeType === 1) t += k.textContent;
                });
                return t;
            },
            set: function (v) {
                self.__kids = [];
                var text = String(v == null ? '' : v);
                self.__payload.text = text;
                // 文本突变必须进 store：渲染树消费的是提取期种子快照，
                // payload 改动不经 extract 回传（__ocHtmlExtract 只在提取期
                // 调一次）。gsap 的 tl.call 打字/置词都走这条 setter。
                // 目标 id 与 serializeNode 同规则：div 型元素的文本落在合成
                // 子节点 `<id>:t`；提取期写过文本的元素即使为空也保留该子
                // 节点（__textDynamic），渲染期的突变才有落点。
                self.__textDynamic = true;
                if (hasStore && self.__payload.id) {
                    var target = self.__payload.type === 'div'
                        ? self.__payload.id + ':t'
                        : self.__payload.id;
                    __write_style_value(target, 'text', text);
                }
            },
            enumerable: true,
        });
        Object.defineProperty(this, 'children', {
            get: function () { return self.__kids.filter(function (k) { return k.nodeType === 1; }); },
        });
        Object.defineProperty(this, 'childNodes', {
            get: function () { return self.__kids.slice(); },
        });
        Object.defineProperty(this, 'firstChild', {
            get: function () { return self.__kids.length ? self.__kids[0] : null; },
        });
        Object.defineProperty(this, 'lastChild', {
            get: function () { return self.__kids.length ? self.__kids[self.__kids.length - 1] : null; },
        });
        Object.defineProperty(this, 'nextSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                var i = p.__kids.indexOf(self);
                return i >= 0 && i + 1 < p.__kids.length ? p.__kids[i + 1] : null;
            },
        });
        Object.defineProperty(this, 'previousSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                var i = p.__kids.indexOf(self);
                return i > 0 ? p.__kids[i - 1] : null;
            },
        });
        // gsap 量 transform 后按 nextElementSibling 把元素插回原位；缺这个
        // 属性时它退回 appendChild，元素被挪到父节点末尾。
        Object.defineProperty(this, 'nextElementSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                for (var i = p.__kids.indexOf(self) + 1; i > 0 && i < p.__kids.length; i++) {
                    if (p.__kids[i].nodeType === 1) return p.__kids[i];
                }
                return null;
            },
        });
        Object.defineProperty(this, 'previousElementSibling', {
            get: function () {
                var p = self.parentNode;
                if (!p) return null;
                for (var i = p.__kids.indexOf(self) - 1; i >= 0; i--) {
                    if (p.__kids[i].nodeType === 1) return p.__kids[i];
                }
                return null;
            },
        });
        // first/lastElementChild：插入过滤语义与 next/previousElementSibling
        // 一致（heygen mk() 的 `fxdefs.lastElementChild.firstElementChild` 取
        // insertAdjacentHTML 落进 defs 的 filter）。
        Object.defineProperty(this, 'firstElementChild', {
            get: function () {
                for (var i = 0; i < self.__kids.length; i++) {
                    if (self.__kids[i].nodeType === 1) return self.__kids[i];
                }
                return null;
            },
        });
        Object.defineProperty(this, 'lastElementChild', {
            get: function () {
                for (var i = self.__kids.length - 1; i >= 0; i--) {
                    if (self.__kids[i].nodeType === 1) return self.__kids[i];
                }
                return null;
            },
        });
    }

    function detach(child) {
        if (child.parentNode) {
            var sibs = child.parentNode.__kids;
            var i = sibs.indexOf(child);
            if (i >= 0) sibs.splice(i, 1);
        }
    }

    HtmlElement.prototype.appendChild = function (child) {
        if (!child) return child;
        if (child instanceof FragmentFace) {
            var kids = child.__kids.slice();
            kids.forEach(function (k) { this.appendChild(k); }, this);
            child.__kids = [];
            return child;
        }
        detach(child);
        child.parentNode = this;
        this.__kids.push(child);
        return child;
    };
    HtmlElement.prototype.insertBefore = function (child, ref) {
        if (!child) return child;
        if (!ref) return this.appendChild(child);
        if (child instanceof FragmentFace) {
            var kids = child.__kids.slice();
            var self = this;
            kids.forEach(function (k) { self.insertBefore(k, ref); });
            return child;
        }
        detach(child);
        var i = this.__kids.indexOf(ref);
        if (i < 0) { child.parentNode = this; this.__kids.push(child); }
        else { child.parentNode = this; this.__kids.splice(i, 0, child); }
        return child;
    };
    HtmlElement.prototype.removeChild = function (child) {
        var i = this.__kids.indexOf(child);
        if (i >= 0) { this.__kids.splice(i, 1); child.parentNode = null; }
        return child;
    };
    HtmlElement.prototype.replaceChild = function (nu, old) {
        var i = this.__kids.indexOf(old);
        if (i < 0) return this.appendChild(nu);
        this.removeChild(old);
        this.insertBefore(nu, this.__kids[i] || null);
        return old;
    };
    HtmlElement.prototype.contains = function (other) {
        var cur = other;
        while (cur) { if (cur === this) return true; cur = cur.parentNode; }
        return false;
    };
    HtmlElement.prototype.cloneNode = function () {
        var payload = {};
        for (var k in this.__payload) {
            if (Object.prototype.hasOwnProperty.call(this.__payload, k)) payload[k] = this.__payload[k];
        }
        payload.id = nextSynthId(this.__payload.id);
        var copy = createElementFace(payload);
        var self = this;
        this.__kids.forEach(function (k) {
            if (k.nodeType === 3) copy.appendChild(new HtmlText(k.data));
            else if (k.nodeType === 1) copy.appendChild(k.cloneNode(true));
        });
        return copy;
    };
    HtmlElement.prototype.setAttribute = function (name, value) {
        var n = String(name).toLowerCase();
        var v = String(value);
        if (n === 'class') this.__payload.className = v;
        else if (n === 'style') this.__payload.style = v;
        else if (n === 'id') this.id = v;
        else if (n === 'src') this.__payload.src = v;
        else if (n === 'data-start') this.__payload.data_start = parseFloat(v);
        else if (n === 'data-duration') this.__payload.data_duration = parseFloat(v);
        else this.__attrs[n] = v;
    };
    HtmlElement.prototype.getAttribute = function (name) {
        var n = String(name).toLowerCase();
        if (n === 'class') return this.__payload.className || null;
        if (n === 'style') return this.__payload.style || null;
        if (n === 'id') return this.__payload.id;
        if (n === 'src') return this.__payload.src || null;
        if (n === 'data-start') return this.__payload.data_start !== undefined ? String(this.__payload.data_start) : null;
        if (n === 'data-duration') return this.__payload.data_duration !== undefined ? String(this.__payload.data_duration) : null;
        return Object.prototype.hasOwnProperty.call(this.__attrs, n) ? this.__attrs[n] : null;
    };
    HtmlElement.prototype.hasAttribute = function (name) {
        return this.getAttribute(name) !== null;
    };
    HtmlElement.prototype.removeAttribute = function (name) {
        var n = String(name).toLowerCase();
        if (n === 'class') this.__payload.className = '';
        else if (n === 'style') this.__payload.style = '';
        else if (n === 'src') delete this.__payload.src;
        else delete this.__attrs[n];
    };
    // 事件面：从空操作升级为真记录 + 派发。img.src 的异步 load/error
    // （three ImageLoader）依赖它；无人派发时行为与空操作一致。
    HtmlElement.prototype.addEventListener = function (type, fn) {
        if (typeof fn !== 'function') return;
        (this.__evs[String(type)] = this.__evs[String(type)] || []).push(fn);
    };
    HtmlElement.prototype.removeEventListener = function (type, fn) {
        var a = this.__evs[String(type)];
        if (a) {
            var i = a.indexOf(fn);
            if (i >= 0) a.splice(i, 1);
        }
    };
    HtmlElement.prototype.dispatchEvent = function (ev) {
        var a = ev && this.__evs[ev.type];
        if (a) {
            a.slice().forEach(function (fn) {
                try { fn.call(this, ev); } catch (e) { /* 单监听失败不拖垮 */ }
            }, this);
        }
        return true;
    };
    HtmlElement.prototype.focus = function () {};
    HtmlElement.prototype.blur = function () {};

    // 浏览器语义:position ∈ {beforebegin, afterbegin, beforeend, afterend}。
    // 解析与物化复用 innerHTML 的同一链路(parseFragment → materialize
    // FragmentNode,自注册 nodes);<svg> 字符串走整体 inline 通道。
    HtmlElement.prototype.insertAdjacentHTML = function (position, html) {
        var pos = String(position == null ? 'beforeend' : position).toLowerCase();
        var frag = parseFragment(html);
        var self = this;
        var make = function (node) { return materializeFragmentNode(node, null); };
        if (pos === 'beforeend') {
            frag.forEach(function (n) { self.appendChild(make(n)); });
        } else if (pos === 'afterbegin') {
            var first = self.__kids[0] || null;
            frag.forEach(function (n) { self.insertBefore(make(n), first); });
        } else if (pos === 'beforebegin' || pos === 'afterend') {
            var p = self.parentNode;
            if (!p) return;
            if (pos === 'beforebegin') {
                frag.forEach(function (n) { p.insertBefore(make(n), self); });
            } else {
                var anchor = self.nextSibling;
                frag.forEach(function (n) { p.insertBefore(make(n), anchor); });
            }
        }
    };

    // dataset:__attrs 里 data-* 的 camelCase 视图;写走 setAttribute 保证
    // 与选择器/序列化同源(data-start/duration 构造期已留原文进 __attrs)。
    function datasetKey(name) {
        return 'data-' + String(name).replace(/[A-Z]/g, function (c) { return '-' + c.toLowerCase(); });
    }
    Object.defineProperty(HtmlElement.prototype, 'dataset', {
        get: function () {
            var self = this;
            return new Proxy(Object.create(null), {
                get: function (_t, prop) {
                    if (typeof prop !== 'string') return undefined;
                    var k = datasetKey(prop);
                    return Object.prototype.hasOwnProperty.call(self.__attrs, k) ? self.__attrs[k] : undefined;
                },
                set: function (_t, prop, value) {
                    if (typeof prop !== 'string') return false;
                    self.setAttribute(datasetKey(prop), value);
                    return true;
                },
                deleteProperty: function (_t, prop) {
                    if (typeof prop !== 'string') return false;
                    self.removeAttribute(datasetKey(prop));
                    return true;
                },
                has: function (_t, prop) {
                    return typeof prop === 'string'
                        && Object.prototype.hasOwnProperty.call(self.__attrs, datasetKey(prop));
                },
            });
        },
        enumerable: true,
    });

    // append/prepend：多节点变参插入（浏览器 ChildNode API）。字符串参数
    // 按文本节点处理（texture 语料 `look.append(photo, cardA, …)`）。
    HtmlElement.prototype.append = function () {
        for (var i = 0; i < arguments.length; i++) {
            var v = arguments[i];
            this.appendChild(typeof v === 'string' ? new HtmlText(v) : v);
        }
    };
    HtmlElement.prototype.prepend = function () {
        for (var i = arguments.length - 1; i >= 0; i--) {
            var v = arguments[i];
            this.insertBefore(typeof v === 'string' ? new HtmlText(v) : v, this.__kids[0] || null);
        }
    };

    // SVG 几何面（标准 SVGGeometryElement）：html 通道的 svg 查询 face 手上
    // 有原文属性（__attrs.d）。绑定 __path_total_length/__path_bbox 两个
    // realm 都可用（提取期 binding_shim_js、渲染期 ensure_runtime）。非 path
    // 形状（circle/rect 等）语料未命中，回零值，按需后续补。
    HtmlElement.prototype.getTotalLength = function () {
        if (String(this.__payload.type || '').toLowerCase() !== 'path') return 0;
        var d = this.getAttribute('d');
        return d && typeof __path_total_length === 'function' ? __path_total_length(d) : 0;
    };
    HtmlElement.prototype.getBBox = function () {
        if (String(this.__payload.type || '').toLowerCase() === 'path') {
            var d = this.getAttribute('d');
            if (d && typeof __path_bbox === 'function') {
                var b = __path_bbox(d);
                return { x: b[0], y: b[1], width: b[2], height: b[3] };
            }
        }
        return { x: 0, y: 0, width: 0, height: 0 };
    };

    function attachClassList(el) {
        var face = {
            add: function () {
                var cls = elClasses(el);
                for (var i = 0; i < arguments.length; i++) {
                    var c = String(arguments[i]);
                    if (c && cls.indexOf(c) < 0) cls.push(c);
                }
                el.__payload.className = cls.join(' ');
            },
            remove: function () {
                var cls = elClasses(el);
                for (var i = 0; i < arguments.length; i++) {
                    var at = cls.indexOf(String(arguments[i]));
                    if (at >= 0) cls.splice(at, 1);
                }
                el.__payload.className = cls.join(' ');
            },
            toggle: function (c, force) {
                var cls = elClasses(el);
                var at = cls.indexOf(String(c));
                var want = force === undefined ? at < 0 : !!force;
                if (want && at < 0) { cls.push(String(c)); }
                else if (!want && at >= 0) cls.splice(at, 1);
                el.__payload.className = cls.join(' ');
                return want;
            },
            contains: function (c) { return elClasses(el).indexOf(String(c)) >= 0; },
        };
        Object.defineProperty(face, 'value', { get: function () { return el.__payload.className || ''; } });
        return face;
    }

    // 布局盒：先读本帧样式盒，声明盒为空（内容自适应尺寸）时退回上一帧布局
    // 探针的视口坐标盒——`offsetWidth` 是浏览器布局的事实，页面脚本（k3 的
    // pillDims）据它算位置，恒回 0 会让所有这类定位偏掉。
    function boxOf(el) {
        var id = el.__payload.id;
        var raw = (typeof __read_box === 'function' && __read_box(id)) || {};
        var box = {
            left: Number(raw.left) || 0,
            top: Number(raw.top) || 0,
            width: Number(raw.width) || 0,
            height: Number(raw.height) || 0,
        };
        if ((box.width === 0 || box.height === 0) && typeof __read_layout_box === 'function') {
            var laid = __read_layout_box(id);
            if (laid) {
                box.left = Number(laid.left) || 0;
                box.top = Number(laid.top) || 0;
                box.width = Number(laid.width) || 0;
                box.height = Number(laid.height) || 0;
            }
        }
        return box;
    }

    // img.src 赋值 → 异步解码 + load/error 事件（three ImageLoader 的
    // 浏览器语义：监听挂在 src 赋值之前，微任务后派发）。uaTag 判 img
    // （createElementNS('img') 的 face type 走 FRAG_TAG_MAP 归一成 div，
    // uaTag 保留原文）。
    Object.defineProperty(HtmlElement.prototype, 'src', {
        get: function () { return this.__payload.src || ''; },
        set: function (v) {
            this.setAttribute('src', v);
            var self = this;
            if (String(this.__payload.uaTag || '') !== 'img') return;
            Promise.resolve().then(function () {
                var url = self.__payload.src || '';
                var ok = typeof __opencatAssetStat === 'function' && __opencatAssetStat(url) >= 0;
                self.dispatchEvent({ type: ok ? 'load' : 'error', target: self });
            });
        },
    });

    HtmlElement.prototype.getBoundingClientRect = function () {
        var b = boxOf(this);
        return {
            x: b.left, y: b.top, left: b.left, top: b.top,
            right: b.left + b.width, bottom: b.top + b.height,
            width: b.width, height: b.height,
        };
    };
    Object.defineProperty(HtmlElement.prototype, 'offsetWidth', { get: function () { return boxOf(this).width; } });
    Object.defineProperty(HtmlElement.prototype, 'offsetHeight', { get: function () { return boxOf(this).height; } });
    Object.defineProperty(HtmlElement.prototype, 'offsetLeft', { get: function () { return boxOf(this).left; } });
    Object.defineProperty(HtmlElement.prototype, 'offsetTop', { get: function () { return boxOf(this).top; } });
    Object.defineProperty(HtmlElement.prototype, 'clientWidth', { get: function () { return boxOf(this).width; } });
    Object.defineProperty(HtmlElement.prototype, 'clientHeight', { get: function () { return boxOf(this).height; } });
    // offsetLeft/Top 是视口坐标，等价于以 body 为偏移父：挂在文档树上的元素
    // 统一回 body，游离元素回 null（浏览器语义）。gsap 据它判断元素在不在
    // 文档里，null 会走「临时挂到根上量 transform 再插回」的分支。
    Object.defineProperty(HtmlElement.prototype, 'offsetParent', {
        get: function () {
            for (var cur = this.parentNode; cur; cur = cur.parentNode) {
                if (cur === rootFace) return documentFace.body;
            }
            return null;
        },
    });
    HtmlElement.prototype.querySelector = function (sel) {
        var list = queryAll([this], sel);
        return list.length ? list[0] : null;
    };
    HtmlElement.prototype.querySelectorAll = function (sel) { return queryAll([this], sel); };
    HtmlElement.prototype.getElementsByTagName = function (tag) { return byTagNames([this], tag); };
    HtmlElement.prototype.matches = function (sel) {
        var parsed = parseSelector(sel);
        return !!parsed && matchesSelector(this, parsed);
    };

    // innerHTML 读回（调试用途为主）
    function serializeHtml(el) {
        var s = String(el.__payload.text || '');
        el.__kids.forEach(function (k) {
            if (k.nodeType === 3) s += k.data.replace(/&/g, '&amp;').replace(/</g, '&lt;');
            else if (k.nodeType === 1) {
                s += '<' + elTag(k);
                if (k.__payload.className) s += ' class="' + k.__payload.className + '"';
                s += '>';
                s += serializeHtml(k);
                s += '</' + elTag(k) + '>';
            }
        });
        return s;
    }

    function materializeFragmentNode(node, parent) {
        if (node.text !== undefined) return new HtmlText(node.text);
        // svg 子树：整体字符串化进 inline，不展开 JS 子树
        if (node.tag === 'svg') {
            var svgPayload = {
                type: 'svg',
                id: node.attrs['id'] || nextSynthId('svg'),
                className: node.attrs['class'] || '',
                style: node.attrs['style'] || '',
                __svgInline: xmlSerialize(node),
            };
            delete node.attrs['id'];
            var svgEl = createElementFace(svgPayload);
            svgEl.parentNode = parent;
            nodes[svgPayload.id] = svgEl;
            attachSvgQueryFaces(svgEl, svgPayload.__svgInline);
            return svgEl;
        }
        var payload = {
            type: normalizeEngineTag(node.tag),
            uaTag: node.tag,
            id: nextSynthId(node.tag),
            className: node.attrs['class'] || '',
            style: node.attrs['style'] || '',
        };
        delete node.attrs['class'];
        delete node.attrs['style'];
        var bag = Object.create(null);
        for (var a in node.attrs) {
            if (a === 'src') payload.src = node.attrs[a];
            else if (a === 'id') payload.id = node.attrs[a];
            else if (a === 'data-start') payload.data_start = parseFloat(node.attrs[a]);
            else if (a === 'data-duration') payload.data_duration = parseFloat(node.attrs[a]);
            else bag[a] = node.attrs[a];
        }
        var el = createElementFace(payload);
        el.parentNode = parent;
        el.__attrs = bag;
        node.kids.forEach(function (kid) { el.__kids.push(materializeFragmentNode(kid, el)); });
        nodes[payload.id] = el;
        return el;
    }

    // canvas 元素：getContext('2d') → ctx.surface 面（其返回值本身就是 2D
    // 上下文：fillText/measureText/getImageData 直接可用）。页面脚本常先设
    // canvas.width/height 再画再 getImageData —— 代理层每次访问按当前
    // payload 尺寸惰性重建 surface，保证读回矩形永不越界。
    //
    // 像素写入（putImageData）两条腿：surface 有真字形栅格、没有上传通道，
    // 树内 canvas 的像素因此走 `__opencatPutImageData` 记成帧内生成图；
    // 离屏 canvas（createElement 出来、没挂进文档）只被读回，不必合成。
    var putSeq = 0;
    // 树内的 canvas 有父节点；`createElement("canvas")` 出来、只当离屏栅格用
    // 的那张从不 append，parentNode 恒为 null —— 只有前者需要合成。
    function isAttached(el) {
        return !!(el && el.parentNode);
    }
    HtmlElement.prototype.getContext = function (kind) {
        var self = this;
        var k = String(kind).toLowerCase();
        // WebGL2：走 core 的录制面（webgl_api.js 覆盖的 ctx.getContext），
        // three 的 WebGLRenderer 因此能拿到命令流上下文。
        if (k === 'webgl2' || k === 'webgl' || k === 'experimental-webgl') {
            if (self.__glCtx) return self.__glCtx;
            if (typeof ctx === 'undefined' || !ctx || typeof ctx.getContext !== 'function') return null;
            var gw = Math.max(1, Number(self.__payload.width) || 300);
            var gh = Math.max(1, Number(self.__payload.height) || 150);
            self.__glCtx = ctx.getContext(
                self.__payload.id,
                k === 'webgl2' ? 'webgl2' : 'webgl',
                {},
                gw,
                gh,
                0,
                0
            );
            return self.__glCtx;
        }
        if (k !== '2d') return null;
        if (self.__ctx2d) return self.__ctx2d;
        if (typeof ctx === 'undefined' || !ctx || typeof ctx.surface !== 'function') return null;
        var surf = null;
        function ensure() {
            var w = Math.max(1, Number(self.__payload.width) || 300);
            var h = Math.max(1, Number(self.__payload.height) || 150);
            if (!surf || surf.__ocW !== w || surf.__ocH !== h) {
                // 每张 canvas 一张自己的 surface：ctx.surface 按尺寸共享，
                // 同尺寸的两张 canvas 会互相覆写像素（k3 的离屏 ox 与树内
                // hcanvas 正是这种情况）。
                var key = 'html-canvas:' + self.__payload.id + ':' + w + 'x' + h;
                surf = (typeof ctx.__surfaceById === 'function')
                    ? ctx.__surfaceById(key, w, h)
                    : ctx.surface(w, h);
                surf.__ocW = w;
                surf.__ocH = h;
            }
            return surf;
        }
        // 树内的 canvas 走 core 的 2D 命令流（CONTEXT2D_RUNTIME：路径/圆弧/
        // 渐变/drawImage 都映射成 DrawOp）；离屏 canvas 仍用 surface（像素
        // 读回 getImageData 只有它支持）。像素写入两侧都走 putImageData。
        var ckFace = null;
        function ensureCk() {
            if (ckFace) return ckFace;
            if (typeof ctx !== 'undefined' && ctx && typeof ctx.getContext === 'function') {
                try { ckFace = ctx.getContext(self.__payload.id, '2d'); } catch (e) { ckFace = null; }
            }
            return ckFace;
        }
        self.__ctx2d = new Proxy({}, {
            get: function (_t, prop) {
                if (prop === 'canvas') return self;
                if (prop === 'createImageData') {
                    return function (w, h) {
                        var ww = Math.max(1, Math.round(Number(w) || 1));
                        var hh = Math.max(1, Math.round(Number(h) || 1));
                        return { width: ww, height: hh, data: new Uint8ClampedArray(ww * hh * 4) };
                    };
                }
                if (prop === 'putImageData') {
                    return function (img, dx, dy) {
                        if (!img || !img.data || typeof __opencatPutImageData !== 'function') return;
                        if (!isAttached(self)) return;
                        var src = img.data;
                        var u8 = src instanceof Uint8Array
                            ? src
                            : new Uint8Array(src.buffer, src.byteOffset, src.byteLength);
                        putSeq += 1;
                        // 生成图按内容寻址：同键不同像素是硬错误。键里带帧
                        // 时刻；提取期 t<0 的那一遍干跑不出图。
                        var t = Number(globalThis.__ocSeekT);
                        if (!(t >= 0)) return;
                        __opencatPutImageData(
                            self.__payload.id,
                            'html-put:' + self.__payload.id + ':' + t + ':' + putSeq,
                            u8,
                            Number(dx) || 0,
                            Number(dy) || 0,
                            img.width,
                            img.height
                        );
                    };
                }
                if (prop === 'getImageData') {
                    // 挂树画布的笔迹在 CK 命令流里：读回走 CK 面（重放本帧
                    // 已录命令）；未挂树的离屏元素读 surface。
                    var reader = (isAttached(self) ? ensureCk() : null) || ensure();
                    return reader.getImageData.bind(reader);
                }
                if (prop === 'width' || prop === 'height') {
                    var own = ensure()[prop];
                    return typeof own === 'function' ? own.bind(ensure()) : own;
                }
                var c = isAttached(self) ? ensureCk() : null;
                if (c && c[prop] !== undefined) {
                    var cv = c[prop];
                    return typeof cv === 'function' ? cv.bind(c) : cv;
                }
                var s = ensure();
                var v = s[prop];
                return typeof v === 'function' ? v.bind(s) : v;
            },
            set: function (_t, prop, value) {
                var c = isAttached(self) ? ensureCk() : null;
                if (c && c[prop] !== undefined) {
                    c[prop] = value;
                    return true;
                }
                ensure()[prop] = value;
                return true;
            },
        });
        return self.__ctx2d;
    };
    Object.defineProperty(HtmlElement.prototype, 'width', {
        get: function () { return Number(this.__payload.width) || (this.__payload.type === 'canvas' ? 300 : undefined); },
        set: function (v) { this.__payload.width = Number(v); },
    });
    Object.defineProperty(HtmlElement.prototype, 'height', {
        get: function () { return Number(this.__payload.height) || (this.__payload.type === 'canvas' ? 150 : undefined); },
        set: function (v) { this.__payload.height = Number(v); },
    });

    // ── 种子物化 ─────────────────────────────────────────────────
    // serde 的连字符键（"data-start"）归一成 JS 侧驼峰字段。
    function normalizeSeedPayload(p) {
        if (p.data_start === undefined && p['data-start'] !== undefined) {
            var v = parseFloat(p['data-start']);
            if (!isNaN(v)) p.data_start = v;
        }
        if (p.data_duration === undefined && p['data-duration'] !== undefined) {
            var v2 = parseFloat(p['data-duration']);
            if (!isNaN(v2)) p.data_duration = v2;
        }
        return p;
    }

    function materializeSeed(payload, parent) {
        normalizeSeedPayload(payload);
        var el = createElementFace(payload);
        el.parentNode = parent;
        nodes[payload.id] = el;
        if (payload.type === 'svg') attachSvgQueryFaces(el, payload.inline || payload.__svgInline);
        var kids = payload.children || [];
        for (var i = 0; i < kids.length; i++) {
            el.__kids.push(materializeSeed(kids[i], el));
        }
        // seed payload 的文本（text 字段）留在 payload —— 序列化取回。
        return el;
    }

    // ── gsap 包裹 + seek 协议 ─────────────────────────────────────
    function findGsap() {
        if (globalThis.gsap) return globalThis.gsap;
        // UMD gsap 挂 window（shim shim）：提升到 globalThis 让裸标识符可解析。
        if (typeof window !== 'undefined' && window && window.gsap) {
            globalThis.gsap = window.gsap;
            return window.gsap;
        }
        return null;
    }

    function wrapGsap() {
        var g = findGsap();
        if (!g || g.__ocHtmlWrapped) return;
        g.__ocHtmlWrapped = true;
        ['to', 'from', 'fromTo', 'set'].forEach(function (name) {
            var orig = g[name];
            g[name] = function () {
                var anim = orig.apply(g, arguments);
                if (anim) timelines.push(anim);
                return anim;
            };
        });
        var origTimeline = g.timeline;
        g.timeline = function () {
            var tl = origTimeline.apply(g, arguments);
            if (tl) timelines.push(tl);
            return tl;
        };
    }

    // 只驱动 [from, to) 这段 timeline：嵌套 composition 各有自己的时间
    // 基准（宿主 data-start），driver 按段调用。
    function seekRange(t, from, to) {
        for (var i = from; i < to && i < timelines.length; i++) {
            var a = timelines[i];
            try {
                a.pause();
                // 不能抑制事件：页面把整帧重绘挂在 timeline 的 onUpdate 上
                // （k3 的 orbitPills/drawDissolve/hatchGlide 都在这条通道），
                // suppressEvents=true 会让这些逐帧函数永不执行。
                a.totalTime(t, false);
                // 播放头本来就等于 t 时 gsap 会短路、不渲染（首帧 t=0 正是
                // 这种情况：`tl.set(x, {autoAlpha:0}, 0)` 永远不落地）。先推
                // 一丁点再回到 t，逼出一次渲染。
                if (Math.abs(a.totalTime() - t) < 1e-9) {
                    a.totalTime(t + 1e-4, false);
                    a.totalTime(t, false);
                }
            } catch (e) { /* 单产物失败不拖垮整帧 */ }
        }
    }

    globalThis.__ocTimelineCount = function () { return timelines.length; };

    globalThis.__ocHtmlSeek = function (t) {
        globalThis.__ocSeekT = t;
        seekRange(t, 0, timelines.length);
    };

    // [[局部时间, from, to], …]：嵌套 composition 按段用各自的时间基准。
    globalThis.__ocHtmlSeekGroups = function (list) {
        for (var i = 0; i < list.length; i++) {
            var g = list[i];
            globalThis.__ocSeekT = g[0];
            seekRange(g[0], g[1], g[2]);
        }
        // putImageData 的键要用主时间轴的时刻；最后一段是宿主自身。
        globalThis.__ocSeekT = list.length ? list[0][0] : 0;
    };

    globalThis.__ocHtmlAfterScripts = function () {
        wrapGsap(); // 页面自带 gsap 之外的兜底
    };

    // ── document face ─────────────────────────────────────────────
    // getElementsByTagName 共用体:候选含根自身与全部后裔(含 svg 查询子
    // 面,walkAll 已覆盖)。tag 匹配与选择器引擎同语义(elTag 归一),`*`
    // 通配。返回静态数组——HTML 的 live list 本通道无消费者。
    function byTagNames(rootList, tag) {
        var t = String(tag == null ? '' : tag).toLowerCase();
        var out = [];
        for (var i = 0; i < rootList.length; i++) {
            var root = rootList[i];
            if (!root) continue;
            var all = [root].concat(walkAll(root, []));
            for (var j = 0; j < all.length; j++) {
                if (t === '*' || elTag(all[j]) === t) out.push(all[j]);
            }
        }
        return out;
    }
    var documentFace = {
        nodeType: 9,
        documentElement: null, // 物化后指 rootFace
        body: null,
        head: null,
        title: '',
        readyState: 'complete',
        // JS_PLATFORM_SHIM 的空 query 静态形态；页面脚本 document.location。
        location: (typeof globalThis.location === 'object' && globalThis.location) || null,
        getElementById: function (id) {
            var el = nodes[String(id)];
            return el === undefined ? null : el;
        },
        // 浏览器语义：document.querySelector 能匹配文档根元素（html/body）。
        // 种子树里 rootFace 代理 html/body，候选集要含根自身，否则
        // hyperframes 页面 `document.querySelector('[data-composition-id=…]')`
        // 拿到 null（根就是 composition 宿主）。
        querySelector: function (sel) {
            var parsed = parseSelector(sel);
            if (!parsed) return null;
            var all = [rootFace].concat(walkAll(rootFace, []));
            var list = all.filter(function (el) { return matchesSelector(el, parsed); });
            return list.length ? list[0] : null;
        },
        querySelectorAll: function (sel) {
            var parsed = parseSelector(sel);
            if (!parsed) return [];
            var all = [rootFace].concat(walkAll(rootFace, []));
            return all.filter(function (el) { return matchesSelector(el, parsed); });
        },
        getElementsByTagName: function (tag) { return byTagNames([rootFace], tag); },
        createElement: function (tag) {
            var t = String(tag || 'div').toLowerCase();
            var payload = {
                type: normalizeEngineTag(t),
                uaTag: t,
                id: nextSynthId(t),
                className: '',
                style: '',
            };
            var el = createElementFace(payload);
            nodes[payload.id] = el;
            return el;
        },
        // SVG 命名空间建元（three 的 ImageLoader 内部 `createElementNS(xhtml,
        // 'img')` 走这里）：namespace 忽略，与 createElement 同语义。
        createElementNS: function (ns, tag) {
            return this.createElement(tag);
        },
        createTextNode: function (text) { return new HtmlText(text); },
        createDocumentFragment: function () { return new FragmentFace(); },
        addEventListener: function () {},
        removeEventListener: function () {},
    };

    // ── 帧边界 ────────────────────────────────────────────────────
    // CK 面像素读回按 id 查树内 canvas 的位图尺寸（离屏画布见 dom_api）。
    globalThis.__ocHtmlCanvasById = function (id) {
        var el = nodes[String(id)];
        if (!el || !el.__payload) return null;
        return {
            width: Number(el.__payload.width) || 300,
            height: Number(el.__payload.height) || 150,
        };
    };
    globalThis.__ocHtmlResetFrame = function (seedJson) {
        if (seedJson) seed = seedJson;
        // 上一帧的 gsap 产物要 kill 掉：它们挂在 gsap 的全局时间线上并强引用
        // 本帧的 DOM 面，不杀就逐帧累积（整片渲染会顶到 V8 堆上限）。
        for (var i = 0; i < timelines.length; i++) {
            var old = timelines[i];
            if (old && typeof old.kill === 'function') {
                try { old.kill(); } catch (e) { /* 已失效的时间线忽略 */ }
            }
        }
        nodes = Object.create(null);
        rootFace = null;
        timelines.length = 0;
        synthSeq = 0;
        // 页脚本每帧重放、按同序重建 GL 资源：句柄序归零让句柄逐帧稳定，
        // 执行器按句柄复用 GL 对象（单调句柄会逐帧新建对象，全片 OOM）。
        if (typeof __gl_reset_handles === 'function') __gl_reset_handles();
        if (!seed) return;
        rootFace = materializeSeed(seed.tree, null);
        documentFace.documentElement = rootFace;
        documentFace.body = rootFace;
    };

    // 返回对象（宿主统一 JSON.stringify——jsx 的 __ocExtract 同约定；
    // 这里若先 stringify 会被双重包裹成 String，from_extract 读不到 tree）。
    globalThis.__ocHtmlExtract = function () {
        return {
            html: true,
            meta: seed ? seed.meta : {},
            css: seed ? seed.css : '',
            // from_extract 的 tree 是 `Vec<DomElement>`（多根约定）：单根包数组。
            tree: [serializeNode(rootFace)],
        };
    };

    // getComputedStyle 换皮：gsap CSSPlugin `_parseTransform` 从这里读
    // 当前 transform/opacity（`null.replace` 的 null 就来自 dom_api 版对
    // html 元素面返回的空值）。读 store，缺省给 CSS 初始值。
    function computedStyleFor(el) {
        var id = el && el.__payload ? el.__payload.id : null;
        var defaults = {
            transform: 'none',
            opacity: '1',
            visibility: 'visible',
            display: 'block',
            color: 'rgb(0, 0, 0)',
        };
        return new Proxy({}, {
            get(_t, prop) {
                if (typeof prop !== 'string') return undefined;
                if (prop === 'getPropertyValue') {
                    return function (name) {
                        var n = String(name);
                        if (id) {
                            var v = readStyle(id, n);
                            if (v !== '') return String(v);
                        }
                        return Object.prototype.hasOwnProperty.call(defaults, n)
                            ? defaults[n]
                            : '';
                    };
                }
                if (id) {
                    var v = readStyle(id, prop);
                    if (v !== '') return String(v);
                }
                return Object.prototype.hasOwnProperty.call(defaults, prop) ? defaults[prop] : '';
            },
        });
    }
    globalThis.getComputedStyle = computedStyleFor;
    if (ocWinFor()) {
        ocWinFor().getComputedStyle = computedStyleFor;
    }

    // document 覆盖（dom_api 的 chart 面给 jsx 用；HTML 前端换皮）。
    // gsap 从 `window.document` 缓存 doc（`Ie=window, Le=Ie.document`），
    // 所以 shimWindow 这层必须一起换——它藏在 `globalThis.__ocWindow`
    // （dom_api 的 shimWindow 是其 IIFE 局部变量，外面摸不到）。
    globalThis.document = documentFace;
    // 模块工厂（jsx_runtime 的 new Function）把 document 形参绑到
    // `__ocDocument || document`——dom_api 的 chart 面对未知 id 会抛
    // "unknown id"（liquid 语料 module 里 getElementById 中止的根因）；
    // html 通道这里必须换皮成 documentFace。
    globalThis.__ocDocument = documentFace;
    function ocWinFor() {
        return typeof globalThis.__ocWindow === 'object' ? globalThis.__ocWindow : null;
    }
    var ocWin = ocWinFor();
    if (ocWin) ocWin.document = documentFace;
})();
